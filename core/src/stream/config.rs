//! Per-call yt-dlp configuration, resolved fresh from settings each time
//! (never cached) so a changed binary path or cookies take effect
//! immediately — the same freshness rule `ffmpeg_path` and `mcp_enabled`
//! already follow.

use std::path::{Path, PathBuf};

use crate::settings::AppSettings;

#[derive(Debug, Clone, Default)]
pub struct YtDlpConfig {
    /// `None` means "auto-locate" — the bundled sidecar next to the app
    /// executable, then PATH (see `downloadhub_ytdlp::locate_ytdlp`).
    pub binary_path: Option<PathBuf>,
    /// A Netscape-format cookies file to pass via `--cookies`, written from
    /// the settings' pasted cookie text. `None` when no cookies are
    /// configured.
    pub cookies_path: Option<PathBuf>,
}

/// Resolves a [`YtDlpConfig`] from `settings`, writing the pasted cookies
/// text (if any) to `<app_data_dir>/cookies.txt` so it can be handed to
/// yt-dlp as a file path. Called fresh on every search/download rather than
/// cached, so editing Settings takes effect without a restart.
pub async fn resolve_ytdlp_config(app_data_dir: &Path, settings: &AppSettings) -> YtDlpConfig {
    let binary_path = settings
        .ytdlp_path
        .as_deref()
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .map(PathBuf::from);

    let cookies_path = match settings.ytdlp_cookies.as_deref().map(str::trim) {
        Some(content) if !content.is_empty() => {
            let path = crate::paths::cookies_path(app_data_dir);
            match tokio::fs::write(&path, content).await {
                Ok(()) => Some(path),
                Err(e) => {
                    eprintln!("failed to write yt-dlp cookies file at {path:?}: {e}");
                    None
                }
            }
        }
        _ => None,
    };

    YtDlpConfig {
        binary_path,
        cookies_path,
    }
}
