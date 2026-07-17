//! The download orchestrator: executes one queue entry's download through
//! `StreamClient`, updating its status in `QueueStore` and reporting
//! progress along the way.
//!
//! An entry flagged `convert_to_mp3` (audio-only formats only) gets one
//! extra step after its download completes: the m4a is transcoded to
//! `<title>.mp3` through the [`Transcode`] seam and the m4a is deleted.
//! The transcode is part of the same `run_download` call, so a batch
//! naturally finishes each entry's conversion before starting the next.

use std::time::Instant;

use crate::queue::{QueueError, QueueStatus, QueueStore};
use crate::stream::{StreamClient, StreamError};

use super::output::{destination_path, mp3_destination_path};
use super::progress::{DownloadPhase, DownloadProgress, ProgressWriter};
use super::transcode::{BoxError, Transcode};

#[derive(Debug, thiserror::Error)]
pub enum DownloadError {
    #[error("queue entry {0} not found")]
    EntryNotFound(i64),
    #[error("format itag {0} is no longer offered for this video")]
    FormatNotFound(u32),
    #[error("MP3 conversion needs an audio-only format, but itag {0} has video")]
    NotAudioOnly(u32),
    #[error("MP3 conversion is unavailable: no ffmpeg binary was found (set an ffmpeg path in Settings, or put ffmpeg on PATH)")]
    TranscoderUnavailable,
    #[error(transparent)]
    Stream(#[from] StreamError),
    #[error(transparent)]
    Queue(#[from] QueueError),
    #[error("{0}")]
    Transcode(BoxError),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
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
    pub transcoder: Option<&'a dyn Transcode>,
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
    let bytes_written = ctx
        .stream_client
        .download(&video, format, &mut writer)
        .await?;

    if let Some(transcoder) = transcoder {
        on_progress(DownloadProgress {
            queue_id,
            bytes_written,
            total_bytes,
            phase: DownloadPhase::Transcoding,
        });
        let mp3_path = mp3_destination_path(&entry.output_path, &entry.title);
        transcoder
            .to_mp3(&dest_path, &mp3_path)
            .await
            .map_err(DownloadError::Transcode)?;
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
