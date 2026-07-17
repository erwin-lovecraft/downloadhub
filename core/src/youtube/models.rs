//! The public YouTube models exposed to callers (and across the Tauri IPC
//! boundary), shaped from the raw API responses in `response`.

#[derive(Debug, Clone, serde::Serialize)]
pub struct VideoSummary {
    pub video_id: String,
    pub title: String,
    pub channel_title: String,
    pub thumbnail_url: Option<String>,
    pub published_at: String,
    /// `None` if `videos.list` didn't return contentDetails for this id
    /// (e.g. a live stream in progress) or its duration couldn't be parsed.
    pub duration_seconds: Option<u64>,
}
