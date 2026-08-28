//! [`YtDlpProvider`]: the concrete [`downloadhub_core::stream::StreamProvider`]
//! backed by a real `yt-dlp` subprocess. This is the crate's one dependency
//! on `core` — everything else here is plain data/process plumbing.

use std::path::Path;

use downloadhub_core::stream::{
    BoxFuture, FormatFallback, FormatRequest, FormatSummary, StreamError, StreamProvider,
    VideoDetail, YtDlpConfig, AUTO_AUDIO_ITAG,
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
        request: FormatRequest,
        dest: &'a Path,
        config: &'a YtDlpConfig,
        on_progress: &'a mut (dyn FnMut(u64, u64) + Send + 'a),
    ) -> BoxFuture<'a, Result<u64, StreamError>> {
        Box::pin(async move {
            let ytdlp = Self::resolve(config)?;
            ytdlp
                .download(url_or_id, &format_selector(request), dest, on_progress)
                .await
                .map_err(map_error)
        })
    }
}

/// Translates a [`FormatRequest`] into a yt-dlp `-f` selector — the one
/// place `core`'s semantic "any stream with audio will do" becomes yt-dlp
/// syntax. yt-dlp resolves a `/`-separated chain left to right, taking the
/// first alternative a video actually offers, so an [`FormatFallback::AnyAudio`]
/// request degrades from the recorded itag through the best audio-only
/// stream (`bestaudio`) and the best stream that merely *contains* audio
/// (`bestaudio*`, which allows video the transcode then discards) to
/// whatever `best` resolves to. That last pair is what makes an MP3 work
/// from a source with no separate audio track at all, and from one whose
/// format ids `core` can't represent, where the recorded itag is
/// [`AUTO_AUDIO_ITAG`] and gets left out of the chain entirely.
fn format_selector(request: FormatRequest) -> String {
    let itag = (request.itag != AUTO_AUDIO_ITAG).then(|| request.itag.to_string());
    match request.fallback {
        FormatFallback::Exact => itag.unwrap_or_else(|| "best".to_string()),
        FormatFallback::AnyAudio => itag
            .into_iter()
            .chain(["bestaudio", "bestaudio*", "best"].map(str::to_string))
            .collect::<Vec<_>>()
            .join("/"),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_exact_request_selects_only_that_itag() {
        assert_eq!(format_selector(FormatRequest::exact(137)), "137");
    }

    #[test]
    fn an_any_audio_request_falls_back_past_its_itag() {
        assert_eq!(
            format_selector(FormatRequest::any_audio(140)),
            "140/bestaudio/bestaudio*/best"
        );
    }

    #[test]
    fn an_auto_itag_leaves_the_pick_entirely_to_yt_dlp() {
        assert_eq!(
            format_selector(FormatRequest::any_audio(AUTO_AUDIO_ITAG)),
            "bestaudio/bestaudio*/best"
        );
    }
}
