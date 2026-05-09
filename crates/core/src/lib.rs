//! TaskListener core: storage, providers, push queue, audio pipeline (stubbed).
//!
//! The Rust core is consumed via the C ABI in the `tasklistener-ffi` crate; the
//! SwiftUI and WinUI front-ends call into that ABI. Nothing in this crate knows
//! about UI.

pub mod audio;
pub mod config;
pub mod db;
pub mod error;
pub mod keychain;
pub mod providers;
pub mod queue;
pub mod task;

pub use error::{Error, Result};
pub use task::{Task, TaskStatus, NewTask, DestinationState, TaskDestinationRow};

use parking_lot::Mutex;
use std::sync::Arc;
use tokio::runtime::Runtime;

/// Top-level handle the FFI layer holds. Owns the runtime, db pool, queue and
/// active provider instances. Cheap to clone (`Arc` inside).
#[derive(Clone)]
pub struct Engine {
    inner: Arc<EngineInner>,
}

struct EngineInner {
    runtime: Runtime,
    db: db::Db,
    queue: queue::PushQueue,
    subscribers: Mutex<Vec<Box<dyn Fn(Event) + Send + Sync>>>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Event {
    TaskCreated { task_id: String },
    TaskUpdated { task_id: String },
    TaskDeleted { task_id: String },
    DestinationStateChanged {
        task_id: String,
        provider: String,
        state: DestinationState,
    },
    ProviderConnected { provider: String },
    ProviderDisconnected { provider: String },
}

impl Engine {
    /// Open or create the SQLite database at `db_path` and start the push queue.
    pub fn start(db_path: std::path::PathBuf) -> Result<Self> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .map_err(|e| Error::Internal(format!("runtime: {e}")))?;

        let db = db::Db::open(&db_path)?;
        db.migrate()?;

        let queue = queue::PushQueue::new(db.clone());

        let inner = Arc::new(EngineInner {
            runtime,
            db,
            queue,
            subscribers: Mutex::new(Vec::new()),
        });

        let engine = Engine { inner };
        engine.inner.queue.spawn(engine.clone());
        Ok(engine)
    }

    pub fn db(&self) -> &db::Db {
        &self.inner.db
    }

    pub fn queue(&self) -> &queue::PushQueue {
        &self.inner.queue
    }

    pub fn runtime(&self) -> &Runtime {
        &self.inner.runtime
    }

    pub fn subscribe(&self, cb: Box<dyn Fn(Event) + Send + Sync>) {
        self.inner.subscribers.lock().push(cb);
    }

    pub fn emit(&self, event: Event) {
        for cb in self.inner.subscribers.lock().iter() {
            cb(event.clone());
        }
    }

    /// Capture a task from any source (manual entry, audio pipeline, test).
    /// Persists, fans out to enabled providers, returns the new id.
    pub fn capture(&self, new: NewTask) -> Result<String> {
        let id = self.inner.db.insert_task(&new)?;
        self.emit(Event::TaskCreated { task_id: id.clone() });

        let providers = self.inner.db.list_enabled_providers()?;
        for p in providers {
            if new.confidence < p.min_confidence {
                continue;
            }
            // Skip kinds we don't have a Rust driver for — those (e.g. Apple
            // Reminders via EventKit) are handled by the front-end, which
            // subscribes to TaskCreated and records results via
            // `record_external_push`.
            if providers::for_kind(&p.kind).is_none() {
                continue;
            }
            self.inner.db.enqueue_push(&id, &p.id)?;
        }
        self.inner.queue.notify();
        Ok(id)
    }
}
