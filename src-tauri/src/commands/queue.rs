//! Thin Tauri command handlers for the download queue. All persistence
//! logic lives in `downloadhub_core::queue`; this module just wires it up.
//!
//! Command names (`add_to_queue`, `list_queue`) match the MCP tool names
//! planned for Phase 3 (see `CLAUDE.md`), so the MCP server can expose the
//! same underlying `downloadhub_core::queue` operations without renaming.

use crate::state::AppState;
use downloadhub_core::queue::{NewQueueEntry, QueueEntry};
use tauri::State;

#[tauri::command]
pub async fn add_to_queue(
    video_id: String,
    title: String,
    itag: u32,
    quality_label: Option<String>,
    output_path: String,
    state: State<'_, AppState>,
) -> Result<QueueEntry, String> {
    state
        .queue_store()?
        .add_entry(NewQueueEntry {
            video_id,
            title,
            itag,
            quality_label,
            output_path,
        })
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_queue(state: State<'_, AppState>) -> Result<Vec<QueueEntry>, String> {
    state
        .queue_store()?
        .list_entries()
        .await
        .map_err(|e| e.to_string())
}

/// Removes an entry from the queue. If a download is currently running for
/// it, that task is aborted first — otherwise it would keep writing to a
/// file its queue record no longer exists for.
#[tauri::command]
pub async fn remove_from_queue(queue_id: i64, state: State<'_, AppState>) -> Result<(), String> {
    if let Some(handle) = state
        .running_downloads
        .lock()
        .expect("running downloads mutex poisoned")
        .remove(&queue_id)
    {
        handle.abort();
    }

    state
        .queue_store()?
        .delete_entry(queue_id)
        .await
        .map_err(|e| e.to_string())
}
