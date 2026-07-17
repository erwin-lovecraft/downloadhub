//! Video format/quality lookup via `y7dl`.
//!
//! - [`client`]: [`StreamClient`], the `y7dl::Client` wrapper
//! - [`models`]: the IPC-facing DTOs (`VideoDetail`, `FormatSummary`) and
//!   the [`FormatPreference`] selection logic

mod client;
mod models;

pub use client::StreamClient;
pub use models::{FormatPreference, FormatSummary, VideoDetail};

#[derive(Debug, thiserror::Error)]
pub enum StreamError {
    #[error("invalid video id or url: {0}")]
    InvalidVideoId(String),
    #[error("video unavailable ({status}): {}", reason.as_deref().unwrap_or("no reason given"))]
    VideoUnavailable {
        status: String,
        reason: Option<String>,
    },
    #[error("no format matched the requested filter")]
    FormatNotFound,
    #[error("y7dl error: {0}")]
    Other(String),
}

impl From<y7dl::Error> for StreamError {
    fn from(error: y7dl::Error) -> Self {
        match error {
            y7dl::Error::InvalidVideoId(input) => StreamError::InvalidVideoId(input),
            y7dl::Error::VideoUnavailable { status, reason } => {
                StreamError::VideoUnavailable { status, reason }
            }
            y7dl::Error::FormatNotFound => StreamError::FormatNotFound,
            other => StreamError::Other(other.to_string()),
        }
    }
}
