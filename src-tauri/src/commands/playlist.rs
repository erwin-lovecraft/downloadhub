//! Tauri command for bulk playlist import (logic in `downloadhub_core::enqueue`).

use crate::state::AppState;
use downloadhub_core::enqueue::{self, EnqueueOutcome};
use downloadhub_core::stream::FormatPreference;
use downloadhub_core::youtube::YoutubeClient;
use tauri::State;

#[tauri::command]
pub async fn import_playlist_to_queue(
    video_ids: Vec<String>,
    preference: FormatPreference,
    output_path: String,
    state: State<'_, AppState>,
) -> Result<EnqueueOutcome, String> {
    let settings = state.load_settings().await;
    let youtube = state.youtube_api_key.clone().map(YoutubeClient::new);
    enqueue::enqueue_videos(
        youtube.as_ref(),
        state.queue_store()?,
        &video_ids,
        preference,
        &output_path,
        settings.enqueue_itag,
    )
    .await
    .map_err(|e| e.to_string())
}
