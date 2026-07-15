//! Thin Tauri command handler that starts a queued download in the
//! background and streams its progress to the frontend as `download-progress`
//! events. All download logic lives in `downloadhub_core::download`; this
//! module just spawns it and forwards progress through Tauri's event system
//! (which `core` has no dependency on).

use crate::state::AppState;
use downloadhub_core::download::{self, DownloadProgress};
use downloadhub_core::queue::QueueStatus;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

const PROGRESS_EVENT: &str = "download-progress";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
enum DownloadStatusEvent {
    Downloading,
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
    fn downloading(progress: DownloadProgress) -> Self {
        Self {
            queue_id: progress.queue_id,
            bytes_written: progress.bytes_written,
            total_bytes: progress.total_bytes,
            status: DownloadStatusEvent::Downloading,
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

/// Starts a queued entry's download in the background and returns
/// immediately; progress/completion/failure are reported via
/// `download-progress` events rather than this command's return value.
#[tauri::command]
pub async fn start_download<R: tauri::Runtime>(
    queue_id: i64,
    app: AppHandle<R>,
    state: State<'_, AppState>,
) -> Result<(), String> {
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
        let stream_client = &state.stream_client;

        let progress_app = task_app.clone();
        let result = download::run_download(queue_id, stream_client, store, move |progress| {
            let _ = progress_app.emit(PROGRESS_EVENT, DownloadProgressEvent::downloading(progress));
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
