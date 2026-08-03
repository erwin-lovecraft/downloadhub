//! Tauri commands for keyword search and playlist listing.

use crate::state::AppState;
use downloadhub_core::youtube::{VideoSummary, YoutubeClient};
use tauri::State;

const MAX_RESULTS: u32 = 25;

#[tauri::command]
pub async fn search_videos(
    query: String,
    state: State<'_, AppState>,
) -> Result<Vec<VideoSummary>, String> {
    YoutubeClient::new(state.youtube_api_key()?)
        .search_videos(&query, MAX_RESULTS)
        .await
        .map_err(|e| e.to_string())
}

/// Lists a playlist's videos for preview before a bulk import. Accepts
/// either a bare playlist id or a playlist/watch URL.
#[tauri::command]
pub async fn list_playlist_items(
    playlist_url_or_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<VideoSummary>, String> {
    YoutubeClient::new(state.youtube_api_key()?)
        .list_playlist_items(&playlist_url_or_id)
        .await
        .map_err(|e| e.to_string())
}
