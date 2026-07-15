//! Thin Tauri command handler for video format/quality lookup. All `y7dl`
//! logic lives in `downloadhub_core::stream`; this module just wires it up.

use crate::state::AppState;
use downloadhub_core::stream::VideoDetail;
use tauri::State;

#[tauri::command]
pub async fn get_video_formats(
    video_id: String,
    state: State<'_, AppState>,
) -> Result<VideoDetail, String> {
    state
        .stream_client
        .get_video_formats(&video_id)
        .await
        .map_err(|e| e.to_string())
}
