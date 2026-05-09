//! Things 3 (mac-only) via the `things:///add` URL scheme.
//! No auth, no network. Things must be installed; if it isn't, the URL open
//! fails and we treat that as a permanent push failure.

use super::{AuthKind, PushResult, Target, TaskDestination};
use crate::db::ProviderConfig;
use crate::error::{Error, Result};
use crate::task::Task;
use async_trait::async_trait;
use url::Url;

pub struct ThingsProvider;

impl ThingsProvider {
    fn build_url(task: &Task, target: Option<&str>) -> Result<String> {
        let mut url = Url::parse("things:///add").unwrap();
        {
            let mut q = url.query_pairs_mut();
            q.append_pair("title", &task.text);
            if let Some(due) = &task.due_hint {
                q.append_pair("when", due);
            }
            if let Some(snippet) = &task.source_snippet {
                q.append_pair("notes", snippet);
            }
            if let Some(t) = target {
                if !t.is_empty() && t != "inbox" {
                    q.append_pair("list", t);
                }
            }
        }
        Ok(url.into())
    }
}

#[async_trait]
impl TaskDestination for ThingsProvider {
    fn kind(&self) -> &'static str {
        "things"
    }
    fn display_name(&self) -> &'static str {
        "Things 3"
    }
    fn auth_kind(&self) -> AuthKind {
        AuthKind::UrlScheme
    }

    async fn list_targets(&self, _cfg: &ProviderConfig) -> Result<Vec<Target>> {
        // Things' URL scheme accepts area / project names as free text, and there
        // is no read API. Offer the built-in lists; the user can also type a
        // project or area name in the connect sheet.
        Ok(vec![
            Target { id: "inbox".into(), label: "Inbox".into() },
            Target { id: "today".into(), label: "Today".into() },
            Target { id: "anytime".into(), label: "Anytime".into() },
            Target { id: "someday".into(), label: "Someday".into() },
        ])
    }

    async fn push(&self, cfg: &ProviderConfig, task: &Task) -> Result<PushResult> {
        let url = Self::build_url(task, cfg.target_id.as_deref())?;

        #[cfg(target_os = "macos")]
        {
            let status = std::process::Command::new("/usr/bin/open")
                .arg("-g") // don't steal focus
                .arg(&url)
                .status()
                .map_err(|e| Error::Provider {
                    provider: cfg.id.clone(),
                    message: format!("open Things: {e}"),
                })?;
            if !status.success() {
                return Err(Error::Provider {
                    provider: cfg.id.clone(),
                    message: format!("`open` exited with {:?}", status.code()),
                });
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = url;
            return Err(Error::Provider {
                provider: cfg.id.clone(),
                message: "Things is macOS-only".into(),
            });
        }

        // Things' x-callback-url doesn't return anything synchronously over `open`,
        // so we don't get an external id back. The task is created, but we can't
        // deep-link to it.
        Ok(PushResult {
            external_id: None,
            external_url: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::task::TaskStatus;
    use chrono::Utc;

    #[test]
    fn url_encodes_title_and_when() {
        let t = Task {
            id: "x".into(),
            text: "send Alex the report".into(),
            due_hint: Some("Friday".into()),
            source_snippet: None,
            captured_at: Utc::now(),
            status: TaskStatus::Open,
            confidence: 1.0,
        };
        let u = ThingsProvider::build_url(&t, Some("today")).unwrap();
        assert!(u.starts_with("things:///add?"));
        assert!(u.contains("title=send+Alex+the+report"));
        assert!(u.contains("when=Friday"));
        assert!(u.contains("list=today"));
    }
}
