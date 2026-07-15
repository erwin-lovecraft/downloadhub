//! App-wide state read from the environment once at startup.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Mutex;

use downloadhub_core::auth::AuthConfig;
use downloadhub_core::queue::QueueStore;
use downloadhub_core::stream::StreamClient;

pub struct AppState {
    /// `None` means `GOOGLE_OAUTH_CLIENT_ID`/`_SECRET` weren't set; the app
    /// still runs, login just reports it isn't configured.
    pub auth_config: Option<AuthConfig>,
    /// `None` means `YOUTUBE_API_KEY` wasn't set; search reports the same.
    pub youtube_api_key: Option<String>,
    /// Reused across format lookups: caches parsed player JS and pools HTTP
    /// connections internally. Needs no configuration (no API key/OAuth).
    pub stream_client: StreamClient,
    /// `None` means the queue database couldn't be opened (e.g. no writable
    /// app data directory); queue commands report that instead of the app
    /// failing to start, matching how a missing API key/OAuth config
    /// degrades rather than panics.
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
    /// run while this is set, since `download_all` calls `run_download`
    /// directly rather than through the `running_downloads` registry —
    /// racing an individual command against the batch's own handling of
    /// the same entries wouldn't be safe (see `docs/ARCHITECTURE.md`,
    /// "Download all").
    pub batch_running: AtomicBool,
}

impl AppState {
    pub fn from_env() -> Self {
        let auth_config = match (
            std::env::var("GOOGLE_OAUTH_CLIENT_ID"),
            std::env::var("GOOGLE_OAUTH_CLIENT_SECRET"),
        ) {
            (Ok(client_id), Ok(client_secret)) => Some(AuthConfig {
                client_id,
                client_secret,
            }),
            _ => None,
        };
        let youtube_api_key = std::env::var("YOUTUBE_API_KEY").ok();
        // Shared with the mcp-server binary (same queue database and
        // settings file), so resolution lives in core::paths.
        let app_data_dir = downloadhub_core::paths::app_data_dir();

        Self {
            auth_config,
            youtube_api_key,
            stream_client: StreamClient::new(),
            queue_store: app_data_dir.as_deref().and_then(open_queue_store),
            settings_path: app_data_dir
                .as_deref()
                .map(downloadhub_core::paths::settings_path),
            running_downloads: Mutex::new(HashMap::new()),
            batch_running: AtomicBool::new(false),
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

    /// Errors out if a `download_all` batch is currently running. Shared by
    /// every command that individually starts, cancels, or removes a queue
    /// entry, since those would race with the batch's own handling of the
    /// same entries.
    pub fn ensure_no_batch_running(&self) -> Result<(), String> {
        if self.batch_running.load(std::sync::atomic::Ordering::SeqCst) {
            Err("A batch download is in progress; wait for it to finish.".to_string())
        } else {
            Ok(())
        }
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
