//! Tauri command for video format/quality lookup.

use crate::state::AppState;
use downloadhub_core::stream::VideoDetail;
use tauri::State;

#[tauri::command]
pub async fn get_video_formats(
    video_id: String,
    state: State<'_, AppState>,
) -> Result<VideoDetail, String> {
    let ytdlp_config = state.resolve_ytdlp_config().await;
    state
        .stream_client
        .get_video_formats(&video_id, &ytdlp_config)
        .await
        .map_err(|e| e.to_string())
}
