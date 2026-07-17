//! The async facade for agent actions, as extra methods on [`QueueStore`]
//! (their table lives in the same database file, created in the queue's
//! schema setup). Mirrors `queue::store`: lock + `spawn_blocking` here, all
//! SQL in [`AgentActionRepository`].

use crate::queue::QueueStore;

use super::action::{AgentActionError, AgentActionRequest, AgentActionStatus, PendingAgentAction};
use super::repository::AgentActionRepository;

impl QueueStore {
    /// Records a new action in `Pending` status and returns it as persisted.
    /// Called by the MCP server; the desktop app never inserts here.
    pub async fn add_agent_action(
        &self,
        request: AgentActionRequest,
        requested_by: Option<String>,
    ) -> Result<PendingAgentAction, AgentActionError> {
        self.with_agent_repository(move |repo| repo.insert(request, requested_by))
            .await
    }

    /// Looks up a single action by id, if it exists.
    pub async fn get_agent_action(
        &self,
        id: i64,
    ) -> Result<Option<PendingAgentAction>, AgentActionError> {
        self.with_agent_repository(move |repo| repo.get(id)).await
    }

    /// Lists every not-yet-resolved action (`Pending` or `Approved`,
    /// i.e. everything the desktop app should be surfacing), oldest first
    /// so approvals are presented in the order the agent asked.
    pub async fn list_unresolved_agent_actions(
        &self,
    ) -> Result<Vec<PendingAgentAction>, AgentActionError> {
        self.with_agent_repository(|repo| repo.list_unresolved())
            .await
    }

    /// Atomically transitions a `Pending` action to `Approved`, returning
    /// the action so the caller can execute it. Errors if the action is
    /// missing or was already resolved (e.g. approved from another window
    /// in the race window), so an action can never execute twice.
    pub async fn approve_agent_action(
        &self,
        id: i64,
    ) -> Result<PendingAgentAction, AgentActionError> {
        self.with_agent_repository(move |repo| repo.claim_pending(id, AgentActionStatus::Approved))
            .await
    }

    /// Atomically transitions a `Pending` action to `Rejected`. Same
    /// missing/already-resolved errors as [`Self::approve_agent_action`].
    pub async fn reject_agent_action(
        &self,
        id: i64,
    ) -> Result<PendingAgentAction, AgentActionError> {
        self.with_agent_repository(move |repo| repo.claim_pending(id, AgentActionStatus::Rejected))
            .await
    }

    /// Records the execution outcome of an `Approved` action.
    pub async fn resolve_agent_action(
        &self,
        id: i64,
        outcome: AgentActionStatus,
        error_message: Option<&str>,
    ) -> Result<(), AgentActionError> {
        let error_message = error_message.map(str::to_string);
        self.with_agent_repository(move |repo| repo.record_outcome(id, outcome, error_message))
            .await
    }

    /// Runs one agent-action repository operation on a blocking thread with
    /// the connection locked for its duration.
    async fn with_agent_repository<T, F>(&self, op: F) -> Result<T, AgentActionError>
    where
        T: Send + 'static,
        F: FnOnce(AgentActionRepository<'_>) -> Result<T, AgentActionError> + Send + 'static,
    {
        let conn = self.conn.clone();
        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().expect("queue db mutex poisoned");
            op(AgentActionRepository::new(&conn))
        })
        .await?
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::queue::NewQueueEntry;

    fn add_request() -> AgentActionRequest {
        AgentActionRequest::AddToQueue {
            entry: NewQueueEntry {
                video_id: "abc123".to_string(),
                title: "Test Video".to_string(),
                itag: 18,
                quality_label: Some("360p".to_string()),
                output_path: "/tmp/downloads".to_string(),
                convert_to_mp3: false,
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
