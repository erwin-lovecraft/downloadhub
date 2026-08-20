//! Video format/quality lookup via `yt-dlp`.

mod client;
mod config;
mod models;

pub use client::StreamClient;
pub use config::{resolve_ytdlp_config, YtDlpConfig};
pub use models::{FormatPreference, FormatSummary, ResolvedFormat, VideoDetail, MP3_SOURCE_ITAG};

#[derive(Debug, thiserror::Error)]
pub enum StreamError {
    #[error("invalid video id or url: {0}")]
    InvalidVideoId(String),
    #[error("video unavailable: {0}")]
    VideoUnavailable(String),
    #[error("no format matched the requested filter")]
    FormatNotFound,
    #[error("no yt-dlp binary was found (set a yt-dlp path in Settings, or put yt-dlp on PATH)")]
    YtDlpNotFound,
    #[error("yt-dlp error: {0}")]
    Other(String),
}

impl From<downloadhub_ytdlp::Error> for StreamError {
    fn from(error: downloadhub_ytdlp::Error) -> Self {
        match error {
            downloadhub_ytdlp::Error::InvalidVideoId(input) => StreamError::InvalidVideoId(input),
            downloadhub_ytdlp::Error::VideoUnavailable(reason) => {
                StreamError::VideoUnavailable(reason)
            }
            downloadhub_ytdlp::Error::FormatNotFound => StreamError::FormatNotFound,
            downloadhub_ytdlp::Error::BinaryNotFound(_) => StreamError::YtDlpNotFound,
            other => StreamError::Other(other.to_string()),
        }
    }
}
