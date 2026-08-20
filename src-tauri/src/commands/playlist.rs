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
    let ytdlp_config = state.resolve_ytdlp_config().await;
    enqueue::enqueue_videos(
        &state.stream_client,
        state.queue_store()?,
        &video_ids,
        preference,
        &output_path,
        &ytdlp_config,
    )
    .await
    .map_err(|e| e.to_string())
}
