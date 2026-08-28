//! Video format/quality lookup via the [`StreamProvider`] seam — `core`
//! decides *when* to look up formats or download a stream, never *how*.

mod client;
mod config;
mod models;
mod provider;

pub use client::StreamClient;
pub use config::{resolve_ytdlp_config, YtDlpConfig};
pub(crate) use models::select_format;
pub use models::{
    FormatFallback, FormatPreference, FormatRequest, FormatSummary, ResolvedFormat, VideoDetail,
    AUTO_AUDIO_ITAG, MP3_SOURCE_ITAG,
};
pub use provider::{BoxFuture, StreamProvider};

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
