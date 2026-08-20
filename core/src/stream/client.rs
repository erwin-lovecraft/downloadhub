//! `StreamClient`, the thin wrapper over `downloadhub_ytdlp::YtDlp`.

use std::path::Path;

use downloadhub_ytdlp::YtDlp;

use super::config::YtDlpConfig;
use super::models::{select_format, FormatPreference, FormatSummary, ResolvedFormat, VideoDetail};
use super::StreamError;

/// Stateless marker kept for API-shape stability (call sites hold one
/// reused instance, mirroring the old `y7dl::Client`-pooling shape) — but
/// yt-dlp is a subprocess spawned fresh per call, so there's no connection
/// or cache to actually hold onto. Every method takes a [`YtDlpConfig`]
/// resolved by the caller so a settings change (binary path, cookies)
/// applies to the very next call.
#[derive(Debug, Clone, Copy, Default)]
pub struct StreamClient;

impl StreamClient {
    pub fn new() -> Self {
        Self
    }

    fn resolve(config: &YtDlpConfig) -> Result<YtDlp, StreamError> {
        let binary_path = config
            .binary_path
            .clone()
            .or_else(downloadhub_ytdlp::locate_ytdlp)
            .ok_or(StreamError::YtDlpNotFound)?;
        Ok(YtDlp::new(binary_path, config.cookies_path.clone()))
    }

    /// Fetches metadata and the full available format/quality list for a
    /// video URL or bare 11-character ID.
    pub async fn get_video_formats(
        &self,
        url_or_id: &str,
        config: &YtDlpConfig,
    ) -> Result<VideoDetail, StreamError> {
        let video = self.fetch_video(url_or_id, config).await?;
        Ok(VideoDetail {
            video_id: video.id,
            title: video.title,
            author: video.author,
            duration_seconds: video.duration_secs,
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

    /// Fetches raw yt-dlp video metadata (including formats) for a video URL
    /// or bare ID. Exposed for `core::download`, which needs the raw
    /// `downloadhub_ytdlp::Format` rather than the IPC-facing `FormatSummary`
    /// DTO.
    pub async fn fetch_video(
        &self,
        url_or_id: &str,
        config: &YtDlpConfig,
    ) -> Result<downloadhub_ytdlp::Video, StreamError> {
        let ytdlp = Self::resolve(config)?;
        Ok(ytdlp.fetch_video(url_or_id).await?)
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
        on_progress: impl FnMut(u64, u64) + Send,
    ) -> Result<u64, StreamError> {
        let ytdlp = Self::resolve(config)?;
        Ok(ytdlp
            .download(url_or_id, &itag.to_string(), dest, on_progress)
            .await?)
    }
}
