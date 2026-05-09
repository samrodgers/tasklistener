//! Notion provider. Auth = internal integration token. The user shares a
//! database with the integration; we list databases via search and push
//! tasks as new pages.

use super::{AuthKind, PushResult, Target, TaskDestination};
use crate::db::ProviderConfig;
use crate::error::{Error, Result};
use crate::keychain::Keychain;
use crate::task::Task;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

const API_BASE: &str = "https://api.notion.com/v1";
const NOTION_VERSION: &str = "2022-06-28";

pub struct NotionProvider {
    client: reqwest::Client,
    base: String,
}

impl NotionProvider {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
            base: API_BASE.to_string(),
        }
    }

    #[cfg(test)]
    pub fn with_base(base: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            base,
        }
    }

    fn token(cfg: &ProviderConfig) -> Result<String> {
        Keychain::get(&cfg.id)?
            .ok_or_else(|| Error::AuthRequired(cfg.id.clone()))
    }
}

#[async_trait]
impl TaskDestination for NotionProvider {
    fn kind(&self) -> &'static str {
        "notion"
    }
    fn display_name(&self) -> &'static str {
        "Notion"
    }
    fn auth_kind(&self) -> AuthKind {
        AuthKind::ApiToken
    }

    async fn list_targets(&self, cfg: &ProviderConfig) -> Result<Vec<Target>> {
        let token = Self::token(cfg)?;
        let resp = self
            .client
            .post(format!("{}/search", self.base))
            .bearer_auth(&token)
            .header("Notion-Version", NOTION_VERSION)
            .json(&json!({ "filter": { "value": "database", "property": "object" } }))
            .send()
            .await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(Error::Provider {
                provider: cfg.id.clone(),
                message: format!("search databases: {status}: {body}"),
            });
        }
        #[derive(Deserialize)]
        struct SearchResp {
            results: Vec<Value>,
        }
        let parsed: SearchResp = resp.json().await?;
        let mut out = Vec::new();
        for db in parsed.results {
            let id = db.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let title = db
                .pointer("/title/0/plain_text")
                .and_then(|v| v.as_str())
                .unwrap_or("Untitled")
                .to_string();
            if !id.is_empty() {
                out.push(Target { id, label: title });
            }
        }
        Ok(out)
    }

    async fn push(&self, cfg: &ProviderConfig, task: &Task) -> Result<PushResult> {
        let token = Self::token(cfg)?;
        let database_id = cfg
            .target_id
            .as_deref()
            .ok_or_else(|| Error::InvalidInput("no target picked".into()))?;

        // We assume the database has a Title property called "Name". Most Notion
        // task templates do; if not, we surface the API error and let the user
        // adjust their database. Keeping the first version simple beats
        // schema-introspection for v0.1.
        #[derive(Serialize)]
        struct Body<'a> {
            parent: Parent<'a>,
            properties: Value,
        }
        #[derive(Serialize)]
        struct Parent<'a> {
            database_id: &'a str,
        }

        let mut properties = json!({
            "Name": {
                "title": [{ "text": { "content": task.text } }]
            }
        });
        if let Some(due) = &task.due_hint {
            // Stash the natural-language due hint as a rich-text "Due Hint" property
            // *if* the database has one; if not, the API call will fail and the user
            // will see the message. v3 will resolve dates locally.
            properties["Due Hint"] = json!({
                "rich_text": [{ "text": { "content": due } }]
            });
        }

        let body = Body {
            parent: Parent { database_id },
            properties,
        };

        let resp = self
            .client
            .post(format!("{}/pages", self.base))
            .bearer_auth(&token)
            .header("Notion-Version", NOTION_VERSION)
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(Error::Provider {
                provider: cfg.id.clone(),
                message: format!("create page: {status}: {text}"),
            });
        }
        #[derive(Deserialize)]
        struct Created {
            id: String,
            url: Option<String>,
        }
        let c: Created = resp.json().await?;
        Ok(PushResult {
            external_id: Some(c.id),
            external_url: c.url,
        })
    }
}
