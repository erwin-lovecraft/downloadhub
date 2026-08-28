//! Download orchestrator: runs one queue entry (plus optional MP3 transcode) to
//! completion, updating its `QueueStore` status along the way.

use tokio_util::sync::CancellationToken;

use crate::queue::{QueueError, QueueStatus, QueueStore};
use crate::stream::{
    select_format, FormatPreference, FormatRequest, FormatSummary, StreamClient, StreamError,
    YtDlpConfig,
};

use super::output::{destination_path, mp3_destination_path};
use super::progress::{DownloadPhase, DownloadProgress, Throttle};
use super::transcode::{BoxError, Transcode};

#[derive(Debug, thiserror::Error)]
pub enum DownloadError {
    #[error("queue entry {0} not found")]
    EntryNotFound(i64),
    #[error("format itag {0} is no longer offered for this video")]
    FormatNotFound(u32),
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
    /// Resolved fresh by the caller (settings override, bundled sidecar, or
    /// PATH) before each `run_download`/`run_all_queued` call, so a changed
    /// yt-dlp path or cookies apply without a restart.
    pub ytdlp_config: &'a YtDlpConfig,
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
    /// True when `cancel` was cancelled before every `Queued` entry was
    /// processed — i.e. a user-requested stop, not just running out of
    /// entries. The entry in progress when the token was cancelled still
    /// runs to completion; this only reflects whether the *next* one was
    /// skipped.
    pub stopped: bool,
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
/// `cancel` is checked *between* entries, never mid-transfer: cancelling it
/// stops the entry that's currently downloading from being the last one
/// started, but lets it run to completion rather than aborting it (aborting
/// an in-flight transfer is what `cancel_download` is for). This is what
/// lets a caller stop a 100-item batch after the 11th without losing the
/// bytes already in flight for it. The caller owns the token — this
/// function only ever reads it — so a batch-runner task can hand a clone to
/// whatever registry a "stop" command looks it up from.
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
    cancel: &CancellationToken,
    mut on_progress: impl FnMut(DownloadProgress) + Send,
    mut on_item_done: impl FnMut(i64, Result<DownloadProgress, String>) + Send,
) -> Result<BatchDownloadOutcome, QueueError> {
    let entries = ctx.store.list_entries().await?;
    let mut outcome = BatchDownloadOutcome::default();

    for entry in entries
        .into_iter()
        .filter(|e| e.status == QueueStatus::Queued)
    {
        if cancel.is_cancelled() {
            outcome.stopped = true;
            break;
        }

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
    on_progress: &mut (impl FnMut(DownloadProgress) + Send),
) -> Result<DownloadProgress, DownloadError> {
    let video = ctx
        .stream_client
        .get_video_formats(&entry.video_id, ctx.ytdlp_config)
        .await?;
    let format = resolve_source_format(&video.formats, entry)?;
    let total_bytes = format.and_then(|f| f.content_length_bytes).unwrap_or(0);

    // Validate the conversion prerequisite up front so a doomed entry fails
    // before spending bandwidth on the download. The source stream itself
    // needs no validating: ffmpeg drops any video track it carries.
    let transcoder = if entry.convert_to_mp3 {
        Some(ctx.transcoder.ok_or(DownloadError::TranscoderUnavailable)?)
    } else {
        None
    };

    // An MP3 entry lets the provider substitute another stream with audio,
    // so a format list that shifted between enqueue and download (a
    // different extractor client, a video that went live) doesn't sink a
    // conversion that would have worked from any source.
    let request = if entry.convert_to_mp3 {
        FormatRequest::any_audio(format.map_or(entry.itag, |f| f.itag))
    } else {
        FormatRequest::exact(entry.itag)
    };

    tokio::fs::create_dir_all(&entry.output_path).await?;
    let dest_path = destination_path(&entry.output_path, &entry.title, format);

    let mut throttle = Throttle::new();
    let bytes_written = ctx
        .stream_client
        .download(
            &entry.video_id,
            request,
            &dest_path,
            ctx.ytdlp_config,
            |downloaded, total| {
                if throttle.should_emit() {
                    on_progress(DownloadProgress {
                        queue_id,
                        bytes_written: downloaded,
                        total_bytes: if total > 0 { total } else { total_bytes },
                        phase: DownloadPhase::Downloading,
                    });
                }
            },
        )
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

/// The stream to actually fetch for `entry`: the itag it recorded, or — for
/// an MP3 entry whose itag the video no longer offers — whatever else can
/// feed the transcode. `None` means nothing in the list qualified and the
/// provider will pick (`FormatFallback::AnyAudio`), which only an MP3 entry
/// is allowed to do; anything else is a hard error, since substituting a
/// quality the user explicitly chose would be wrong.
fn resolve_source_format<'a>(
    formats: &'a [FormatSummary],
    entry: &crate::queue::QueueEntry,
) -> Result<Option<&'a FormatSummary>, DownloadError> {
    let exact = formats.iter().find(|f| f.itag == entry.itag);
    if !entry.convert_to_mp3 {
        return exact
            .map(Some)
            .ok_or(DownloadError::FormatNotFound(entry.itag));
    }
    Ok(exact.or_else(|| select_format(formats, FormatPreference::Mp3)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::queue::{NewQueueEntry, QueueStore};
    use crate::stream::{BoxFuture, StreamClient, StreamError, StreamProvider, VideoDetail};

    /// Never actually invoked by the tests below (they exercise the
    /// cancel-before-start path), but `StreamClient::new` needs some
    /// `StreamProvider` to wrap.
    struct UnusedProvider;

    impl StreamProvider for UnusedProvider {
        fn get_video<'a>(
            &'a self,
            _url_or_id: &'a str,
            _config: &'a YtDlpConfig,
        ) -> BoxFuture<'a, Result<VideoDetail, StreamError>> {
            Box::pin(async { unreachable!("test never calls the stream provider") })
        }

        fn download<'a>(
            &'a self,
            _url_or_id: &'a str,
            _request: FormatRequest,
            _dest: &'a std::path::Path,
            _config: &'a YtDlpConfig,
            _on_progress: &'a mut (dyn FnMut(u64, u64) + Send + 'a),
        ) -> BoxFuture<'a, Result<u64, StreamError>> {
            Box::pin(async { unreachable!("test never calls the stream provider") })
        }
    }

    fn new_entry(video_id: &str) -> NewQueueEntry {
        NewQueueEntry {
            video_id: video_id.to_string(),
            title: video_id.to_string(),
            itag: 140,
            quality_label: None,
            output_path: "/tmp/downloadhub-test".to_string(),
            convert_to_mp3: false,
        }
    }

    #[tokio::test]
    async fn a_pre_cancelled_token_stops_before_the_next_entry_starts() {
        let store = QueueStore::open_in_memory().unwrap();
        store.add_entry(new_entry("a")).await.unwrap();
        store.add_entry(new_entry("b")).await.unwrap();

        let ytdlp_config = YtDlpConfig::default();
        let ctx = DownloadContext {
            stream_client: &StreamClient::new(UnusedProvider),
            store: &store,
            ytdlp_config: &ytdlp_config,
            transcoder: None,
        };
        let cancel = CancellationToken::new();
        cancel.cancel();

        let outcome = run_all_queued(&ctx, &cancel, |_| {}, |_, _| {})
            .await
            .unwrap();

        assert!(outcome.stopped);
        assert_eq!(outcome.completed, 0);
        assert_eq!(outcome.failed, 0);

        // Neither entry was touched: a token already cancelled before the
        // first iteration must never even start the entry that would've
        // gone next, let alone call `run_download` on it.
        let entries = store.list_entries().await.unwrap();
        assert!(entries.iter().all(|e| e.status == QueueStatus::Queued));
    }

    #[tokio::test]
    async fn an_uncancelled_token_with_an_empty_queue_finishes_without_stopping() {
        let store = QueueStore::open_in_memory().unwrap();
        let ytdlp_config = YtDlpConfig::default();
        let ctx = DownloadContext {
            stream_client: &StreamClient::new(UnusedProvider),
            store: &store,
            ytdlp_config: &ytdlp_config,
            transcoder: None,
        };

        let outcome = run_all_queued(&ctx, &CancellationToken::new(), |_| {}, |_, _| {})
            .await
            .unwrap();

        assert!(!outcome.stopped);
        assert_eq!(outcome.completed, 0);
        assert_eq!(outcome.failed, 0);
    }
}
