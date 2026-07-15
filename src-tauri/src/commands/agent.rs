//! Tauri command handlers for pending agent actions: the approval gate
//! between the MCP server and anything actually happening.
//!
//! The MCP server (a separate process) only ever inserts `Pending` rows
//! into the shared queue database; these commands are the *only* place an
//! agent-requested mutation executes, and only after the user explicitly
//! approved it in the UI. Approval claims the row atomically
//! (`Pending` → `Approved`), so a double-click or a second window can't
//! execute the same action twice; the execution outcome is then recorded
//! back on the row (`Completed`/`Failed`).

use crate::commands::download::{run_batch_guarded, spawn_download};
use crate::state::AppState;
use downloadhub_core::agent::{AgentActionRequest, AgentActionStatus, PendingAgentAction};
use tauri::{AppHandle, State};

/// Everything awaiting the user: `Pending` actions plus `Approved` ones
/// still executing (a `download_all` batch can run for a while). Polled by
/// the frontend, since the writer is another process the app gets no
/// in-process signal from.
#[tauri::command]
pub async fn list_pending_agent_actions(
    state: State<'_, AppState>,
) -> Result<Vec<PendingAgentAction>, String> {
    state
        .queue_store()?
        .list_unresolved_agent_actions()
        .await
        .map_err(|e| e.to_string())
}

/// Declines a pending action without executing anything.
#[tauri::command]
pub async fn reject_agent_action(action_id: i64, state: State<'_, AppState>) -> Result<(), String> {
    state
        .queue_store()?
        .reject_agent_action(action_id)
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Approves a pending action and executes it through the exact same guarded
/// paths the UI's own buttons use (`add_to_queue`'s store insert,
/// `start_download`'s spawn, `download_all`'s batch). Execution failure is
/// recorded on the action (`Failed` + message) *and* returned as this
/// command's error; the agent can submit a fresh request.
///
/// For `DownloadAll` this awaits the whole batch, mirroring the
/// `download_all` command's deliberate await-don't-spawn semantics — the
/// action stays visibly `Approved` (in progress) until the batch ends.
#[tauri::command]
pub async fn approve_agent_action<R: tauri::Runtime>(
    action_id: i64,
    app: AppHandle<R>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let store = state.queue_store()?;
    let action = store
        .approve_agent_action(action_id)
        .await
        .map_err(|e| e.to_string())?;

    let result = match action.request {
        AgentActionRequest::AddToQueue { entry } => store
            .add_entry(entry)
            .await
            .map(|_| ())
            .map_err(|e| e.to_string()),
        AgentActionRequest::StartDownload { queue_id, .. } => {
            spawn_download(&app, &state, queue_id).await
        }
        AgentActionRequest::DownloadAll => run_batch_guarded(&app, &state).await.map(|_| ()),
    };

    match result {
        Ok(()) => store
            .resolve_agent_action(action_id, AgentActionStatus::Completed, None)
            .await
            .map_err(|e| e.to_string()),
        Err(e) => {
            store
                .resolve_agent_action(action_id, AgentActionStatus::Failed, Some(&e))
                .await
                .map_err(|record_err| {
                    format!("{e} (and recording the failure also failed: {record_err})")
                })?;
            Err(e)
        }
    }
}
