//! All SQL touching the `pending_agent_actions` table, and nothing else.
//!
//! Same shape as `queue::repository`: [`AgentActionRepository`] borrows an
//! already-locked connection and is constructed fresh per operation by the
//! store facade; it holds no state of its own.

use rusqlite::{Connection, Row};

use super::action::{AgentActionError, AgentActionRequest, AgentActionStatus, PendingAgentAction};
use crate::queue::now_unix;

const SELECT_COLUMNS: &str =
    "id, payload, status, requested_by, error_message, created_at, resolved_at";

pub(crate) struct AgentActionRepository<'c> {
    conn: &'c Connection,
}

impl<'c> AgentActionRepository<'c> {
    pub(crate) fn new(conn: &'c Connection) -> Self {
        Self { conn }
    }

    /// Inserts `request` in `Pending` status and returns it as persisted.
    pub(crate) fn insert(
        &self,
        request: AgentActionRequest,
        requested_by: Option<String>,
    ) -> Result<PendingAgentAction, AgentActionError> {
        let payload = serde_json::to_string(&request)?;
        let created_at = now_unix();
        self.conn.execute(
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
            id: self.conn.last_insert_rowid(),
            request,
            status: AgentActionStatus::Pending,
            requested_by,
            error_message: None,
            created_at,
            resolved_at: None,
        })
    }

    pub(crate) fn get(&self, id: i64) -> Result<Option<PendingAgentAction>, AgentActionError> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {SELECT_COLUMNS} FROM pending_agent_actions WHERE id = ?1"
        ))?;
        let mut rows = stmt.query(rusqlite::params![id])?;
        match rows.next()? {
            Some(row) => Ok(Some(row_to_action(row)?)),
            None => Ok(None),
        }
    }

    /// Every not-yet-resolved action (`Pending` or `Approved`), oldest
    /// first so approvals are presented in the order the agent asked.
    pub(crate) fn list_unresolved(&self) -> Result<Vec<PendingAgentAction>, AgentActionError> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {SELECT_COLUMNS} FROM pending_agent_actions
             WHERE status IN ('pending', 'approved')
             ORDER BY created_at ASC, id ASC"
        ))?;
        let rows = stmt.query_map([], row_to_action)?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(AgentActionError::from)
    }

    /// Atomically transitions a `Pending` action to `to` (the `WHERE
    /// status = 'pending'` guard is what makes a concurrent second claim
    /// lose), returning the updated action. Errors if the action is missing
    /// or was already resolved, so an action can never execute twice.
    pub(crate) fn claim_pending(
        &self,
        id: i64,
        to: AgentActionStatus,
    ) -> Result<PendingAgentAction, AgentActionError> {
        let updated = self.conn.execute(
            "UPDATE pending_agent_actions
             SET status = ?1, resolved_at = ?2
             WHERE id = ?3 AND status = 'pending'",
            rusqlite::params![to.as_str(), now_unix(), id],
        )?;
        if updated == 0 {
            // Distinguish "no such action" from "already resolved" for a
            // clearer user-facing error.
            let mut stmt = self
                .conn
                .prepare("SELECT status FROM pending_agent_actions WHERE id = ?1")?;
            let mut rows = stmt.query(rusqlite::params![id])?;
            return match rows.next()? {
                Some(row) => {
                    let status = AgentActionStatus::from_str(&row.get::<_, String>(0)?);
                    Err(AgentActionError::AlreadyResolved(id, status.as_str()))
                }
                None => Err(AgentActionError::NotFound(id)),
            };
        }

        match self.get(id)? {
            Some(action) => Ok(action),
            None => Err(AgentActionError::NotFound(id)),
        }
    }

    /// Records the execution outcome of an `Approved` action.
    pub(crate) fn record_outcome(
        &self,
        id: i64,
        outcome: AgentActionStatus,
        error_message: Option<String>,
    ) -> Result<(), AgentActionError> {
        self.conn.execute(
            "UPDATE pending_agent_actions
             SET status = ?1, error_message = ?2, resolved_at = ?3
             WHERE id = ?4",
            rusqlite::params![outcome.as_str(), error_message, now_unix(), id],
        )?;
        Ok(())
    }
}

fn row_to_action(row: &Row<'_>) -> Result<PendingAgentAction, rusqlite::Error> {
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
