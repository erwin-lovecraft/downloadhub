//! Tauri command for checking a yt-dlp cookies file actually works.
//!
//! "Cookies are configured" and "cookies do anything" are different things:
//! a file whose tabs became spaces loads with every entry skipped, expired
//! entries are dropped without a word, and cookies exported from a session
//! the user kept browsing get invalidated by YouTube. All three end as the
//! same bot-check failure at download time, long after the Settings dialog
//! said nothing was wrong. This checks both halves up front — the file's
//! shape, then a real request through yt-dlp.

use std::path::PathBuf;

use downloadhub_core::stream::{inspect_cookie_file, StreamError, YtDlpConfig};
use tauri::State;

use crate::state::AppState;

/// The video the live probe requests: "Me at the zoo", the oldest video on
/// YouTube. Public, 19 seconds, and about as unlikely to be taken down as a
/// video gets — the probe only needs metadata, so nothing is downloaded.
const PROBE_VIDEO_ID: &str = "jNQXAC9IVRw";

#[derive(Debug, serde::Serialize)]
pub struct CookieCheck {
    /// Whether yt-dlp got a signed-in-looking response through: the file
    /// parsed *and* YouTube served the probe without a bot check.
    pub ok: bool,
    pub summary: String,
    /// Everything wrong with the file itself, from
    /// `CookieFileReport::problems`. Non-empty with `ok: true` is possible
    /// and worth showing: YouTube isn't challenging this request, but the
    /// file is weaker than the user thinks.
    pub problems: Vec<String>,
}

/// Checks the cookies file at `path` (the settings value, not yet saved —
/// so the user can test before committing to it).
#[tauri::command]
pub async fn check_ytdlp_cookies(
    path: String,
    state: State<'_, AppState>,
) -> Result<CookieCheck, String> {
    let path = path.trim();
    if path.is_empty() {
        return Err("Choose a cookies.txt file first.".to_string());
    }
    let cookies_path = PathBuf::from(path);

    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let report = inspect_cookie_file(&cookies_path, now_secs)
        .map_err(|e| format!("Can't read {path}: {e}"))?;
    let problems = report.problems();

    // A file yt-dlp would reject outright, or one with nothing left to
    // send, can't tell us anything more from a live request.
    if !report.has_netscape_header || report.usable_entries == 0 {
        return Ok(CookieCheck {
            ok: false,
            summary: "These cookies won't work.".to_string(),
            problems,
        });
    }

    // Deliberately the file the user picked, not a copy: yt-dlp rewrites it
    // with whatever YouTube rotated, which is the point of storing a path.
    let config = YtDlpConfig {
        binary_path: state.resolve_ytdlp_config().await.binary_path,
        cookies_path: Some(cookies_path),
    };
    match state
        .stream_client
        .get_video_formats(PROBE_VIDEO_ID, &config)
        .await
    {
        Ok(video) => Ok(CookieCheck {
            ok: true,
            summary: format!(
                "Cookies work — YouTube served {} formats without a sign-in challenge ({} cookie(s) loaded).",
                video.formats.len(),
                report.usable_entries
            ),
            problems,
        }),
        Err(StreamError::BotCheckRequired) => Ok(CookieCheck {
            ok: false,
            summary: "YouTube still asked for sign-in verification, so these cookies aren't being accepted."
                .to_string(),
            problems: with_stale_session_hint(problems),
        }),
        Err(e) => Err(e.to_string()),
    }
}

/// The remaining explanation once the file itself checks out: YouTube
/// invalidates cookies exported from a session that keeps browsing, which
/// no amount of inspecting the file can detect.
fn with_stale_session_hint(mut problems: Vec<String>) -> Vec<String> {
    problems.push(
        "The file looks fine, so the session behind it is most likely expired. Export again from a private/incognito window: sign in, export, then close the window without signing out — YouTube rotates cookies of a session you keep using."
            .to_string(),
    );
    problems
}
