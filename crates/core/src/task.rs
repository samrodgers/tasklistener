use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Open,
    Done,
    Dismissed,
}

impl TaskStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            TaskStatus::Open => "open",
            TaskStatus::Done => "done",
            TaskStatus::Dismissed => "dismissed",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "open" => TaskStatus::Open,
            "done" => TaskStatus::Done,
            "dismissed" => TaskStatus::Dismissed,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub text: String,
    pub due_hint: Option<String>,
    pub source_snippet: Option<String>,
    pub captured_at: DateTime<Utc>,
    pub status: TaskStatus,
    pub confidence: f32,
}

#[derive(Debug, Clone)]
pub struct NewTask {
    pub text: String,
    pub due_hint: Option<String>,
    pub source_snippet: Option<String>,
    pub confidence: f32,
}

impl NewTask {
    pub fn manual(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            due_hint: None,
            source_snippet: Some("manual entry".to_string()),
            confidence: 1.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DestinationState {
    Pending,
    Pushing,
    Pushed,
    Failed,
    DeadLetter,
}

impl DestinationState {
    pub fn as_str(&self) -> &'static str {
        match self {
            DestinationState::Pending => "pending",
            DestinationState::Pushing => "pushing",
            DestinationState::Pushed => "pushed",
            DestinationState::Failed => "failed",
            DestinationState::DeadLetter => "dead_letter",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "pending" => DestinationState::Pending,
            "pushing" => DestinationState::Pushing,
            "pushed" => DestinationState::Pushed,
            "failed" => DestinationState::Failed,
            "dead_letter" => DestinationState::DeadLetter,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskDestinationRow {
    pub id: String,
    pub task_id: String,
    pub provider: String,
    pub external_id: Option<String>,
    pub external_url: Option<String>,
    pub pushed_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    pub state: DestinationState,
    pub attempts: i32,
    pub next_attempt_at: Option<DateTime<Utc>>,
}
