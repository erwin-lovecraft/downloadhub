//! Persisted app settings (default output folder, default quality).
//!
//! A small JSON blob at a path the caller provides — `core` stays
//! platform-path-agnostic, same as `QueueStore` (see `AppState` in
//! `src-tauri` for where that path actually comes from). A missing file
//! (normal on first run) falls back to [`AppSettings::default`] rather
//! than erroring; a present-but-corrupt file still errors, since silently
//! discarding a user's saved settings would be surprising.

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

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AppSettings {
    #[serde(default)]
    pub default_output_path: Option<String>,
    #[serde(default)]
    pub default_quality: FormatPreference,
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
        };
        save(&path, &settings).await.unwrap();

        let loaded = load(&path).await.unwrap();
        assert_eq!(loaded, settings);

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
