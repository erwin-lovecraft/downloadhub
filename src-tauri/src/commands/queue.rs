//! Thin Tauri command handlers for the download queue. All persistence
//! logic lives in `downloadhub_core::queue`; this module just wires it up.
//!
//! Command names (`add_to_queue`, `list_queue`) match the MCP tool names
//! planned for Phase 3 (see `CLAUDE.md`), so the MCP server can expose the
//! same underlying `downloadhub_core::queue` operations without renaming.

use crate::state::AppState;
use downloadhub_core::queue::{NewQueueEntry, QueueEntry};
use tauri::State;

fn queue_store(state: &AppState) -> Result<&downloadhub_core::queue::QueueStore, String> {
    state.queue_store.as_ref().ok_or_else(|| {
        "The download queue database is not available (couldn't be opened at startup — check the app's log output).".to_string()
    })
}

#[tauri::command]
pub async fn add_to_queue(
    video_id: String,
    title: String,
    itag: u32,
    quality_label: Option<String>,
    output_path: String,
    state: State<'_, AppState>,
) -> Result<QueueEntry, String> {
    queue_store(&state)?
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
    queue_store(&state)?
        .list_entries()
        .await
        .map_err(|e| e.to_string())
}
