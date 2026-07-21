//! Shared filesystem locations. Lives in `core` (rather than `src-tauri`,
//! where it originated) because the desktop app and the `mcp-server` binary
//! must resolve the *same* app data directory to share the queue database
//! and settings file — see `docs/ARCHITECTURE.md`.

use std::path::{Path, PathBuf};

/// Resolves (and creates, if missing) `<platform-data-dir>/downloadhub`,
/// which holds `queue.sqlite3` and `settings.json`. `None` if no data
/// directory is available at all, or it couldn't be created.
pub fn app_data_dir() -> Option<PathBuf> {
    let dir = dirs::data_dir()
        .or_else(dirs::home_dir)?
        .join("downloadhub");
    if let Err(e) = std::fs::create_dir_all(&dir) {
        eprintln!("failed to create app data directory {dir:?}: {e}");
        return None;
    }
    Some(dir)
}

/// Where the queue database lives inside [`app_data_dir`].
pub fn queue_db_path(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("queue.sqlite3")
}

/// Where the settings file lives inside [`app_data_dir`].
pub fn settings_path(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("settings.json")
}

/// The OS "Downloads" folder, used as the last-resort destination when the
/// user hasn't configured a default output folder. `None` on a system with
/// no such folder, where the caller must ask for an explicit path instead.
pub fn downloads_dir() -> Option<PathBuf> {
    dirs::download_dir()
}
