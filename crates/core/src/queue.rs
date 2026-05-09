//! Push queue. A single Tokio worker pulls due rows from `task_destinations`,
//! invokes the matching provider, writes back the result.
//!
//! Backoff: 1m, 5m, 30m, 2h. After the 4th failure, dead-letter.
//! Workers are notified via `notify` so freshly-enqueued jobs run immediately.

use crate::db::Db;
use crate::providers;
use crate::task::DestinationState;
use chrono::{Duration, Utc};
use std::sync::Arc;
use tokio::sync::Notify;

const BACKOFF_SECS: &[i64] = &[60, 5 * 60, 30 * 60, 2 * 3600];
const POLL_INTERVAL_SECS: u64 = 30;
const BATCH_SIZE: i64 = 10;

#[derive(Clone)]
pub struct PushQueue {
    db: Db,
    notify: Arc<Notify>,
}

impl PushQueue {
    pub fn new(db: Db) -> Self {
        Self {
            db,
            notify: Arc::new(Notify::new()),
        }
    }

    pub fn notify(&self) {
        self.notify.notify_one();
    }

    /// Spawn the worker on the engine's runtime.
    pub fn spawn(&self, engine: crate::Engine) {
        let db = self.db.clone();
        let notify = self.notify.clone();

        if let Err(e) = db.reset_pushing_jobs() {
            tracing::warn!(error = %e, "reset_pushing_jobs at startup failed");
        }

        let worker_engine = engine.clone();
        engine.runtime().spawn(async move {
            loop {
                if let Err(e) = run_once(&db, &worker_engine).await {
                    tracing::warn!(error = %e, "push worker tick failed");
                }
                tokio::select! {
                    _ = notify.notified() => {},
                    _ = tokio::time::sleep(std::time::Duration::from_secs(POLL_INTERVAL_SECS)) => {},
                }
            }
        });
    }
}

async fn run_once(db: &Db, engine: &crate::Engine) -> crate::Result<()> {
    let due = db.claim_due_jobs(BATCH_SIZE)?;
    if due.is_empty() {
        return Ok(());
    }
    for row in due {
        process_one(db, engine, row).await;
    }
    Ok(())
}

async fn process_one(db: &Db, engine: &crate::Engine, row: crate::TaskDestinationRow) {
    let provider_cfg = match db.get_provider(&row.provider) {
        Ok(Some(p)) => p,
        Ok(None) => {
            let _ = db.record_push_failure(
                &row.id,
                "provider not found",
                None,
                true,
            );
            return;
        }
        Err(e) => {
            tracing::error!(error = %e, "load provider config");
            return;
        }
    };
    let task = match db.get_task(&row.task_id) {
        Ok(Some(t)) => t,
        Ok(None) => {
            // Local task was deleted; cancel.
            let _ = db.record_push_failure(&row.id, "task deleted", None, true);
            return;
        }
        Err(e) => {
            tracing::error!(error = %e, "load task");
            return;
        }
    };
    let provider = match providers::for_kind(&provider_cfg.kind) {
        Some(p) => p,
        None => {
            let _ = db.record_push_failure(
                &row.id,
                &format!("no driver for kind '{}'", provider_cfg.kind),
                None,
                true,
            );
            return;
        }
    };

    engine.emit(crate::Event::DestinationStateChanged {
        task_id: task.id.clone(),
        provider: provider_cfg.id.clone(),
        state: DestinationState::Pushing,
    });

    match provider.push(&provider_cfg, &task).await {
        Ok(result) => {
            let _ = db.record_push_success(
                &row.id,
                result.external_id.as_deref(),
                result.external_url.as_deref(),
            );
            engine.emit(crate::Event::DestinationStateChanged {
                task_id: task.id,
                provider: provider_cfg.id,
                state: DestinationState::Pushed,
            });
        }
        Err(err) => {
            let attempts = row.attempts + 1;
            let dead_letter = attempts as usize >= BACKOFF_SECS.len();
            let next = if dead_letter {
                None
            } else {
                Some(Utc::now() + Duration::seconds(BACKOFF_SECS[attempts as usize - 1]))
            };
            let msg = err.to_string();
            let _ = db.record_push_failure(&row.id, &msg, next, dead_letter);
            engine.emit(crate::Event::DestinationStateChanged {
                task_id: task.id,
                provider: provider_cfg.id,
                state: if dead_letter {
                    DestinationState::DeadLetter
                } else {
                    DestinationState::Failed
                },
            });
        }
    }
}
