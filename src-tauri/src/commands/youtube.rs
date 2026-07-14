//! Thin Tauri command handler for keyword search. All YouTube Data API
//! logic lives in `downloadhub_core::youtube`; this module just wires it up.

use crate::state::AppState;
use downloadhub_core::youtube::{VideoSummary, YoutubeClient};
use tauri::State;

const MAX_RESULTS: u32 = 25;

#[tauri::command]
pub async fn search_videos(
    query: String,
    state: State<'_, AppState>,
) -> Result<Vec<VideoSummary>, String> {
    let api_key = state.youtube_api_key.clone().ok_or_else(|| {
        "YouTube search is not configured. Set YOUTUBE_API_KEY (see README) and restart the app."
            .to_string()
    })?;

    YoutubeClient::new(api_key)
        .search_videos(&query, MAX_RESULTS)
        .await
        .map_err(|e| e.to_string())
}
