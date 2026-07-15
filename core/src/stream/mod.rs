//! Video format/quality lookup via `y7dl`.
//!
//! `y7dl::Video`/`Format` don't derive `Serialize` (they're built from a
//! `Deserialize`d player response), so this module maps them into small DTOs
//! that cross the Tauri IPC boundary as plain JSON, mirroring how
//! `core::youtube` shapes `VideoSummary` from the raw API response.

use serde::Serialize;

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

/// One downloadable stream variant (progressive or adaptive).
#[derive(Debug, Clone, Serialize)]
pub struct FormatSummary {
    pub itag: u32,
    pub mime_type: String,
    /// Coarse quality bucket, e.g. `tiny`, `hd720`.
    pub quality: Option<String>,
    /// Human label for video formats, e.g. `720p`, `1080p60`.
    pub quality_label: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub fps: Option<u32>,
    pub bitrate: Option<u64>,
    pub content_length_bytes: Option<u64>,
    pub has_video: bool,
    pub has_audio: bool,
}

impl From<y7dl::Format> for FormatSummary {
    fn from(format: y7dl::Format) -> Self {
        Self {
            itag: format.itag,
            mime_type: format.mime_type.clone(),
            quality: format.quality.clone(),
            quality_label: format.quality_label.clone(),
            width: format.width,
            height: format.height,
            fps: format.fps,
            bitrate: format.bitrate,
            content_length_bytes: format.content_length(),
            has_video: format.is_video(),
            has_audio: format.has_audio(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct VideoDetail {
    pub video_id: String,
    pub title: String,
    pub author: String,
    pub duration_seconds: u64,
    /// Progressive formats first, then adaptive (video-only/audio-only) ones
    /// — the order `y7dl::Video::formats` already returns them in.
    pub formats: Vec<FormatSummary>,
}

/// A quality shortcut for bulk operations (playlist import) that can't
/// reasonably ask the user to pick an exact itag per video, since the
/// available itags vary video to video. Also used as the persisted
/// default quality in `core::settings`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FormatPreference {
    /// Highest-resolution format that has both video and audio in one
    /// stream. Deliberately doesn't fall back to a video-only format if no
    /// progressive one exists — silently producing a video with no sound
    /// would violate what the user asked for.
    #[default]
    BestProgressive,
    /// Highest-bitrate audio-only format.
    BestAudioOnly,
}

/// Picks the format matching `preference` from an already-fetched list, or
/// `None` if nothing qualifies. Pure/no I/O so it's unit-testable without a
/// real video lookup.
fn select_format(
    formats: &[FormatSummary],
    preference: FormatPreference,
) -> Option<&FormatSummary> {
    match preference {
        FormatPreference::BestProgressive => formats
            .iter()
            .filter(|f| f.has_video && f.has_audio)
            .max_by_key(|f| f.height.unwrap_or(0)),
        FormatPreference::BestAudioOnly => formats
            .iter()
            .filter(|f| f.has_audio && !f.has_video)
            .max_by_key(|f| f.bitrate.unwrap_or(0)),
    }
}

/// Thin wrapper over `y7dl::Client`. Reuse one instance across lookups: the
/// underlying client caches parsed player JS and pools HTTP connections.
pub struct StreamClient(y7dl::Client);

impl Default for StreamClient {
    fn default() -> Self {
        Self::new()
    }
}

impl StreamClient {
    pub fn new() -> Self {
        Self(y7dl::Client::new())
    }

    /// Fetches metadata and the full available format/quality list for a
    /// video URL or bare 11-character ID.
    pub async fn get_video_formats(&self, url_or_id: &str) -> Result<VideoDetail, StreamError> {
        let video = self.fetch_video(url_or_id).await?;
        Ok(VideoDetail {
            video_id: video.id,
            title: video.title,
            author: video.author,
            duration_seconds: video.duration.as_secs(),
            formats: video.formats.into_iter().map(FormatSummary::from).collect(),
        })
    }

    /// Fetches a video's formats and picks the one matching `preference`.
    /// Used for playlist import, where asking the user to pick an exact
    /// itag per video isn't practical. Returns [`StreamError::FormatNotFound`]
    /// if nothing matches (e.g. `BestProgressive` on a video YouTube only
    /// serves as separate video/audio DASH streams).
    pub async fn resolve_preferred_format(
        &self,
        url_or_id: &str,
        preference: FormatPreference,
    ) -> Result<(VideoDetail, FormatSummary), StreamError> {
        let detail = self.get_video_formats(url_or_id).await?;
        let format = select_format(&detail.formats, preference)
            .cloned()
            .ok_or(StreamError::FormatNotFound)?;
        Ok((detail, format))
    }

    /// Fetches raw `y7dl` video metadata (including formats with their
    /// stream URLs) for a video URL or bare ID. Exposed for `core::download`,
    /// which needs the raw `y7dl::Format` (stream URL, mime type) rather than
    /// the IPC-facing `FormatSummary` DTO.
    pub async fn fetch_video(&self, url_or_id: &str) -> Result<y7dl::Video, StreamError> {
        Ok(self.0.get_video(url_or_id).await?)
    }

    /// Downloads `format`'s stream (resolved from a `fetch_video` call, so
    /// its stream URL is fresh) into `dest` using ranged chunk requests.
    /// Returns the number of bytes written.
    pub async fn download<W>(
        &self,
        video: &y7dl::Video,
        format: &y7dl::Format,
        dest: &mut W,
    ) -> Result<u64, StreamError>
    where
        W: tokio::io::AsyncWrite + Unpin + ?Sized,
    {
        Ok(self.0.download(video, format, dest).await?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn format(
        itag: u32,
        has_video: bool,
        has_audio: bool,
        height: Option<u32>,
        bitrate: Option<u64>,
    ) -> FormatSummary {
        FormatSummary {
            itag,
            mime_type: "video/mp4".to_string(),
            quality: None,
            quality_label: None,
            width: None,
            height,
            fps: None,
            bitrate,
            content_length_bytes: None,
            has_video,
            has_audio,
        }
    }

    #[test]
    fn best_progressive_picks_highest_resolution_progressive_format() {
        let formats = vec![
            format(18, true, true, Some(360), None),
            format(22, true, true, Some(720), None),
            // Higher resolution but video-only -- must not be picked for
            // BestProgressive, or the user would silently get no audio.
            format(137, true, false, Some(1080), None),
        ];
        let picked = select_format(&formats, FormatPreference::BestProgressive).unwrap();
        assert_eq!(picked.itag, 22);
    }

    #[test]
    fn best_progressive_returns_none_when_no_progressive_format_exists() {
        let formats = vec![
            format(137, true, false, Some(1080), None),
            format(140, false, true, None, Some(128_000)),
        ];
        assert!(select_format(&formats, FormatPreference::BestProgressive).is_none());
    }

    #[test]
    fn best_audio_only_picks_highest_bitrate_audio_only_format() {
        let formats = vec![
            format(18, true, true, Some(360), Some(96_000)),
            format(139, false, true, None, Some(48_000)),
            format(140, false, true, None, Some(128_000)),
        ];
        let picked = select_format(&formats, FormatPreference::BestAudioOnly).unwrap();
        assert_eq!(picked.itag, 140);
    }
}
