//! Generic webhook. POSTs the task as JSON to a user-supplied URL.
//! Optional bearer header is stored in the keychain.
//!
//! `config_json` shape: { "url": "https://..." }
//! Keychain entry holds the bearer token (optional — empty string = none).

use super::{AuthKind, PushResult, Target, TaskDestination};
use crate::db::ProviderConfig;
use crate::error::{Error, Result};
use crate::keychain::Keychain;
use crate::task::Task;
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize)]
struct WebhookConfig {
    url: String,
}

pub struct WebhookProvider {
    client: reqwest::Client,
}

impl WebhookProvider {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }

    fn parse_config(cfg: &ProviderConfig) -> Result<WebhookConfig> {
        let parsed: WebhookConfig = serde_json::from_str(&cfg.config_json)
            .map_err(|e| Error::InvalidInput(format!("webhook config: {e}")))?;
        if !parsed.url.starts_with("https://") && !parsed.url.starts_with("http://") {
            return Err(Error::InvalidInput("webhook url must be http(s)".into()));
        }
        Ok(parsed)
    }
}

#[async_trait]
impl TaskDestination for WebhookProvider {
    fn kind(&self) -> &'static str {
        "webhook"
    }
    fn display_name(&self) -> &'static str {
        "Webhook"
    }
    fn auth_kind(&self) -> AuthKind {
        AuthKind::Webhook
    }

    async fn list_targets(&self, _cfg: &ProviderConfig) -> Result<Vec<Target>> {
        // Webhooks have no concept of a target list — the user-supplied URL is
        // the destination. Return a single placeholder so the connect flow can
        // satisfy the "must pick a target" rule trivially.
        Ok(vec![Target {
            id: "default".into(),
            label: "POST to URL".into(),
        }])
    }

    async fn push(&self, cfg: &ProviderConfig, task: &Task) -> Result<PushResult> {
        let parsed = Self::parse_config(cfg)?;
        let mut req = self.client.post(&parsed.url).json(&json!({
            "id": task.id,
            "text": task.text,
            "due_hint": task.due_hint,
            "source_snippet": task.source_snippet,
            "captured_at": task.captured_at.to_rfc3339(),
            "confidence": task.confidence,
        }));
        if let Some(bearer) = Keychain::get(&cfg.id)? {
            if !bearer.is_empty() {
                req = req.bearer_auth(bearer);
            }
        }
        let resp = req.send().await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(Error::Provider {
                provider: cfg.id.clone(),
                message: format!("webhook returned {status}: {body}"),
            });
        }
        Ok(PushResult {
            external_id: None,
            external_url: Some(parsed.url),
        })
    }
}
