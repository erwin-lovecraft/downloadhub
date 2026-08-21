//! [`YtDlpProvider`]: the concrete [`downloadhub_core::stream::StreamProvider`]
//! backed by a real `yt-dlp` subprocess. This is the crate's one dependency
//! on `core` — everything else here is plain data/process plumbing.

use std::path::Path;

use downloadhub_core::stream::{
    BoxFuture, FormatSummary, StreamError, StreamProvider, VideoDetail, YtDlpConfig,
};

use crate::{locate_ytdlp, Error, Format, YtDlp};

/// Stateless marker kept for API-shape stability — yt-dlp is a subprocess
/// spawned fresh per call, so there's no connection or cache to actually
/// hold onto. Every call resolves a [`YtDlpConfig`] itself so a settings
/// change (binary path, cookies) applies to the very next call.
#[derive(Debug, Clone, Copy, Default)]
pub struct YtDlpProvider;

impl YtDlpProvider {
    pub fn new() -> Self {
        Self
    }

    fn resolve(config: &YtDlpConfig) -> Result<YtDlp, StreamError> {
        let binary_path = config
            .binary_path
            .clone()
            .or_else(locate_ytdlp)
            .ok_or(StreamError::YtDlpNotFound)?;
        Ok(YtDlp::new(binary_path, config.cookies_path.clone()))
    }
}

impl StreamProvider for YtDlpProvider {
    fn get_video<'a>(
        &'a self,
        url_or_id: &'a str,
        config: &'a YtDlpConfig,
    ) -> BoxFuture<'a, Result<VideoDetail, StreamError>> {
        Box::pin(async move {
            let ytdlp = Self::resolve(config)?;
            let video = ytdlp.fetch_video(url_or_id).await.map_err(map_error)?;
            Ok(VideoDetail {
                video_id: video.id,
                title: video.title,
                author: video.author,
                duration_seconds: video.duration_secs,
                formats: video.formats.into_iter().map(to_format_summary).collect(),
            })
        })
    }

    fn download<'a>(
        &'a self,
        url_or_id: &'a str,
        itag: u32,
        dest: &'a Path,
        config: &'a YtDlpConfig,
        on_progress: &'a mut (dyn FnMut(u64, u64) + Send + 'a),
    ) -> BoxFuture<'a, Result<u64, StreamError>> {
        Box::pin(async move {
            let ytdlp = Self::resolve(config)?;
            ytdlp
                .download(url_or_id, &itag.to_string(), dest, on_progress)
                .await
                .map_err(map_error)
        })
    }
}

fn to_format_summary(format: Format) -> FormatSummary {
    let has_video = format.is_video();
    let has_audio = format.has_audio();
    FormatSummary {
        itag: format.itag,
        ext: format.ext,
        quality_label: format.quality_label,
        width: format.width,
        height: format.height,
        fps: format.fps,
        bitrate: format.bitrate,
        content_length_bytes: format.filesize_bytes,
        has_video,
        has_audio,
    }
}

fn map_error(error: Error) -> StreamError {
    match error {
        Error::InvalidVideoId(input) => StreamError::InvalidVideoId(input),
        Error::VideoUnavailable(reason) => StreamError::VideoUnavailable(reason),
        Error::FormatNotFound => StreamError::FormatNotFound,
        Error::BinaryNotFound(_) => StreamError::YtDlpNotFound,
        other => StreamError::Other(other.to_string()),
    }
}
