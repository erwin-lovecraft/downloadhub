//! The agent-action domain types: what an agent asked for, what state the
//! request is in, and the errors those transitions can produce.

use serde::{Deserialize, Serialize};

use crate::queue::{NewQueueEntry, QueueError};

#[derive(Debug, thiserror::Error)]
pub enum AgentActionError {
    #[error(transparent)]
    Queue(#[from] QueueError),
    #[error("agent action {0} not found")]
    NotFound(i64),
    #[error("agent action {0} was already {1}")]
    AlreadyResolved(i64, &'static str),
    #[error("corrupt agent action payload: {0}")]
    Payload(#[from] serde_json::Error),
}

impl From<rusqlite::Error> for AgentActionError {
    fn from(e: rusqlite::Error) -> Self {
        AgentActionError::Queue(QueueError::from(e))
    }
}

impl From<tokio::task::JoinError> for AgentActionError {
    fn from(e: tokio::task::JoinError) -> Self {
        AgentActionError::Queue(QueueError::from(e))
    }
}

/// What the agent asked for. Stored as a JSON payload on the row; the
/// desktop app deserializes it back to execute the operation on approval.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AgentActionRequest {
    /// Add one resolved entry to the download queue. The MCP server fills
    /// `entry` from an authoritative format lookup (title, quality label),
    /// not from agent-supplied strings, so the approval prompt describes
    /// the real video.
    AddToQueue { entry: NewQueueEntry },
    /// Start downloading one existing queue entry. `title` is a display
    /// snapshot taken from the queue row at request time so the approval
    /// prompt can name it without a join.
    StartDownload { queue_id: i64, title: String },
    /// Download every currently-queued entry, sequentially.
    DownloadAll,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentActionStatus {
    /// Awaiting the user's decision in the desktop app.
    Pending,
    /// Approved; the desktop app is executing it right now.
    Approved,
    /// The user declined it.
    Rejected,
    /// Approved and executed (for downloads: successfully *started*).
    Completed,
    /// Approved but execution failed — see `error_message`.
    Failed,
}

impl AgentActionStatus {
    /// The value persisted in the `status` column.
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            AgentActionStatus::Pending => "pending",
            AgentActionStatus::Approved => "approved",
            AgentActionStatus::Rejected => "rejected",
            AgentActionStatus::Completed => "completed",
            AgentActionStatus::Failed => "failed",
        }
    }

    pub(crate) fn from_str(s: &str) -> Self {
        match s {
            "approved" => AgentActionStatus::Approved,
            "rejected" => AgentActionStatus::Rejected,
            "completed" => AgentActionStatus::Completed,
            "failed" => AgentActionStatus::Failed,
            _ => AgentActionStatus::Pending,
        }
    }
}

/// A persisted agent action, pending or resolved.
#[derive(Debug, Clone, Serialize)]
pub struct PendingAgentAction {
    pub id: i64,
    pub request: AgentActionRequest,
    pub status: AgentActionStatus,
    /// The MCP client's self-reported name (e.g. "claude-desktop"), for
    /// display only — it is not an authenticated identity.
    pub requested_by: Option<String>,
    pub error_message: Option<String>,
    /// Unix timestamp (seconds) the action was requested.
    pub created_at: i64,
    /// Unix timestamp (seconds) the action left `Pending`, if it has.
    pub resolved_at: Option<i64>,
}
