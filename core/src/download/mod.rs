//! Download orchestrator: executes one queue entry's download through
//! `StreamClient`, updating its status in `QueueStore` and reporting
//! progress along the way.
//!
//! `y7dl` doesn't mux DASH streams, so a video-only or audio-only format is
//! saved to its own clearly-labeled file (`<title>.video.<ext>` /
//! `<title>.audio.<ext>`); a progressive format (both tracks in one stream)
//! is saved as `<title>.<ext>` — see `docs/ARCHITECTURE.md`. There is no
//! `Muxer` yet; that's a future extension point, not this module's job.
//!
//! An entry flagged `convert_to_mp3` (audio-only formats only) gets one
//! extra step after its download completes: the m4a is transcoded to
//! `<title>.mp3` through `downloadhub-transcode`'s ffmpeg wrapper and the
//! m4a is deleted. The transcode is part of the same `run_download` call,
//! so a batch naturally finishes each entry's conversion before starting
//! the next download.
//!
//! This module has no Tauri dependency: progress is reported through a
//! plain callback so the Tauri layer can wire it to event emission without
//! `core` knowing anything about Tauri's event system.

use std::path::PathBuf;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

use tokio::io::{self, AsyncWrite};

use crate::queue::{QueueError, QueueStatus, QueueStore};
use crate::stream::{StreamClient, StreamError};
use downloadhub_transcode::{TranscodeError, Transcoder};

#[derive(Debug, thiserror::Error)]
pub enum DownloadError {
    #[error("queue entry {0} not found")]
    EntryNotFound(i64),
    #[error("format itag {0} is no longer offered for this video")]
    FormatNotFound(u32),
    #[error("MP3 conversion needs an audio-only format, but itag {0} has video")]
    NotAudioOnly(u32),
    #[error("MP3 conversion is unavailable: no ffmpeg binary was found (reinstall the app, or put ffmpeg on PATH)")]
    TranscoderUnavailable,
    #[error(transparent)]
    Stream(#[from] StreamError),
    #[error(transparent)]
    Queue(#[from] QueueError),
    #[error(transparent)]
    Transcode(#[from] TranscodeError),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

/// Which step of an entry's pipeline a progress report describes. There is
/// no percentage for `Transcoding` (ffmpeg's duration isn't predicted);
/// callers show it as an indeterminate "converting" state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownloadPhase {
    Downloading,
    Transcoding,
}

#[derive(Debug, Clone, Copy)]
pub struct DownloadProgress {
    pub queue_id: i64,
    pub bytes_written: u64,
    /// 0 when the stream's total size isn't known upfront.
    pub total_bytes: u64,
    pub phase: DownloadPhase,
}

/// Everything `run_download`/`run_all_queued` need besides the entry
/// itself, grouped so their signatures don't grow a parameter per feature.
#[derive(Clone, Copy)]
pub struct DownloadContext<'a> {
    pub stream_client: &'a StreamClient,
    pub store: &'a QueueStore,
    /// `None` when no ffmpeg binary was found at startup; entries flagged
    /// `convert_to_mp3` then fail with a clear message instead of silently
    /// keeping the m4a.
    pub transcoder: Option<&'a Transcoder>,
}

/// Runs one queue entry's download (and, if flagged, its MP3 conversion)
/// to completion, transitioning its status in the store (`Downloading` →
/// `Completed`/`Failed`) as it goes. `on_progress` is throttled internally
/// (~5/sec) so callers can wire it directly to a UI event emitter without
/// flooding it.
pub async fn run_download(
    queue_id: i64,
    ctx: &DownloadContext<'_>,
    mut on_progress: impl FnMut(DownloadProgress) + Send,
) -> Result<DownloadProgress, DownloadError> {
    let entry = ctx
        .store
        .get_entry(queue_id)
        .await?
        .ok_or(DownloadError::EntryNotFound(queue_id))?;

    ctx.store
        .set_status(queue_id, QueueStatus::Downloading, None)
        .await?;

    let result = download_entry(queue_id, &entry, ctx, &mut on_progress).await;

    match &result {
        Ok(progress) => {
            ctx.store
                .set_status(queue_id, QueueStatus::Completed, None)
                .await?;
            on_progress(*progress);
        }
        Err(e) => {
            ctx.store
                .set_status(queue_id, QueueStatus::Failed, Some(&e.to_string()))
                .await?;
        }
    }
    result
}

#[derive(Debug, Clone, Copy, Default, serde::Serialize)]
pub struct BatchDownloadOutcome {
    pub completed: usize,
    pub failed: usize,
}

/// Downloads every currently-`Queued` entry, one at a time (not
/// concurrently — see `docs/ARCHITECTURE.md`). A per-entry failure is
/// recorded (`run_download` already leaves the entry `Failed` with its
/// error message in the store, exactly as if it had been started
/// individually and failed) and processing moves on to the next entry
/// rather than aborting the whole batch. Only a `QueueStore` failure
/// itself (listing entries) stops the batch early, since that affects
/// every remaining entry too.
///
/// Because an entry's MP3 conversion happens inside its `run_download`
/// call, the batch fully finishes one entry — download, transcode, delete
/// the m4a — before moving to the next.
///
/// `on_progress` reports throttled in-progress updates, same as
/// `run_download`. `on_item_done` fires once per entry after its
/// `run_download` call resolves (`Ok` on success, `Err`'s `to_string()` on
/// failure — a plain `String` rather than `DownloadError` so callers don't
/// need to depend on this module's error type just to report it), letting
/// a caller emit the same per-item completion signal it would for an
/// individually-started download.
pub async fn run_all_queued(
    ctx: &DownloadContext<'_>,
    mut on_progress: impl FnMut(DownloadProgress) + Send,
    mut on_item_done: impl FnMut(i64, Result<DownloadProgress, String>) + Send,
) -> Result<BatchDownloadOutcome, QueueError> {
    let entries = ctx.store.list_entries().await?;
    let mut outcome = BatchDownloadOutcome::default();

    for entry in entries
        .into_iter()
        .filter(|e| e.status == QueueStatus::Queued)
    {
        let result = run_download(entry.id, ctx, &mut on_progress).await;
        match &result {
            Ok(_) => outcome.completed += 1,
            Err(_) => outcome.failed += 1,
        }
        on_item_done(entry.id, result.map_err(|e| e.to_string()));
    }

    Ok(outcome)
}

async fn download_entry(
    queue_id: i64,
    entry: &crate::queue::QueueEntry,
    ctx: &DownloadContext<'_>,
    on_progress: &mut impl FnMut(DownloadProgress),
) -> Result<DownloadProgress, DownloadError> {
    let video = ctx.stream_client.fetch_video(&entry.video_id).await?;
    let format = video
        .format_by_itag(entry.itag)
        .ok_or(DownloadError::FormatNotFound(entry.itag))?;
    let total_bytes = format.content_length().unwrap_or(0);

    // Validate the conversion prerequisites up front so a doomed entry
    // fails before spending bandwidth on the download.
    let transcoder = if entry.convert_to_mp3 {
        if format.is_video() {
            return Err(DownloadError::NotAudioOnly(entry.itag));
        }
        Some(ctx.transcoder.ok_or(DownloadError::TranscoderUnavailable)?)
    } else {
        None
    };

    tokio::fs::create_dir_all(&entry.output_path).await?;
    let dest_path = destination_path(&entry.output_path, &entry.title, format);
    let mut file = tokio::fs::File::create(&dest_path).await?;

    let mut writer = ProgressWriter {
        inner: &mut file,
        queue_id,
        total_bytes,
        written: 0,
        last_emit: Instant::now(),
        on_progress,
    };
    let bytes_written = ctx.stream_client.download(&video, format, &mut writer).await?;

    if let Some(transcoder) = transcoder {
        on_progress(DownloadProgress {
            queue_id,
            bytes_written,
            total_bytes,
            phase: DownloadPhase::Transcoding,
        });
        let mp3_path = mp3_destination_path(&entry.output_path, &entry.title);
        transcoder.to_mp3(&dest_path, &mp3_path).await?;
        // Only delete the source after a successful transcode; on failure
        // the m4a stays so the downloaded data isn't lost.
        tokio::fs::remove_file(&dest_path).await?;
    }

    Ok(DownloadProgress {
        queue_id,
        bytes_written,
        total_bytes,
        phase: DownloadPhase::Downloading,
    })
}

const PROGRESS_EMIT_INTERVAL: Duration = Duration::from_millis(200);

/// Wraps a destination writer, invoking `on_progress` (throttled) after
/// each successful write so callers get incremental progress without
/// modifying `y7dl` itself.
struct ProgressWriter<'a, W, F> {
    inner: W,
    queue_id: i64,
    total_bytes: u64,
    written: u64,
    last_emit: Instant,
    on_progress: &'a mut F,
}

impl<W, F> AsyncWrite for ProgressWriter<'_, W, F>
where
    W: AsyncWrite + Unpin,
    F: FnMut(DownloadProgress),
{
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let this = self.get_mut();
        let poll = Pin::new(&mut this.inner).poll_write(cx, buf);
        if let Poll::Ready(Ok(n)) = &poll {
            this.written += *n as u64;
            let now = Instant::now();
            if now.duration_since(this.last_emit) >= PROGRESS_EMIT_INTERVAL {
                this.last_emit = now;
                (this.on_progress)(DownloadProgress {
                    queue_id: this.queue_id,
                    bytes_written: this.written,
                    total_bytes: this.total_bytes,
                    phase: DownloadPhase::Downloading,
                });
            }
        }
        poll
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().inner).poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().inner).poll_shutdown(cx)
    }
}

fn destination_path(output_folder: &str, title: &str, format: &y7dl::Format) -> PathBuf {
    let suffix = if format.is_video() && format.has_audio() {
        ""
    } else if format.is_video() {
        ".video"
    } else {
        ".audio"
    };
    let ext = extension_for(&format.mime_type);
    let mut path = PathBuf::from(output_folder);
    path.push(format!("{}{suffix}.{ext}", sanitize_filename(title)));
    path
}

/// The final artifact of an MP3 conversion. No `.audio` marker — unlike a
/// bare DASH stream, the mp3 is exactly the file the user asked for.
fn mp3_destination_path(output_folder: &str, title: &str) -> PathBuf {
    let mut path = PathBuf::from(output_folder);
    path.push(format!("{}.mp3", sanitize_filename(title)));
    path
}

/// Maps a format's mime type (e.g. `video/mp4; codecs="avc1..."`) to a file
/// extension. Falls back to `bin` for anything unrecognized rather than
/// guessing.
fn extension_for(mime_type: &str) -> &'static str {
    match mime_type.split(';').next().unwrap_or(mime_type).trim() {
        "video/mp4" => "mp4",
        "video/webm" => "webm",
        "audio/mp4" => "m4a",
        "audio/webm" => "webm",
        _ => "bin",
    }
}

/// Strips characters invalid in Windows/Unix filenames and caps length well
/// under any filesystem's path-component limit.
fn sanitize_filename(title: &str) -> String {
    let cleaned: String = title
        .chars()
        .map(|c| {
            if "<>:\"/\\|?*".contains(c) || c.is_control() {
                '_'
            } else {
                c
            }
        })
        .collect();
    let trimmed = cleaned.trim();
    let truncated: String = trimmed.chars().take(150).collect();
    if truncated.is_empty() || truncated.chars().all(|c| c == '_') {
        "video".to_string()
    } else {
        truncated
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_filename_replaces_invalid_characters() {
        assert_eq!(
            sanitize_filename("a/b\\c:d*e?f\"g<h>i|j"),
            "a_b_c_d_e_f_g_h_i_j"
        );
    }

    #[test]
    fn sanitize_filename_falls_back_when_empty() {
        assert_eq!(sanitize_filename("   "), "video");
        assert_eq!(sanitize_filename("///"), "video");
    }

    #[test]
    fn sanitize_filename_truncates_long_titles() {
        let long = "a".repeat(300);
        assert_eq!(sanitize_filename(&long).len(), 150);
    }

    #[test]
    fn extension_for_maps_known_mime_types() {
        assert_eq!(extension_for(r#"video/mp4; codecs="avc1.42001E""#), "mp4");
        assert_eq!(extension_for(r#"video/webm; codecs="vp9""#), "webm");
        assert_eq!(extension_for(r#"audio/mp4; codecs="mp4a.40.2""#), "m4a");
        assert_eq!(extension_for(r#"audio/webm; codecs="opus""#), "webm");
        assert_eq!(extension_for("application/octet-stream"), "bin");
    }

    fn format(mime_type: &str, audio_quality: Option<&str>) -> y7dl::Format {
        y7dl::Format {
            itag: 0,
            url: None,
            mime_type: mime_type.to_string(),
            bitrate: None,
            average_bitrate: None,
            width: None,
            height: None,
            fps: None,
            content_length: None,
            quality: None,
            quality_label: None,
            audio_quality: audio_quality.map(str::to_string),
            audio_sample_rate: None,
            audio_channels: None,
            approx_duration_ms: None,
            signature_cipher: None,
        }
    }

    #[test]
    fn destination_path_is_bare_for_progressive_formats() {
        let f = format("video/mp4; codecs=\"avc1\"", Some("AUDIO_QUALITY_MEDIUM"));
        let path = destination_path("C:/out", "My Title", &f);
        assert_eq!(path, PathBuf::from("C:/out").join("My Title.mp4"));
    }

    #[test]
    fn destination_path_labels_video_only_formats() {
        let f = format("video/webm; codecs=\"vp9\"", None);
        let path = destination_path("C:/out", "My Title", &f);
        assert_eq!(path, PathBuf::from("C:/out").join("My Title.video.webm"));
    }

    #[test]
    fn destination_path_labels_audio_only_formats() {
        let f = format(
            "audio/mp4; codecs=\"mp4a.40.2\"",
            Some("AUDIO_QUALITY_MEDIUM"),
        );
        let path = destination_path("C:/out", "My Title", &f);
        assert_eq!(path, PathBuf::from("C:/out").join("My Title.audio.m4a"));
    }

    #[test]
    fn mp3_destination_path_has_no_audio_marker() {
        let path = mp3_destination_path("C:/out", "My: Title?");
        assert_eq!(path, PathBuf::from("C:/out").join("My_ Title_.mp3"));
    }
}
