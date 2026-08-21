//! App-wide state read from the environment once at startup.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;

use downloadhub_core::download::CancellationToken;
use downloadhub_core::queue::QueueStore;
use downloadhub_core::stream::StreamClient;
use downloadhub_transcode::Transcoder;
use downloadhub_ytdlp::YtDlpProvider;

pub struct AppState {
    /// `None` means `YOUTUBE_API_KEY` wasn't set; search reports the same.
    pub youtube_api_key: Option<String>,
    /// Reused across format lookups: caches parsed player JS and pools HTTP
    /// connections internally. Needs no configuration (no API key).
    pub stream_client: StreamClient,
    /// `None` means the queue database couldn't be opened (e.g. no writable
    /// app data directory); queue commands report that instead of the app
    /// failing to start, matching how a missing API key degrades rather
    /// than panics.
    pub queue_store: Option<QueueStore>,
    /// Where `settings.json` lives, alongside `queue.sqlite3`. `None` for
    /// the same reason `queue_store` can be `None` (no writable app data
    /// directory); settings commands report that rather than panicking.
    pub settings_path: Option<PathBuf>,
    /// Handles for in-flight `start_download` tasks, keyed by queue entry
    /// id, so `cancel_download`/`remove_from_queue` can abort them. A
    /// missing entry just means nothing is currently running for that id
    /// (finished already, or never started).
    pub running_downloads: Mutex<HashMap<i64, tauri::async_runtime::JoinHandle<()>>>,
    /// True while `download_all` is sequentially processing the queue.
    /// `start_download`/`cancel_download`/`remove_from_queue` all refuse to
    /// run while this is set, since `download_all` runs its batch job
    /// directly against the store rather than through the
    /// `running_downloads` registry — racing an individual command against
    /// the batch's own handling of the same entries wouldn't be safe (see
    /// `docs/ARCHITECTURE.md`, "Download all").
    pub batch_running: AtomicBool,
    /// One-worker job registry for batch downloads: `download_all` spawns
    /// the batch as a background task (mirroring `start_download`'s
    /// fire-and-forget shape) and registers its `CancellationToken` here
    /// under a freshly allocated job id, so `stop_download_all(job_id)` can
    /// cancel it without needing a handle to the task itself. The task
    /// removes its own entry when it finishes, same as `running_downloads`.
    /// `batch_running` still gates concurrency to one job at a time — this
    /// map exists for lookup-by-id, not for tracking "is a batch running".
    batch_jobs: Mutex<HashMap<u64, CancellationToken>>,
    next_batch_job_id: AtomicU64,
}

impl AppState {
    pub fn from_env() -> Self {
        // The API key resolves from the runtime environment first (a local
        // `.env` via `dotenvy`), then falls back to the value embedded at
        // build time — see `downloadhub_core::secrets`.
        let youtube_api_key = downloadhub_core::secrets::youtube_api_key();
        // Shared with the mcp-server binary (same queue database and
        // settings file), so resolution lives in core::paths.
        let app_data_dir = downloadhub_core::paths::app_data_dir();

        Self {
            youtube_api_key,
            stream_client: StreamClient::new(YtDlpProvider::new()),
            queue_store: app_data_dir.as_deref().and_then(open_queue_store),
            settings_path: app_data_dir
                .as_deref()
                .map(downloadhub_core::paths::settings_path),
            running_downloads: Mutex::new(HashMap::new()),
            batch_running: AtomicBool::new(false),
            batch_jobs: Mutex::new(HashMap::new()),
            next_batch_job_id: AtomicU64::new(1),
        }
    }

    /// The queue database, or a user-facing error if it couldn't be opened
    /// at startup. Shared by every command that touches the queue.
    pub fn queue_store(&self) -> Result<&QueueStore, String> {
        self.queue_store.as_ref().ok_or_else(|| {
            "The download queue database is not available (couldn't be opened at startup — check the app's log output)."
                .to_string()
        })
    }

    /// The YouTube Data API key, or a user-facing error if it wasn't
    /// configured. Shared by every command that calls the YouTube API.
    pub fn youtube_api_key(&self) -> Result<String, String> {
        self.youtube_api_key.clone().ok_or_else(|| {
            "YouTube search is not configured. Set YOUTUBE_API_KEY (see README) and restart the app."
                .to_string()
        })
    }

    /// Where `settings.json` lives, or a user-facing error if no writable
    /// app data directory was found at startup.
    pub fn settings_path(&self) -> Result<&Path, String> {
        self.settings_path.as_deref().ok_or_else(|| {
            "Settings are not available (no writable app data directory found at startup)."
                .to_string()
        })
    }

    /// Resolves the ffmpeg wrapper for MP3 conversion, re-read on every
    /// download start so a settings change applies without restarting.
    /// Priority: the custom path from settings, then the bundled sidecar
    /// next to the app executable (Windows installers ship one), then
    /// PATH (the dev and macOS fallback). `None` means no ffmpeg found;
    /// entries flagged for conversion then fail with a clear message.
    /// A custom path is used as given (not existence-checked here) so a
    /// wrong one fails the download with an error naming that exact path.
    pub async fn resolve_transcoder(&self) -> Option<Transcoder> {
        if let Some(settings_path) = self.settings_path.as_deref() {
            if let Ok(settings) = downloadhub_core::settings::load(settings_path).await {
                if let Some(custom) = settings
                    .ffmpeg_path
                    .as_deref()
                    .map(str::trim)
                    .filter(|p| !p.is_empty())
                {
                    return Some(Transcoder::new(custom));
                }
            }
        }
        downloadhub_transcode::locate_ffmpeg().map(Transcoder::new)
    }

    /// Resolves yt-dlp's binary path override and cookies file, re-read on
    /// every search/download so a settings change (a new binary path,
    /// updated cookies) applies without restarting. Falls back to
    /// `YtDlpConfig::default()` — auto-locate the binary, no cookies — if
    /// settings can't be read at all (e.g. no writable app data directory).
    pub async fn resolve_ytdlp_config(&self) -> downloadhub_core::stream::YtDlpConfig {
        let Some(settings_path) = self.settings_path.as_deref() else {
            return downloadhub_core::stream::YtDlpConfig::default();
        };
        let Some(app_data_dir) = settings_path.parent() else {
            return downloadhub_core::stream::YtDlpConfig::default();
        };
        match downloadhub_core::settings::load(settings_path).await {
            Ok(settings) => {
                downloadhub_core::stream::resolve_ytdlp_config(app_data_dir, &settings).await
            }
            Err(_) => downloadhub_core::stream::YtDlpConfig::default(),
        }
    }

    /// Errors out if a `download_all` batch is currently running. Shared by
    /// every command that individually starts, cancels, or removes a queue
    /// entry, since those would race with the batch's own handling of the
    /// same entries.
    pub fn ensure_no_batch_running(&self) -> Result<(), String> {
        if self.batch_running.load(Ordering::SeqCst) {
            Err("A batch download is in progress; wait for it to finish.".to_string())
        } else {
            Ok(())
        }
    }

    /// Allocates a job id for a newly spawned batch task and registers its
    /// cancellation token under it. Call this before spawning the task so
    /// `stop_download_all` can find the job the instant `download_all`
    /// returns the id to its caller.
    pub fn register_batch_job(&self, cancel: CancellationToken) -> u64 {
        let job_id = self.next_batch_job_id.fetch_add(1, Ordering::SeqCst);
        self.batch_jobs
            .lock()
            .expect("batch jobs mutex poisoned")
            .insert(job_id, cancel);
        job_id
    }

    /// Cancels `job_id`'s token if it's still registered. `Err` means no
    /// batch is running under that id — either it already finished, or the
    /// id was never valid.
    pub fn cancel_batch_job(&self, job_id: u64) -> Result<(), String> {
        match self
            .batch_jobs
            .lock()
            .expect("batch jobs mutex poisoned")
            .get(&job_id)
        {
            Some(cancel) => {
                cancel.cancel();
                Ok(())
            }
            None => Err(
                "No batch download is running under that id (it may have already finished)."
                    .to_string(),
            ),
        }
    }

    /// Deregisters `job_id`. Called by the batch task itself once its run
    /// resolves, same as `running_downloads` entries removing themselves.
    pub fn finish_batch_job(&self, job_id: u64) {
        self.batch_jobs
            .lock()
            .expect("batch jobs mutex poisoned")
            .remove(&job_id);
    }
}

fn open_queue_store(app_data_dir: &Path) -> Option<QueueStore> {
    let db_path = downloadhub_core::paths::queue_db_path(app_data_dir);
    match QueueStore::open(&db_path) {
        Ok(store) => Some(store),
        Err(e) => {
            eprintln!("failed to open queue database at {db_path:?}: {e}");
            None
        }
    }
}
