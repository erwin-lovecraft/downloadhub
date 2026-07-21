//! Thin Tauri command handler for bulk playlist import. All resolution/
//! queueing logic lives in `downloadhub_core::enqueue`, shared with the MCP
//! server's add tools; this module just wires it up.

use crate::state::AppState;
use downloadhub_core::enqueue::{self, EnqueueOutcome};
use downloadhub_core::stream::FormatPreference;
use tauri::State;

#[tauri::command]
pub async fn import_playlist_to_queue(
    video_ids: Vec<String>,
    preference: FormatPreference,
    output_path: String,
    state: State<'_, AppState>,
) -> Result<EnqueueOutcome, String> {
    enqueue::enqueue_videos(
        &state.stream_client,
        state.queue_store()?,
        &video_ids,
        preference,
        &output_path,
    )
    .await
    .map_err(|e| e.to_string())
}
