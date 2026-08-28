//! Video format/quality lookup via the [`StreamProvider`] seam — `core`
//! decides *when* to look up formats or download a stream, never *how*.

mod client;
mod config;
mod cookies;
mod models;
mod provider;

pub use client::StreamClient;
pub use config::{resolve_ytdlp_config, YtDlpConfig};
pub use cookies::{inspect_cookie_file, CookieFileReport};
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
    #[error(
        "YouTube is requiring sign-in verification (bot check). Point Settings at a cookies.txt exported while signed in, and use \"Test cookies\" there to check it."
    )]
    BotCheckRequired,
    #[error("no yt-dlp binary was found (set a yt-dlp path in Settings, or put yt-dlp on PATH)")]
    YtDlpNotFound,
    #[error("yt-dlp error: {0}")]
    Other(String),
}
