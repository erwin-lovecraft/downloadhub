//! Per-call yt-dlp configuration, resolved fresh from settings each time
//! (never cached) so a changed binary path or cookies file take effect
//! immediately — the same freshness rule `ffmpeg_path` and `mcp_enabled`
//! already follow.

use std::path::PathBuf;

use crate::settings::AppSettings;

#[derive(Debug, Clone, Default)]
pub struct YtDlpConfig {
    /// `None` means "auto-locate" — left to the `StreamProvider` impl (the
    /// bundled sidecar next to the app executable, then PATH; see
    /// `downloadhub_ytdlp::locate_ytdlp`, the concrete provider's resolver).
    pub binary_path: Option<PathBuf>,
    /// A Netscape `cookies.txt` file to pass via `--cookies`, owned by the
    /// user rather than copied here — yt-dlp rewrites it after every run to
    /// persist the cookies YouTube rotated, and a copy would throw those
    /// away on the next call. `None` when no cookies are configured.
    pub cookies_path: Option<PathBuf>,
}

/// Resolves a [`YtDlpConfig`] from `settings`. Both fields are used exactly
/// as the user gave them, without existence-checking: a wrong path should
/// fail the call with an error naming that exact path rather than silently
/// falling back to something else.
pub fn resolve_ytdlp_config(settings: &AppSettings) -> YtDlpConfig {
    YtDlpConfig {
        binary_path: non_empty_path(settings.ytdlp_path.as_deref()),
        cookies_path: non_empty_path(settings.ytdlp_cookies_path.as_deref()),
    }
}

fn non_empty_path(value: Option<&str>) -> Option<PathBuf> {
    value
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blank_settings_resolve_to_no_overrides() {
        let config = resolve_ytdlp_config(&AppSettings::default());
        assert!(config.binary_path.is_none());
        assert!(config.cookies_path.is_none());
    }

    #[test]
    fn whitespace_only_paths_count_as_unset() {
        let settings = AppSettings {
            ytdlp_path: Some("   ".to_string()),
            ytdlp_cookies_path: Some("".to_string()),
            ..AppSettings::default()
        };
        let config = resolve_ytdlp_config(&settings);
        assert!(config.binary_path.is_none());
        assert!(config.cookies_path.is_none());
    }

    #[test]
    fn configured_paths_are_passed_through_trimmed() {
        let settings = AppSettings {
            ytdlp_path: Some(" /usr/bin/yt-dlp ".to_string()),
            ytdlp_cookies_path: Some(" /home/me/cookies.txt ".to_string()),
            ..AppSettings::default()
        };
        let config = resolve_ytdlp_config(&settings);
        assert_eq!(
            config.binary_path.unwrap(),
            PathBuf::from("/usr/bin/yt-dlp")
        );
        assert_eq!(
            config.cookies_path.unwrap(),
            PathBuf::from("/home/me/cookies.txt")
        );
    }
}
