//! Todoist REST API v2 provider. Auth = personal API token.
//! Token UI link: https://app.todoist.com/app/settings/integrations/developer

use super::{AuthKind, PushResult, Target, TaskDestination};
use crate::db::ProviderConfig;
use crate::error::{Error, Result};
use crate::keychain::Keychain;
use crate::task::Task;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

const API_BASE: &str = "https://api.todoist.com/rest/v2";

pub struct TodoistProvider {
    client: reqwest::Client,
    base: String,
}

impl TodoistProvider {
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
impl TaskDestination for TodoistProvider {
    fn kind(&self) -> &'static str {
        "todoist"
    }

    fn display_name(&self) -> &'static str {
        "Todoist"
    }

    fn auth_kind(&self) -> AuthKind {
        AuthKind::ApiToken
    }

    async fn list_targets(&self, cfg: &ProviderConfig) -> Result<Vec<Target>> {
        let token = Self::token(cfg)?;
        let resp = self
            .client
            .get(format!("{}/projects", self.base))
            .bearer_auth(&token)
            .send()
            .await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(Error::Provider {
                provider: cfg.id.clone(),
                message: format!("list projects: {status}: {body}"),
            });
        }
        #[derive(Deserialize)]
        struct Project {
            id: String,
            name: String,
        }
        let projects: Vec<Project> = resp.json().await?;
        Ok(projects
            .into_iter()
            .map(|p| Target {
                id: p.id,
                label: p.name,
            })
            .collect())
    }

    async fn push(&self, cfg: &ProviderConfig, task: &Task) -> Result<PushResult> {
        let token = Self::token(cfg)?;
        let target_id = cfg
            .target_id
            .as_deref()
            .ok_or_else(|| Error::InvalidInput("no target picked".into()))?;

        #[derive(Serialize)]
        struct Body<'a> {
            content: &'a str,
            project_id: &'a str,
            #[serde(skip_serializing_if = "Option::is_none")]
            description: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            due_string: Option<&'a str>,
        }

        let description = task.source_snippet.as_ref().map(|s| {
            format!(
                "Captured by TaskListener at {}\n\n> {}",
                task.captured_at.format("%Y-%m-%d %H:%M"),
                s
            )
        });
        let body = Body {
            content: &task.text,
            project_id: target_id,
            description,
            due_string: task.due_hint.as_deref(),
        };

        let resp = self
            .client
            .post(format!("{}/tasks", self.base))
            .bearer_auth(&token)
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(Error::Provider {
                provider: cfg.id.clone(),
                message: format!("create task: {status}: {text}"),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::task::TaskStatus;
    use chrono::Utc;

    fn cfg(id: &str) -> ProviderConfig {
        ProviderConfig {
            id: id.to_string(),
            kind: "todoist".into(),
            display_name: "Todoist".into(),
            enabled: true,
            config_json: "{}".into(),
            min_confidence: 0.0,
            auto_push: true,
            last_synced_at: None,
            keychain_ref: Some(format!("TaskListener.{id}")),
            target_id: Some("12345".into()),
            target_label: Some("Inbox".into()),
        }
    }

    fn task() -> Task {
        Task {
            id: "abc".into(),
            text: "send Alex the report".into(),
            due_hint: Some("Friday".into()),
            source_snippet: Some("…I need to send Alex the report by Friday…".into()),
            captured_at: Utc::now(),
            status: TaskStatus::Open,
            confidence: 0.9,
        }
    }

    #[tokio::test]
    async fn push_sends_expected_body() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/tasks")
            .match_header("authorization", "Bearer secret-token")
            .match_body(mockito::Matcher::PartialJsonString(
                r#"{"content":"send Alex the report","project_id":"12345","due_string":"Friday"}"#
                    .into(),
            ))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"id":"7777","url":"https://todoist.com/showTask?id=7777"}"#)
            .create_async()
            .await;

        let id = format!("todoist-test-{}", uuid::Uuid::new_v4());
        Keychain::store(&id, "secret-token").unwrap();
        let p = TodoistProvider::with_base(server.url());
        let mut c = cfg(&id);
        c.id = id.clone();

        let res = p.push(&c, &task()).await.unwrap();
        assert_eq!(res.external_id.as_deref(), Some("7777"));
        mock.assert_async().await;
        Keychain::delete(&id).unwrap();
    }

    #[tokio::test]
    async fn list_targets_returns_projects() {
        let mut server = mockito::Server::new_async().await;
        let _mock = server
            .mock("GET", "/projects")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"[{"id":"1","name":"Inbox"},{"id":"2","name":"Work"}]"#)
            .create_async()
            .await;

        let id = format!("todoist-test-{}", uuid::Uuid::new_v4());
        Keychain::store(&id, "tok").unwrap();
        let p = TodoistProvider::with_base(server.url());
        let mut c = cfg(&id);
        c.id = id.clone();

        let targets = p.list_targets(&c).await.unwrap();
        assert_eq!(targets.len(), 2);
        assert_eq!(targets[0].label, "Inbox");
        Keychain::delete(&id).unwrap();
    }
}
