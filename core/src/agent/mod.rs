//! Pending agent actions: queue-mutating requests made by an external AI
//! agent (via the `mcp-server` binary) that must be explicitly approved by
//! the user in the running desktop app before they execute.
//!
//! The MCP server never mutates the queue or starts a download itself — it
//! only inserts rows here (status `pending`). The desktop app polls this
//! table, shows each pending action to the user, and on approval executes
//! the underlying operation itself, recording the outcome back on the row.
//! Rows live in the same SQLite database as the queue (`QueueStore`), which
//! both processes already share — see `docs/ARCHITECTURE.md`.

use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::queue::{NewQueueEntry, QueueError, QueueStore};

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
    fn as_str(self) -> &'static str {
        match self {
            AgentActionStatus::Pending => "pending",
            AgentActionStatus::Approved => "approved",
            AgentActionStatus::Rejected => "rejected",
            AgentActionStatus::Completed => "completed",
            AgentActionStatus::Failed => "failed",
        }
    }

    fn from_str(s: &str) -> Self {
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

fn row_to_action(row: &rusqlite::Row<'_>) -> Result<PendingAgentAction, rusqlite::Error> {
    let payload: String = row.get(1)?;
    let request = serde_json::from_str(&payload).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(1, rusqlite::types::Type::Text, Box::new(e))
    })?;
    Ok(PendingAgentAction {
        id: row.get(0)?,
        request,
        status: AgentActionStatus::from_str(&row.get::<_, String>(2)?),
        requested_by: row.get(3)?,
        error_message: row.get(4)?,
        created_at: row.get(5)?,
        resolved_at: row.get(6)?,
    })
}

const SELECT_COLUMNS: &str =
    "id, payload, status, requested_by, error_message, created_at, resolved_at";

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

/// Agent-action persistence lives on `QueueStore` because the actions share
/// the queue's database file (and therefore its connection) — the table is
/// created in `QueueStore`'s schema setup.
impl QueueStore {
    /// Records a new action in `Pending` status and returns it as persisted.
    /// Called by the MCP server; the desktop app never inserts here.
    pub async fn add_agent_action(
        &self,
        request: AgentActionRequest,
        requested_by: Option<String>,
    ) -> Result<PendingAgentAction, AgentActionError> {
        let conn = self.conn.clone();
        tokio::task::spawn_blocking(move || {
            let payload = serde_json::to_string(&request)?;
            let created_at = now_unix();
            let conn = conn.lock().expect("queue db mutex poisoned");
            conn.execute(
                "INSERT INTO pending_agent_actions
                    (payload, status, requested_by, error_message, created_at, resolved_at)
                 VALUES (?1, ?2, ?3, NULL, ?4, NULL)",
                rusqlite::params![
                    payload,
                    AgentActionStatus::Pending.as_str(),
                    requested_by,
                    created_at,
                ],
            )?;
            Ok(PendingAgentAction {
                id: conn.last_insert_rowid(),
                request,
                status: AgentActionStatus::Pending,
                requested_by,
                error_message: None,
                created_at,
                resolved_at: None,
            })
        })
        .await?
    }

    /// Looks up a single action by id, if it exists.
    pub async fn get_agent_action(
        &self,
        id: i64,
    ) -> Result<Option<PendingAgentAction>, AgentActionError> {
        let conn = self.conn.clone();
        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().expect("queue db mutex poisoned");
            let mut stmt = conn.prepare(&format!(
                "SELECT {SELECT_COLUMNS} FROM pending_agent_actions WHERE id = ?1"
            ))?;
            let mut rows = stmt.query(rusqlite::params![id])?;
            match rows.next()? {
                Some(row) => Ok(Some(row_to_action(row)?)),
                None => Ok(None),
            }
        })
        .await?
    }

    /// Lists every not-yet-resolved action (`Pending` or `Approved`,
    /// i.e. everything the desktop app should be surfacing), oldest first
    /// so approvals are presented in the order the agent asked.
    pub async fn list_unresolved_agent_actions(
        &self,
    ) -> Result<Vec<PendingAgentAction>, AgentActionError> {
        let conn = self.conn.clone();
        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().expect("queue db mutex poisoned");
            let mut stmt = conn.prepare(&format!(
                "SELECT {SELECT_COLUMNS} FROM pending_agent_actions
                 WHERE status IN ('pending', 'approved')
                 ORDER BY created_at ASC, id ASC"
            ))?;
            let rows = stmt.query_map([], row_to_action)?;
            rows.collect::<Result<Vec<_>, _>>()
                .map_err(AgentActionError::from)
        })
        .await?
    }

    /// Atomically transitions a `Pending` action to `Approved`, returning
    /// the action so the caller can execute it. Errors if the action is
    /// missing or was already resolved (e.g. approved from another window
    /// in the race window), so an action can never execute twice.
    pub async fn approve_agent_action(
        &self,
        id: i64,
    ) -> Result<PendingAgentAction, AgentActionError> {
        self.transition_pending(id, AgentActionStatus::Approved)
            .await
    }

    /// Atomically transitions a `Pending` action to `Rejected`. Same
    /// missing/already-resolved errors as [`Self::approve_agent_action`].
    pub async fn reject_agent_action(
        &self,
        id: i64,
    ) -> Result<PendingAgentAction, AgentActionError> {
        self.transition_pending(id, AgentActionStatus::Rejected)
            .await
    }

    /// Records the execution outcome of an `Approved` action.
    pub async fn resolve_agent_action(
        &self,
        id: i64,
        outcome: AgentActionStatus,
        error_message: Option<&str>,
    ) -> Result<(), AgentActionError> {
        let conn = self.conn.clone();
        let error_message = error_message.map(str::to_string);
        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().expect("queue db mutex poisoned");
            conn.execute(
                "UPDATE pending_agent_actions
                 SET status = ?1, error_message = ?2, resolved_at = ?3
                 WHERE id = ?4",
                rusqlite::params![outcome.as_str(), error_message, now_unix(), id],
            )?;
            Ok(())
        })
        .await?
    }

    async fn transition_pending(
        &self,
        id: i64,
        to: AgentActionStatus,
    ) -> Result<PendingAgentAction, AgentActionError> {
        let conn = self.conn.clone();
        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().expect("queue db mutex poisoned");
            let resolved_at = now_unix();
            let updated = conn.execute(
                "UPDATE pending_agent_actions
                 SET status = ?1, resolved_at = ?2
                 WHERE id = ?3 AND status = 'pending'",
                rusqlite::params![to.as_str(), resolved_at, id],
            )?;
            if updated == 0 {
                // Distinguish "no such action" from "already resolved" for
                // a clearer user-facing error.
                let mut stmt =
                    conn.prepare("SELECT status FROM pending_agent_actions WHERE id = ?1")?;
                let mut rows = stmt.query(rusqlite::params![id])?;
                return match rows.next()? {
                    Some(row) => {
                        let status = AgentActionStatus::from_str(&row.get::<_, String>(0)?);
                        Err(AgentActionError::AlreadyResolved(id, status.as_str()))
                    }
                    None => Err(AgentActionError::NotFound(id)),
                };
            }

            let mut stmt = conn.prepare(&format!(
                "SELECT {SELECT_COLUMNS} FROM pending_agent_actions WHERE id = ?1"
            ))?;
            let mut rows = stmt.query(rusqlite::params![id])?;
            match rows.next()? {
                Some(row) => Ok(row_to_action(row)?),
                None => Err(AgentActionError::NotFound(id)),
            }
        })
        .await?
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn add_request() -> AgentActionRequest {
        AgentActionRequest::AddToQueue {
            entry: NewQueueEntry {
                video_id: "abc123".to_string(),
                title: "Test Video".to_string(),
                itag: 18,
                quality_label: Some("360p".to_string()),
                output_path: "/tmp/downloads".to_string(),
            },
        }
    }

    #[tokio::test]
    async fn add_then_list_returns_pending_action_with_payload_roundtripped() {
        let store = QueueStore::open_in_memory().unwrap();
        let added = store
            .add_agent_action(add_request(), Some("test-agent".to_string()))
            .await
            .unwrap();
        assert_eq!(added.status, AgentActionStatus::Pending);
        assert!(added.id > 0);

        let listed = store.list_unresolved_agent_actions().await.unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].request, add_request());
        assert_eq!(listed[0].requested_by.as_deref(), Some("test-agent"));
    }

    #[tokio::test]
    async fn approve_transitions_pending_to_approved_exactly_once() {
        let store = QueueStore::open_in_memory().unwrap();
        let added = store.add_agent_action(add_request(), None).await.unwrap();

        let approved = store.approve_agent_action(added.id).await.unwrap();
        assert_eq!(approved.status, AgentActionStatus::Approved);
        assert!(approved.resolved_at.is_some());

        // A second approval (double-click, second window) must fail rather
        // than let the action execute twice.
        let err = store.approve_agent_action(added.id).await.unwrap_err();
        assert!(matches!(
            err,
            AgentActionError::AlreadyResolved(_, "approved")
        ));
    }

    #[tokio::test]
    async fn reject_transitions_pending_to_rejected_and_hides_it_from_unresolved() {
        let store = QueueStore::open_in_memory().unwrap();
        let added = store.add_agent_action(add_request(), None).await.unwrap();

        let rejected = store.reject_agent_action(added.id).await.unwrap();
        assert_eq!(rejected.status, AgentActionStatus::Rejected);

        assert!(store
            .list_unresolved_agent_actions()
            .await
            .unwrap()
            .is_empty());

        // A rejected action can't be approved afterwards.
        let err = store.approve_agent_action(added.id).await.unwrap_err();
        assert!(matches!(
            err,
            AgentActionError::AlreadyResolved(_, "rejected")
        ));
    }

    #[tokio::test]
    async fn resolve_records_execution_outcome() {
        let store = QueueStore::open_in_memory().unwrap();
        let added = store
            .add_agent_action(AgentActionRequest::DownloadAll, None)
            .await
            .unwrap();
        store.approve_agent_action(added.id).await.unwrap();

        store
            .resolve_agent_action(added.id, AgentActionStatus::Failed, Some("boom"))
            .await
            .unwrap();

        let action = store.get_agent_action(added.id).await.unwrap().unwrap();
        assert_eq!(action.status, AgentActionStatus::Failed);
        assert_eq!(action.error_message.as_deref(), Some("boom"));
        assert!(store
            .list_unresolved_agent_actions()
            .await
            .unwrap()
            .is_empty());
    }

    #[tokio::test]
    async fn approving_a_missing_action_reports_not_found() {
        let store = QueueStore::open_in_memory().unwrap();
        let err = store.approve_agent_action(999).await.unwrap_err();
        assert!(matches!(err, AgentActionError::NotFound(999)));
    }

    #[tokio::test]
    async fn unresolved_actions_list_oldest_first_and_include_approved() {
        let store = QueueStore::open_in_memory().unwrap();
        let first = store.add_agent_action(add_request(), None).await.unwrap();
        let second = store
            .add_agent_action(AgentActionRequest::DownloadAll, None)
            .await
            .unwrap();
        store.approve_agent_action(first.id).await.unwrap();

        let listed = store.list_unresolved_agent_actions().await.unwrap();
        assert_eq!(listed.len(), 2);
        assert_eq!(listed[0].id, first.id);
        assert_eq!(listed[0].status, AgentActionStatus::Approved);
        assert_eq!(listed[1].id, second.id);
    }
}
