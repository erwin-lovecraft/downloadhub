//! Persisted app settings, as a JSON blob at a caller-provided path. A missing
//! file falls back to defaults; a corrupt one errors.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::stream::FormatPreference;

#[derive(Debug, thiserror::Error)]
pub enum SettingsError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("failed to read/write settings: {0}")]
    Serde(#[from] serde_json::Error),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppSettings {
    #[serde(default)]
    pub default_output_path: Option<String>,
    #[serde(default)]
    pub default_quality: FormatPreference,
    /// Whether the `mcp-server` binary serves tool calls. Defaults to
    /// enabled (also for settings files saved before this field existed);
    /// the server re-reads it on every call, so toggling takes effect
    /// without restarting anything. Even when enabled, queue-mutating MCP
    /// tools still require per-action user approval — this switch just
    /// turns agent access off wholesale.
    #[serde(default = "default_mcp_enabled")]
    pub mcp_enabled: bool,
    /// Custom path to an ffmpeg binary for MP3 conversion. `None` (the
    /// default, also for settings files saved before this field existed)
    /// falls back to the bundled sidecar (Windows) or an ffmpeg on PATH —
    /// see `AppState::resolve_transcoder` in `src-tauri`.
    #[serde(default)]
    pub ffmpeg_path: Option<String>,
    /// Custom path to a yt-dlp binary. `None` (the default) falls back to
    /// the bundled sidecar or a yt-dlp on PATH — see
    /// `core::stream::resolve_ytdlp_config`.
    #[serde(default)]
    pub ytdlp_path: Option<String>,
    /// Path to a Netscape `cookies.txt` file to hand yt-dlp via
    /// `--cookies`. YouTube sometimes demands sign-in verification
    /// ("confirm you're not a bot") before it will serve formats or
    /// streams; cookies from a signed-in browser session work around that.
    /// `None`/empty means no `--cookies` flag is passed.
    ///
    /// A *path* rather than the cookie text itself (which is what this used
    /// to be) because yt-dlp rewrites the file after every run to persist
    /// the cookies YouTube rotated. Owning a copy meant overwriting those
    /// refreshed cookies with the original paste on the next call, so a
    /// working export went stale within a few downloads. Pointing at the
    /// user's own file lets yt-dlp keep it current.
    #[serde(default)]
    pub ytdlp_cookies_path: Option<String>,
}

fn default_mcp_enabled() -> bool {
    true
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            default_output_path: None,
            default_quality: FormatPreference::default(),
            mcp_enabled: true,
            ffmpeg_path: None,
            ytdlp_path: None,
            ytdlp_cookies_path: None,
        }
    }
}

/// Loads settings from `path`, or `AppSettings::default()` if the file
/// doesn't exist yet.
pub async fn load(path: &Path) -> Result<AppSettings, SettingsError> {
    match tokio::fs::read_to_string(path).await {
        Ok(contents) => Ok(serde_json::from_str(&contents)?),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(AppSettings::default()),
        Err(e) => Err(e.into()),
    }
}

/// Converts a settings file still holding pasted cookie *text* (the
/// `ytdlp_cookies` field this replaced) into one pointing at a file: the
/// text is written to `<dir>/cookies.txt` and that path stored as
/// `ytdlp_cookies_path`. Idempotent — it does nothing once the legacy field
/// is gone, which it is as soon as this saves — and best-effort: a failure
/// leaves the file untouched and costs the user nothing but re-picking
/// their cookies file.
///
/// Called once at app startup rather than from [`load`], which every search
/// and download goes through and has no business rewriting settings.
pub async fn migrate_pasted_cookies(path: &Path) -> Result<(), SettingsError> {
    let Ok(contents) = tokio::fs::read_to_string(path).await else {
        return Ok(()); // no settings file yet: nothing to migrate
    };
    let mut raw: serde_json::Value = serde_json::from_str(&contents)?;
    let Some(object) = raw.as_object_mut() else {
        return Ok(());
    };
    let Some(text) = object.remove("ytdlp_cookies") else {
        return Ok(());
    };
    let text = text.as_str().unwrap_or_default().trim().to_string();

    let mut settings: AppSettings = serde_json::from_value(raw)?;
    let already_pointed = settings
        .ytdlp_cookies_path
        .as_deref()
        .map(str::trim)
        .is_some_and(|p| !p.is_empty());
    if !text.is_empty() && !already_pointed {
        let cookies_path =
            crate::paths::cookies_path(path.parent().unwrap_or_else(|| Path::new(".")));
        tokio::fs::write(&cookies_path, format!("{text}\n")).await?;
        settings.ytdlp_cookies_path = Some(cookies_path.to_string_lossy().into_owned());
    }
    save(path, &settings).await
}

pub async fn save(path: &Path, settings: &AppSettings) -> Result<(), SettingsError> {
    let json = serde_json::to_string_pretty(settings)?;
    tokio::fs::write(path, json).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn load_missing_file_returns_default() {
        let dir = std::env::temp_dir().join(format!("downloadhub-settings-test-{}", unique_id()));
        let path = dir.join("settings.json");
        assert_eq!(load(&path).await.unwrap(), AppSettings::default());
    }

    #[tokio::test]
    async fn save_then_load_roundtrips() {
        let dir = std::env::temp_dir().join(format!("downloadhub-settings-test-{}", unique_id()));
        tokio::fs::create_dir_all(&dir).await.unwrap();
        let path = dir.join("settings.json");

        let settings = AppSettings {
            default_output_path: Some("/tmp/downloads".to_string()),
            default_quality: FormatPreference::BestAudioOnly,
            mcp_enabled: false,
            ffmpeg_path: Some("/opt/homebrew/bin/ffmpeg".to_string()),
            ytdlp_path: Some("/opt/homebrew/bin/yt-dlp".to_string()),
            ytdlp_cookies_path: Some("/tmp/cookies.txt".to_string()),
        };
        save(&path, &settings).await.unwrap();

        let loaded = load(&path).await.unwrap();
        assert_eq!(loaded, settings);

        tokio::fs::remove_dir_all(&dir).await.ok();
    }

    #[tokio::test]
    async fn settings_file_predating_mcp_enabled_loads_as_enabled() {
        let dir = std::env::temp_dir().join(format!("downloadhub-settings-test-{}", unique_id()));
        tokio::fs::create_dir_all(&dir).await.unwrap();
        let path = dir.join("settings.json");
        tokio::fs::write(&path, br#"{"default_output_path": "/tmp/x"}"#)
            .await
            .unwrap();

        let loaded = load(&path).await.unwrap();
        assert!(loaded.mcp_enabled);

        tokio::fs::remove_dir_all(&dir).await.ok();
    }

    #[tokio::test]
    async fn load_corrupt_file_errors_rather_than_silently_discarding() {
        let dir = std::env::temp_dir().join(format!("downloadhub-settings-test-{}", unique_id()));
        tokio::fs::create_dir_all(&dir).await.unwrap();
        let path = dir.join("settings.json");
        tokio::fs::write(&path, b"not json").await.unwrap();

        assert!(load(&path).await.is_err());

        tokio::fs::remove_dir_all(&dir).await.ok();
    }

    #[tokio::test]
    async fn migration_moves_pasted_cookies_into_a_file() {
        let dir = std::env::temp_dir().join(format!("downloadhub-settings-test-{}", unique_id()));
        tokio::fs::create_dir_all(&dir).await.unwrap();
        let path = dir.join("settings.json");
        tokio::fs::write(
            &path,
            br##"{"ytdlp_cookies": "# Netscape HTTP Cookie File\n.youtube.com\tTRUE\t/\tTRUE\t0\tSID\tv"}"##,
        )
        .await
        .unwrap();

        migrate_pasted_cookies(&path).await.unwrap();

        let loaded = load(&path).await.unwrap();
        let cookies_path = loaded.ytdlp_cookies_path.expect("path recorded");
        assert_eq!(cookies_path, dir.join("cookies.txt").to_string_lossy());
        let written = tokio::fs::read_to_string(&cookies_path).await.unwrap();
        assert!(written.starts_with("# Netscape HTTP Cookie File"));
        // The legacy field is gone, so a second run is a no-op.
        let saved = tokio::fs::read_to_string(&path).await.unwrap();
        assert!(!saved.contains("ytdlp_cookies\""));

        tokio::fs::remove_dir_all(&dir).await.ok();
    }

    #[tokio::test]
    async fn migration_leaves_an_already_chosen_path_alone() {
        let dir = std::env::temp_dir().join(format!("downloadhub-settings-test-{}", unique_id()));
        tokio::fs::create_dir_all(&dir).await.unwrap();
        let path = dir.join("settings.json");
        tokio::fs::write(
            &path,
            br#"{"ytdlp_cookies": "stale text", "ytdlp_cookies_path": "/my/cookies.txt"}"#,
        )
        .await
        .unwrap();

        migrate_pasted_cookies(&path).await.unwrap();

        let loaded = load(&path).await.unwrap();
        assert_eq!(
            loaded.ytdlp_cookies_path.as_deref(),
            Some("/my/cookies.txt")
        );
        assert!(!dir.join("cookies.txt").exists());

        tokio::fs::remove_dir_all(&dir).await.ok();
    }

    #[tokio::test]
    async fn migration_is_a_no_op_without_a_settings_file() {
        let dir = std::env::temp_dir().join(format!("downloadhub-settings-test-{}", unique_id()));
        let path = dir.join("settings.json");
        migrate_pasted_cookies(&path).await.unwrap();
        assert!(!path.exists());
    }

    /// Unique suffix for test temp dirs so parallel test threads never
    /// collide on the same path. A per-process atomic counter, not a
    /// timestamp: tests can start close enough together that even
    /// nanosecond timestamps have collided in practice on some systems.
    fn unique_id() -> u64 {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        COUNTER.fetch_add(1, Ordering::Relaxed)
    }
}
