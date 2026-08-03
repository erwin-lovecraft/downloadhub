//! Tauri commands that run downloads in the background and forward progress to
//! the frontend as `download-progress` events.

use crate::state::AppState;
use downloadhub_core::download::{
    self, BatchDownloadOutcome, CancellationToken, DownloadContext, DownloadPhase,
    DownloadProgress, Transcode,
};
use downloadhub_core::queue::QueueStatus;
use serde::Serialize;
use std::sync::atomic::Ordering;
use tauri::{AppHandle, Emitter, Manager, State};

const PROGRESS_EVENT: &str = "download-progress";
const BATCH_DONE_EVENT: &str = "download-batch-done";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
enum DownloadStatusEvent {
    Downloading,
    Transcoding,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize)]
struct DownloadProgressEvent {
    queue_id: i64,
    bytes_written: u64,
    total_bytes: u64,
    status: DownloadStatusEvent,
    error_message: Option<String>,
}

impl DownloadProgressEvent {
    fn in_progress(progress: DownloadProgress) -> Self {
        Self {
            queue_id: progress.queue_id,
            bytes_written: progress.bytes_written,
            total_bytes: progress.total_bytes,
            status: match progress.phase {
                DownloadPhase::Downloading => DownloadStatusEvent::Downloading,
                DownloadPhase::Transcoding => DownloadStatusEvent::Transcoding,
            },
            error_message: None,
        }
    }

    fn completed(progress: DownloadProgress) -> Self {
        Self {
            queue_id: progress.queue_id,
            bytes_written: progress.bytes_written,
            total_bytes: progress.total_bytes,
            status: DownloadStatusEvent::Completed,
            error_message: None,
        }
    }

    fn failed(queue_id: i64, error_message: String) -> Self {
        Self {
            queue_id,
            bytes_written: 0,
            total_bytes: 0,
            status: DownloadStatusEvent::Failed,
            error_message: Some(error_message),
        }
    }
}

/// Reports how a `download_all` batch job ended, since the command itself
/// now returns the job id immediately rather than awaiting the batch (see
/// `spawn_batch`). `error_message` is only set for a `QueueStore` failure
/// that stopped the batch before it could produce a `BatchDownloadOutcome`
/// at all — a per-*entry* failure is still reported the normal way, as a
/// `download-progress` `Failed` event, and counted in `failed`.
#[derive(Debug, Clone, Serialize)]
struct BatchDownloadDoneEvent {
    job_id: u64,
    completed: usize,
    failed: usize,
    stopped: bool,
    error_message: Option<String>,
}

impl BatchDownloadDoneEvent {
    fn from_outcome(job_id: u64, outcome: Result<BatchDownloadOutcome, String>) -> Self {
        match outcome {
            Ok(outcome) => Self {
                job_id,
                completed: outcome.completed,
                failed: outcome.failed,
                stopped: outcome.stopped,
                error_message: None,
            },
            Err(e) => Self {
                job_id,
                completed: 0,
                failed: 0,
                stopped: false,
                error_message: Some(e),
            },
        }
    }
}

/// Starts a queued entry's download in the background and returns
/// immediately; progress/completion/failure are reported via
/// `download-progress` events rather than this command's return value.
#[tauri::command]
pub async fn start_download<R: tauri::Runtime>(
    queue_id: i64,
    app: AppHandle<R>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    spawn_download(&app, &state, queue_id).await
}

/// The guarded spawn behind `start_download`. Kept factored out from the
/// command itself so any future caller starts a download exactly the way
/// the user clicking Start does: same guards, same `running_downloads`
/// registry, same progress events.
pub async fn spawn_download<R: tauri::Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
    queue_id: i64,
) -> Result<(), String> {
    state.ensure_no_batch_running()?;
    let store = state.queue_store()?;

    if let Some(entry) = store.get_entry(queue_id).await.map_err(|e| e.to_string())? {
        if entry.status == QueueStatus::Downloading {
            return Err("This entry is already downloading.".to_string());
        }
    }

    let task_app = app.clone();
    let handle = tauri::async_runtime::spawn(async move {
        let state = task_app.state::<AppState>();
        let Some(store) = state.queue_store.as_ref() else {
            return;
        };
        let transcoder = state.resolve_transcoder().await;
        let ctx = DownloadContext {
            stream_client: &state.stream_client,
            store,
            transcoder: transcoder.as_ref().map(|t| t as &dyn Transcode),
        };

        let progress_app = task_app.clone();
        let result = download::run_download(queue_id, &ctx, move |progress| {
            let _ = progress_app.emit(PROGRESS_EVENT, DownloadProgressEvent::in_progress(progress));
        })
        .await;

        task_app
            .state::<AppState>()
            .running_downloads
            .lock()
            .expect("running downloads mutex poisoned")
            .remove(&queue_id);

        let event = match result {
            Ok(progress) => DownloadProgressEvent::completed(progress),
            Err(e) => DownloadProgressEvent::failed(queue_id, e.to_string()),
        };
        let _ = task_app.emit(PROGRESS_EVENT, event);
    });

    state
        .running_downloads
        .lock()
        .expect("running downloads mutex poisoned")
        .insert(queue_id, handle);

    Ok(())
}

/// Aborts an in-flight download (if one is running for `queue_id`) and
/// marks the entry `Cancelled`. A no-op status-wise if the entry already
/// reached a terminal state (`Completed`/`Failed`) — cancelling doesn't
/// retroactively undo a finished download.
#[tauri::command]
pub async fn cancel_download(queue_id: i64, state: State<'_, AppState>) -> Result<(), String> {
    state.ensure_no_batch_running()?;
    let store = state.queue_store()?;

    if let Some(handle) = state
        .running_downloads
        .lock()
        .expect("running downloads mutex poisoned")
        .remove(&queue_id)
    {
        handle.abort();
    }

    if let Some(entry) = store.get_entry(queue_id).await.map_err(|e| e.to_string())? {
        if matches!(entry.status, QueueStatus::Queued | QueueStatus::Downloading) {
            store
                .set_status(queue_id, QueueStatus::Cancelled, None)
                .await
                .map_err(|e| e.to_string())?;
        }
    }

    Ok(())
}

/// Starts a batch job downloading every currently-`Queued` entry, one at a
/// time, and returns its job id immediately — the batch itself runs as a
/// single background worker task, the same fire-and-forget shape as
/// `start_download`. Progress/per-item completion stream out as
/// `download-progress` events throughout, same as an individually-started
/// download; the batch's own completion (with the tallied
/// `BatchDownloadOutcome`) arrives as one `download-batch-done` event
/// carrying this job id, since the command's return value can't carry it
/// anymore.
#[tauri::command]
pub async fn download_all<R: tauri::Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
) -> Result<u64, String> {
    spawn_batch(&app, &state).await
}

/// The guarded batch spawn behind `download_all`, factored out so any
/// future caller gets the same `batch_running` exclusivity guards and job
/// registration rather than re-implementing them.
pub async fn spawn_batch<R: tauri::Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
) -> Result<u64, String> {
    if state.batch_running.swap(true, Ordering::SeqCst) {
        return Err("A batch download is already in progress.".to_string());
    }
    if !state
        .running_downloads
        .lock()
        .expect("running downloads mutex poisoned")
        .is_empty()
    {
        state.batch_running.store(false, Ordering::SeqCst);
        return Err("Wait for the current download to finish before starting a batch.".to_string());
    }

    let cancel = CancellationToken::new();
    let job_id = state.register_batch_job(cancel.clone());

    let task_app = app.clone();
    tauri::async_runtime::spawn(async move {
        let state = task_app.state::<AppState>();
        let outcome = run_batch(&task_app, &state, &cancel).await;

        state.finish_batch_job(job_id);
        state.batch_running.store(false, Ordering::SeqCst);

        let _ = task_app.emit(
            BATCH_DONE_EVENT,
            BatchDownloadDoneEvent::from_outcome(job_id, outcome),
        );
    });

    Ok(job_id)
}

/// Cancels `job_id`'s token so its batch worker stops before it starts its
/// next entry. The entry currently downloading (if any) still runs to
/// completion — this only prevents the *next* one from starting, same as
/// pausing after item 11 of a 100-item queue rather than interrupting item
/// 11 itself. Errors if no batch is running under that id (already
/// finished, or a stale id).
#[tauri::command]
pub async fn stop_download_all(job_id: u64, state: State<'_, AppState>) -> Result<(), String> {
    state.cancel_batch_job(job_id)
}

async fn run_batch<R: tauri::Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
    cancel: &CancellationToken,
) -> Result<BatchDownloadOutcome, String> {
    let store = state.queue_store()?;
    let transcoder = state.resolve_transcoder().await;
    let ctx = DownloadContext {
        stream_client: &state.stream_client,
        store,
        transcoder: transcoder.as_ref().map(|t| t as &dyn Transcode),
    };

    let progress_app = app.clone();
    let done_app = app.clone();

    download::run_all_queued(
        &ctx,
        cancel,
        move |progress| {
            let _ = progress_app.emit(PROGRESS_EVENT, DownloadProgressEvent::in_progress(progress));
        },
        move |queue_id, result| {
            let event = match result {
                Ok(progress) => DownloadProgressEvent::completed(progress),
                Err(e) => DownloadProgressEvent::failed(queue_id, e),
            };
            let _ = done_app.emit(PROGRESS_EVENT, event);
        },
    )
    .await
    .map_err(|e| e.to_string())
}
