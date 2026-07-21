//! [`StreamClient`], the thin wrapper over `y7dl::Client`.

use super::models::{select_format, FormatPreference, FormatSummary, ResolvedFormat, VideoDetail};
use super::StreamError;

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
    /// Used wherever asking the user to pick an exact itag per video isn't
    /// practical (playlist import, MCP enqueueing, bulk re-format). Returns
    /// [`StreamError::FormatNotFound`] if nothing matches (e.g.
    /// `BestProgressive` on a video YouTube only serves as separate
    /// video/audio DASH streams).
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

    /// [`Self::resolve_preferred_format`] reduced to just the fields a queue
    /// row needs — including the `convert_to_mp3` flag, which is a property
    /// of the *preference*, not of the selected stream. The one place that
    /// pairing is decided, so enqueueing and re-formatting can't drift.
    pub async fn resolve_queue_format(
        &self,
        url_or_id: &str,
        preference: FormatPreference,
    ) -> Result<(VideoDetail, ResolvedFormat), StreamError> {
        let (detail, format) = self.resolve_preferred_format(url_or_id, preference).await?;
        let resolved = ResolvedFormat {
            itag: format.itag,
            quality_label: format.quality_label,
            convert_to_mp3: preference.convert_to_mp3(),
        };
        Ok((detail, resolved))
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
