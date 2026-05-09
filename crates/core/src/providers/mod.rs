//! Task destination providers.
//!
//! Each implementation knows how to push a task to one external system.
//! Auth is per-provider (PAT, integration token, system permission, URL scheme).
//! No OAuth — see SPEC.md.

pub mod things;
pub mod todoist;
pub mod notion;
pub mod webhook;

use crate::db::ProviderConfig;
use crate::error::Result;
use crate::task::Task;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthKind {
    /// User pastes a PAT / integration token. Stored in keychain.
    ApiToken,
    /// macOS system permission (Reminders). Implemented Swift-side via EventKit.
    SystemPermission,
    /// Local URL scheme, no auth (Things 3).
    UrlScheme,
    /// User-supplied URL, optional bearer header.
    Webhook,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Target {
    pub id: String,
    pub label: String,
}

#[derive(Debug, Clone)]
pub struct PushResult {
    pub external_id: Option<String>,
    pub external_url: Option<String>,
}

#[async_trait]
pub trait TaskDestination: Send + Sync {
    fn kind(&self) -> &'static str;
    fn display_name(&self) -> &'static str;
    fn auth_kind(&self) -> AuthKind;

    /// Validate auth + return the available targets (lists / projects / databases).
    async fn list_targets(&self, cfg: &ProviderConfig) -> Result<Vec<Target>>;

    /// Push a task to the configured target. Returns external id/url.
    async fn push(&self, cfg: &ProviderConfig, task: &Task) -> Result<PushResult>;
}

/// Build a provider instance for a given kind. Uses default reqwest client.
pub fn for_kind(kind: &str) -> Option<Box<dyn TaskDestination>> {
    match kind {
        "todoist" => Some(Box::new(todoist::TodoistProvider::new())),
        "notion" => Some(Box::new(notion::NotionProvider::new())),
        "things" => Some(Box::new(things::ThingsProvider)),
        "webhook" => Some(Box::new(webhook::WebhookProvider::new())),
        // "reminders" is implemented in the Swift app — not here.
        _ => None,
    }
}
