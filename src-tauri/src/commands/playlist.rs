//! Tauri command for bulk playlist import (logic in `downloadhub_core::enqueue`).

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
