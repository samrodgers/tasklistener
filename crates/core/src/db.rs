use crate::error::{Error, Result};
use crate::task::{DestinationState, NewTask, Task, TaskDestinationRow, TaskStatus};
use chrono::{DateTime, TimeZone, Utc};
use parking_lot::Mutex;
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub struct Db {
    conn: Arc<Mutex<Connection>>,
}

#[derive(Debug, Clone)]
pub struct ProviderConfig {
    pub id: String,
    pub kind: String, // "todoist" | "notion" | "things" | "webhook" | "reminders"
    pub display_name: String,
    pub enabled: bool,
    pub config_json: String,
    pub min_confidence: f32,
    pub auto_push: bool,
    pub last_synced_at: Option<DateTime<Utc>>,
    pub keychain_ref: Option<String>,
    pub target_id: Option<String>,
    pub target_label: Option<String>,
}

impl Db {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA foreign_keys = ON;
             PRAGMA synchronous = NORMAL;",
        )?;
        Ok(Db {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;
        Ok(Db {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub fn migrate(&self) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute_batch(MIGRATIONS)?;
        Ok(())
    }

    // ---- tasks ----

    pub fn insert_task(&self, new: &NewTask) -> Result<String> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now();
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO tasks (id, text, due_hint, source_snippet, captured_at, status, confidence)
             VALUES (?1, ?2, ?3, ?4, ?5, 'open', ?6)",
            params![
                id,
                new.text,
                new.due_hint,
                new.source_snippet,
                now.timestamp(),
                new.confidence,
            ],
        )?;
        Ok(id)
    }

    pub fn get_task(&self, id: &str) -> Result<Option<Task>> {
        let conn = self.conn.lock();
        let task = conn
            .query_row(
                "SELECT id, text, due_hint, source_snippet, captured_at, status, confidence
                 FROM tasks WHERE id = ?1",
                [id],
                row_to_task,
            )
            .optional()?;
        Ok(task)
    }

    pub fn list_tasks(&self, include_done: bool, limit: i64) -> Result<Vec<Task>> {
        let conn = self.conn.lock();
        let sql = if include_done {
            "SELECT id, text, due_hint, source_snippet, captured_at, status, confidence
             FROM tasks ORDER BY captured_at DESC LIMIT ?1"
        } else {
            "SELECT id, text, due_hint, source_snippet, captured_at, status, confidence
             FROM tasks WHERE status = 'open' ORDER BY captured_at DESC LIMIT ?1"
        };
        let mut stmt = conn.prepare(sql)?;
        let rows = stmt.query_map([limit], row_to_task)?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn update_task_text(&self, id: &str, text: &str) -> Result<()> {
        let conn = self.conn.lock();
        let n = conn.execute(
            "UPDATE tasks SET text = ?1 WHERE id = ?2",
            params![text, id],
        )?;
        if n == 0 {
            return Err(Error::NotFound(format!("task {id}")));
        }
        Ok(())
    }

    pub fn set_task_status(&self, id: &str, status: TaskStatus) -> Result<()> {
        let conn = self.conn.lock();
        let n = conn.execute(
            "UPDATE tasks SET status = ?1 WHERE id = ?2",
            params![status.as_str(), id],
        )?;
        if n == 0 {
            return Err(Error::NotFound(format!("task {id}")));
        }
        Ok(())
    }

    pub fn delete_task(&self, id: &str) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute("DELETE FROM tasks WHERE id = ?1", [id])?;
        Ok(())
    }

    // ---- providers ----

    pub fn upsert_provider(&self, p: &ProviderConfig) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO providers (id, kind, display_name, enabled, config_json,
                                    min_confidence, auto_push, last_synced_at,
                                    keychain_ref, target_id, target_label)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
             ON CONFLICT(id) DO UPDATE SET
               kind = excluded.kind,
               display_name = excluded.display_name,
               enabled = excluded.enabled,
               config_json = excluded.config_json,
               min_confidence = excluded.min_confidence,
               auto_push = excluded.auto_push,
               keychain_ref = excluded.keychain_ref,
               target_id = excluded.target_id,
               target_label = excluded.target_label",
            params![
                p.id,
                p.kind,
                p.display_name,
                p.enabled as i32,
                p.config_json,
                p.min_confidence,
                p.auto_push as i32,
                p.last_synced_at.map(|t| t.timestamp()),
                p.keychain_ref,
                p.target_id,
                p.target_label,
            ],
        )?;
        Ok(())
    }

    pub fn get_provider(&self, id: &str) -> Result<Option<ProviderConfig>> {
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT id, kind, display_name, enabled, config_json, min_confidence, auto_push,
                    last_synced_at, keychain_ref, target_id, target_label
             FROM providers WHERE id = ?1",
            [id],
            row_to_provider,
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn list_providers(&self) -> Result<Vec<ProviderConfig>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, kind, display_name, enabled, config_json, min_confidence, auto_push,
                    last_synced_at, keychain_ref, target_id, target_label
             FROM providers ORDER BY display_name",
        )?;
        let rows = stmt.query_map([], row_to_provider)?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    /// Providers that are enabled and have a target picked (auto-push capable).
    pub fn list_enabled_providers(&self) -> Result<Vec<ProviderConfig>> {
        Ok(self
            .list_providers()?
            .into_iter()
            .filter(|p| p.enabled && p.auto_push && p.target_id.is_some())
            .collect())
    }

    pub fn delete_provider(&self, id: &str) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute("DELETE FROM providers WHERE id = ?1", [id])?;
        Ok(())
    }

    // ---- task_destinations / push queue ----

    pub fn enqueue_push(&self, task_id: &str, provider_id: &str) -> Result<()> {
        let id = Uuid::new_v4().to_string();
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO task_destinations
                (id, task_id, provider, state, attempts, next_attempt_at)
             VALUES (?1, ?2, ?3, 'pending', 0, ?4)
             ON CONFLICT(task_id, provider) DO NOTHING",
            params![id, task_id, provider_id, Utc::now().timestamp()],
        )?;
        Ok(())
    }

    pub fn list_destinations_for_task(&self, task_id: &str) -> Result<Vec<TaskDestinationRow>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, task_id, provider, external_id, external_url, pushed_at,
                    last_error, state, attempts, next_attempt_at
             FROM task_destinations WHERE task_id = ?1 ORDER BY provider",
        )?;
        let rows = stmt.query_map([task_id], row_to_destination)?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    /// Pull due jobs (state=pending|failed AND next_attempt_at <= now) up to `limit`,
    /// claim them by flipping state to 'pushing'. Returns claimed rows.
    pub fn claim_due_jobs(&self, limit: i64) -> Result<Vec<TaskDestinationRow>> {
        let conn = self.conn.lock();
        let now = Utc::now().timestamp();
        let mut stmt = conn.prepare(
            "SELECT id, task_id, provider, external_id, external_url, pushed_at,
                    last_error, state, attempts, next_attempt_at
             FROM task_destinations
             WHERE state IN ('pending', 'failed') AND next_attempt_at <= ?1
             ORDER BY next_attempt_at ASC LIMIT ?2",
        )?;
        let rows: Vec<_> = stmt
            .query_map(params![now, limit], row_to_destination)?
            .collect::<rusqlite::Result<_>>()?;

        for r in &rows {
            conn.execute(
                "UPDATE task_destinations SET state = 'pushing' WHERE id = ?1",
                [&r.id],
            )?;
        }
        Ok(rows)
    }

    /// Record an externally-handled push (e.g. Apple Reminders via EventKit
    /// in the Swift app). Inserts the destination row if missing.
    pub fn record_external_push(
        &self,
        task_id: &str,
        provider_id: &str,
        external_id: Option<&str>,
        external_url: Option<&str>,
        error: Option<&str>,
    ) -> Result<()> {
        let conn = self.conn.lock();
        let row_id = Uuid::new_v4().to_string();
        let now = Utc::now().timestamp();
        let (state, pushed_at) = if error.is_some() {
            ("failed", None)
        } else {
            ("pushed", Some(now))
        };
        conn.execute(
            "INSERT INTO task_destinations
                (id, task_id, provider, external_id, external_url, pushed_at,
                 last_error, state, attempts, next_attempt_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 1, NULL)
             ON CONFLICT(task_id, provider) DO UPDATE SET
                external_id = excluded.external_id,
                external_url = excluded.external_url,
                pushed_at = excluded.pushed_at,
                last_error = excluded.last_error,
                state = excluded.state,
                attempts = task_destinations.attempts + 1",
            params![row_id, task_id, provider_id, external_id, external_url, pushed_at, error, state],
        )?;
        Ok(())
    }

    pub fn record_push_success(
        &self,
        destination_id: &str,
        external_id: Option<&str>,
        external_url: Option<&str>,
    ) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE task_destinations
                SET state = 'pushed', external_id = ?1, external_url = ?2,
                    pushed_at = ?3, last_error = NULL
              WHERE id = ?4",
            params![external_id, external_url, Utc::now().timestamp(), destination_id],
        )?;
        Ok(())
    }

    pub fn record_push_failure(
        &self,
        destination_id: &str,
        err: &str,
        next_attempt_at: Option<DateTime<Utc>>,
        dead_letter: bool,
    ) -> Result<()> {
        let state = if dead_letter { "dead_letter" } else { "failed" };
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE task_destinations
                SET state = ?1, last_error = ?2, attempts = attempts + 1,
                    next_attempt_at = ?3
              WHERE id = ?4",
            params![
                state,
                err,
                next_attempt_at.map(|t| t.timestamp()),
                destination_id,
            ],
        )?;
        Ok(())
    }

    pub fn reset_pushing_jobs(&self) -> Result<()> {
        // Called on startup so jobs that were 'pushing' when we crashed are retried.
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE task_destinations SET state = 'pending' WHERE state = 'pushing'",
            [],
        )?;
        Ok(())
    }
}

fn row_to_task(row: &rusqlite::Row<'_>) -> rusqlite::Result<Task> {
    let captured_ts: i64 = row.get(4)?;
    let status_str: String = row.get(5)?;
    Ok(Task {
        id: row.get(0)?,
        text: row.get(1)?,
        due_hint: row.get(2)?,
        source_snippet: row.get(3)?,
        captured_at: Utc
            .timestamp_opt(captured_ts, 0)
            .single()
            .unwrap_or_else(Utc::now),
        status: TaskStatus::parse(&status_str).unwrap_or(TaskStatus::Open),
        confidence: row.get(6)?,
    })
}

fn row_to_destination(row: &rusqlite::Row<'_>) -> rusqlite::Result<TaskDestinationRow> {
    let pushed_ts: Option<i64> = row.get(5)?;
    let next_ts: Option<i64> = row.get(9)?;
    let state_str: String = row.get(7)?;
    Ok(TaskDestinationRow {
        id: row.get(0)?,
        task_id: row.get(1)?,
        provider: row.get(2)?,
        external_id: row.get(3)?,
        external_url: row.get(4)?,
        pushed_at: pushed_ts.and_then(|t| Utc.timestamp_opt(t, 0).single()),
        last_error: row.get(6)?,
        state: DestinationState::parse(&state_str).unwrap_or(DestinationState::Pending),
        attempts: row.get(8)?,
        next_attempt_at: next_ts.and_then(|t| Utc.timestamp_opt(t, 0).single()),
    })
}

fn row_to_provider(row: &rusqlite::Row<'_>) -> rusqlite::Result<ProviderConfig> {
    let last_synced: Option<i64> = row.get(7)?;
    let enabled: i32 = row.get(3)?;
    let auto_push: i32 = row.get(6)?;
    Ok(ProviderConfig {
        id: row.get(0)?,
        kind: row.get(1)?,
        display_name: row.get(2)?,
        enabled: enabled != 0,
        config_json: row.get(4)?,
        min_confidence: row.get(5)?,
        auto_push: auto_push != 0,
        last_synced_at: last_synced.and_then(|t| Utc.timestamp_opt(t, 0).single()),
        keychain_ref: row.get(8)?,
        target_id: row.get(9)?,
        target_label: row.get(10)?,
    })
}

const MIGRATIONS: &str = r#"
CREATE TABLE IF NOT EXISTS tasks (
    id TEXT PRIMARY KEY,
    text TEXT NOT NULL,
    due_hint TEXT,
    source_snippet TEXT,
    captured_at INTEGER NOT NULL,
    status TEXT NOT NULL DEFAULT 'open',
    confidence REAL NOT NULL DEFAULT 1.0
);

CREATE INDEX IF NOT EXISTS idx_tasks_status_captured
    ON tasks(status, captured_at DESC);

CREATE TABLE IF NOT EXISTS providers (
    id TEXT PRIMARY KEY,
    kind TEXT NOT NULL,
    display_name TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 0,
    config_json TEXT NOT NULL DEFAULT '{}',
    min_confidence REAL NOT NULL DEFAULT 0.7,
    auto_push INTEGER NOT NULL DEFAULT 1,
    last_synced_at INTEGER,
    keychain_ref TEXT,
    target_id TEXT,
    target_label TEXT
);

CREATE TABLE IF NOT EXISTS task_destinations (
    id TEXT PRIMARY KEY,
    task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    provider TEXT NOT NULL REFERENCES providers(id) ON DELETE CASCADE,
    external_id TEXT,
    external_url TEXT,
    pushed_at INTEGER,
    last_error TEXT,
    state TEXT NOT NULL DEFAULT 'pending',
    attempts INTEGER NOT NULL DEFAULT 0,
    next_attempt_at INTEGER,
    UNIQUE(task_id, provider)
);

CREATE INDEX IF NOT EXISTS idx_dest_due
    ON task_destinations(state, next_attempt_at);

CREATE TABLE IF NOT EXISTS settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
"#;

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> Db {
        let db = Db::open_in_memory().unwrap();
        db.migrate().unwrap();
        db
    }

    #[test]
    fn insert_get_task() {
        let db = db();
        let id = db
            .insert_task(&NewTask::manual("buy milk"))
            .unwrap();
        let t = db.get_task(&id).unwrap().unwrap();
        assert_eq!(t.text, "buy milk");
        assert_eq!(t.status, TaskStatus::Open);
    }

    #[test]
    fn list_tasks_excludes_done_by_default() {
        let db = db();
        let a = db.insert_task(&NewTask::manual("a")).unwrap();
        let _b = db.insert_task(&NewTask::manual("b")).unwrap();
        db.set_task_status(&a, TaskStatus::Done).unwrap();
        let open = db.list_tasks(false, 100).unwrap();
        assert_eq!(open.len(), 1);
        let all = db.list_tasks(true, 100).unwrap();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn enqueue_dedupes_by_task_provider() {
        let db = db();
        let task_id = db.insert_task(&NewTask::manual("a")).unwrap();
        db.upsert_provider(&ProviderConfig {
            id: "todoist:1".into(),
            kind: "todoist".into(),
            display_name: "Todoist".into(),
            enabled: true,
            config_json: "{}".into(),
            min_confidence: 0.0,
            auto_push: true,
            last_synced_at: None,
            keychain_ref: None,
            target_id: Some("inbox".into()),
            target_label: Some("Inbox".into()),
        })
        .unwrap();
        db.enqueue_push(&task_id, "todoist:1").unwrap();
        db.enqueue_push(&task_id, "todoist:1").unwrap();
        let dests = db.list_destinations_for_task(&task_id).unwrap();
        assert_eq!(dests.len(), 1);
    }

    #[test]
    fn claim_due_jobs_flips_state() {
        let db = db();
        let task_id = db.insert_task(&NewTask::manual("a")).unwrap();
        db.upsert_provider(&ProviderConfig {
            id: "todoist:1".into(),
            kind: "todoist".into(),
            display_name: "Todoist".into(),
            enabled: true,
            config_json: "{}".into(),
            min_confidence: 0.0,
            auto_push: true,
            last_synced_at: None,
            keychain_ref: None,
            target_id: Some("inbox".into()),
            target_label: Some("Inbox".into()),
        })
        .unwrap();
        db.enqueue_push(&task_id, "todoist:1").unwrap();
        let claimed = db.claim_due_jobs(10).unwrap();
        assert_eq!(claimed.len(), 1);
        let again = db.claim_due_jobs(10).unwrap();
        assert!(again.is_empty(), "second claim must skip 'pushing' rows");
    }
}
