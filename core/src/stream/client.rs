//! `StreamClient`, the business-logic wrapper over a [`StreamProvider`].

use std::path::Path;

use super::config::YtDlpConfig;
use super::models::{select_format, FormatPreference, FormatSummary, ResolvedFormat, VideoDetail};
use super::provider::StreamProvider;
use super::StreamError;

/// Wraps whatever [`StreamProvider`] the caller constructs it with (a real
/// yt-dlp process in production, a test double in unit tests) and adds the
/// format-selection logic on top, so call sites don't need to depend on a
/// concrete provider at all.
pub struct StreamClient {
    provider: Box<dyn StreamProvider>,
}

impl StreamClient {
    pub fn new(provider: impl StreamProvider + 'static) -> Self {
        Self {
            provider: Box::new(provider),
        }
    }

    /// Fetches metadata and the full available format/quality list for a
    /// video URL or bare 11-character ID.
    pub async fn get_video_formats(
        &self,
        url_or_id: &str,
        config: &YtDlpConfig,
    ) -> Result<VideoDetail, StreamError> {
        self.provider.get_video(url_or_id, config).await
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
        config: &YtDlpConfig,
    ) -> Result<(VideoDetail, FormatSummary), StreamError> {
        let detail = self.get_video_formats(url_or_id, config).await?;
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
        config: &YtDlpConfig,
    ) -> Result<(VideoDetail, ResolvedFormat), StreamError> {
        let (detail, format) = self
            .resolve_preferred_format(url_or_id, preference, config)
            .await?;
        let resolved = ResolvedFormat {
            itag: format.itag,
            quality_label: format.quality_label,
            convert_to_mp3: preference.convert_to_mp3(),
        };
        Ok((detail, resolved))
    }

    /// Downloads `itag`'s stream for `url_or_id` to the exact path `dest`.
    /// `on_progress` receives raw `(downloaded_bytes, total_bytes)` calls,
    /// unthrottled — the caller decides how often to forward them.
    /// Returns the number of bytes written.
    pub async fn download(
        &self,
        url_or_id: &str,
        itag: u32,
        dest: &Path,
        config: &YtDlpConfig,
        mut on_progress: impl FnMut(u64, u64) + Send,
    ) -> Result<u64, StreamError> {
        self.provider
            .download(url_or_id, itag, dest, config, &mut on_progress)
            .await
    }
}
