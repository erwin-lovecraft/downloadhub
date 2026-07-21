//! Thin Tauri command handlers for the download queue. All persistence
//! logic lives in `downloadhub_core::queue`; this module just wires it up.
//!
//! Command names (`add_to_queue`, `list_queue`) match the MCP tool names
//! planned for Phase 3 (see `CLAUDE.md`), so the MCP server can expose the
//! same underlying `downloadhub_core::queue` operations without renaming.

use crate::state::AppState;
use downloadhub_core::enqueue::{self, ReformatOutcome};
use downloadhub_core::queue::{NewQueueEntry, QueueEntry, QueueStatus};
use downloadhub_core::stream::{FormatPreference, ResolvedFormat};
use tauri::State;

#[tauri::command]
pub async fn add_to_queue(
    video_id: String,
    title: String,
    itag: u32,
    quality_label: Option<String>,
    output_path: String,
    // `Option` so existing callers that don't send the field keep working
    // (Tauri rejects a missing required argument).
    convert_to_mp3: Option<bool>,
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
            convert_to_mp3: convert_to_mp3.unwrap_or(false),
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

/// Repoints one entry at an exact format the user picked from that video's
/// own format list, resetting it to `Queued`. The precise counterpart to
/// [`set_queue_entries_quality`], which can only work in preferences
/// because a multi-select spans videos with different itags.
#[tauri::command]
pub async fn set_queue_entry_format(
    queue_id: i64,
    itag: u32,
    quality_label: Option<String>,
    convert_to_mp3: bool,
    state: State<'_, AppState>,
) -> Result<QueueEntry, String> {
    state.ensure_no_batch_running()?;
    let store = state.queue_store()?;

    // Changing the format under a running download would leave the task
    // writing the old stream to a path derived from the new one.
    match store.get_entry(queue_id).await.map_err(|e| e.to_string())? {
        Some(entry) if entry.status == QueueStatus::Downloading => {
            return Err("This entry is downloading; cancel it before changing its format.".into());
        }
        Some(_) => {}
        None => return Err("That queue entry no longer exists.".to_string()),
    }

    store
        .set_format(
            queue_id,
            &ResolvedFormat {
                itag,
                quality_label,
                convert_to_mp3,
            },
        )
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "That queue entry no longer exists.".to_string())
}

/// Re-resolves several entries against a single quality preference (the
/// "update all selected" path). Each video's formats are looked up
/// individually, so this makes one network round-trip per entry; entries
/// that fail to resolve are reported in `skipped` rather than sinking the
/// whole update.
#[tauri::command]
pub async fn set_queue_entries_quality(
    queue_ids: Vec<i64>,
    preference: FormatPreference,
    state: State<'_, AppState>,
) -> Result<ReformatOutcome, String> {
    state.ensure_no_batch_running()?;
    enqueue::reformat_entries(
        &state.stream_client,
        state.queue_store()?,
        &queue_ids,
        preference,
    )
    .await
    .map_err(|e| e.to_string())
}

/// Removes an entry from the queue. If a download is currently running for
/// it, that task is aborted first — otherwise it would keep writing to a
/// file its queue record no longer exists for.
#[tauri::command]
pub async fn remove_from_queue(queue_id: i64, state: State<'_, AppState>) -> Result<(), String> {
    state.ensure_no_batch_running()?;

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
