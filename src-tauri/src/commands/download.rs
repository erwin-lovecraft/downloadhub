//! Thin Tauri command handler that starts a queued download in the
//! background and streams its progress to the frontend as `download-progress`
//! events. All download logic lives in `downloadhub_core::download`; this
//! module just spawns it and forwards progress through Tauri's event system
//! (which `core` has no dependency on).

use crate::state::AppState;
use downloadhub_core::download::{self, DownloadProgress};
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
    if state.queue_store.is_none() {
        return Err(
            "The download queue database is not available (couldn't be opened at startup — check the app's log output)."
                .to_string(),
        );
    }

    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        let Some(store) = state.queue_store.as_ref() else {
            return;
        };
        let stream_client = &state.stream_client;

        let progress_app = app.clone();
        let result = download::run_download(queue_id, stream_client, store, move |progress| {
            let _ = progress_app.emit(PROGRESS_EVENT, DownloadProgressEvent::downloading(progress));
        })
        .await;

        let event = match result {
            Ok(progress) => DownloadProgressEvent::completed(progress),
            Err(e) => DownloadProgressEvent::failed(queue_id, e.to_string()),
        };
        let _ = app.emit(PROGRESS_EVENT, event);
    });

    Ok(())
}
