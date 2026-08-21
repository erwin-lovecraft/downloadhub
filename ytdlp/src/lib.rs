//! Video metadata and downloads via an external `yt-dlp` process (not linked
//! as a library, same posture as `downloadhub-transcode`'s ffmpeg wrapper).
//! yt-dlp is actively maintained against YouTube's extraction changes, unlike
//! the InnerTube-client library this replaced.

use std::path::{Path, PathBuf};
use std::process::Stdio;

use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};

mod provider;

pub use provider::YtDlpProvider;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid video id or url: {0}")]
    InvalidVideoId(String),
    #[error("video unavailable: {0}")]
    VideoUnavailable(String),
    #[error(
        "YouTube is requiring sign-in verification (bot check) — add cookies in Settings to work around this"
    )]
    BotCheckRequired,
    #[error("no format matched the requested filter")]
    FormatNotFound,
    #[error("yt-dlp binary not found at {0}")]
    BinaryNotFound(PathBuf),
    #[error("failed to run yt-dlp: {0}")]
    Spawn(#[from] std::io::Error),
    #[error("failed to parse yt-dlp output: {0}")]
    Json(#[from] serde_json::Error),
    #[error("yt-dlp failed: {0}")]
    Process(String),
}

/// One downloadable stream variant. `itag` is yt-dlp's `format_id` parsed as
/// a number — true for virtually every real YouTube stream; formats with a
/// non-numeric id (storyboards, "sb0"/"sb1") are filtered out during
/// conversion from yt-dlp's JSON, so they never appear here.
#[derive(Debug, Clone, PartialEq)]
pub struct Format {
    pub itag: u32,
    pub ext: String,
    pub quality_label: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub fps: Option<u32>,
    /// Approximate bits per second (yt-dlp's `tbr`, which is in kbit/s).
    pub bitrate: Option<u64>,
    pub filesize_bytes: Option<u64>,
    pub vcodec: Option<String>,
    pub acodec: Option<String>,
}

impl Format {
    pub fn is_video(&self) -> bool {
        self.vcodec.as_deref().is_some_and(|c| c != "none")
    }

    pub fn has_audio(&self) -> bool {
        self.acodec.as_deref().is_some_and(|c| c != "none")
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Video {
    pub id: String,
    pub title: String,
    pub author: String,
    pub duration_secs: u64,
    pub formats: Vec<Format>,
}

impl Video {
    pub fn format_by_itag(&self, itag: u32) -> Option<&Format> {
        self.formats.iter().find(|f| f.itag == itag)
    }
}

#[derive(Debug, serde::Deserialize)]
struct RawVideoInfo {
    id: String,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    uploader: Option<String>,
    #[serde(default)]
    channel: Option<String>,
    #[serde(default)]
    duration: Option<f64>,
    #[serde(default)]
    formats: Vec<RawFormat>,
}

#[derive(Debug, serde::Deserialize)]
struct RawFormat {
    format_id: String,
    #[serde(default)]
    ext: Option<String>,
    #[serde(default)]
    vcodec: Option<String>,
    #[serde(default)]
    acodec: Option<String>,
    #[serde(default)]
    width: Option<u32>,
    #[serde(default)]
    height: Option<u32>,
    #[serde(default)]
    fps: Option<f64>,
    #[serde(default)]
    tbr: Option<f64>,
    #[serde(default)]
    filesize: Option<u64>,
    #[serde(default)]
    filesize_approx: Option<f64>,
    #[serde(default)]
    format_note: Option<String>,
}

impl From<RawVideoInfo> for Video {
    fn from(raw: RawVideoInfo) -> Self {
        Self {
            id: raw.id,
            title: raw.title.unwrap_or_default(),
            author: raw.uploader.or(raw.channel).unwrap_or_default(),
            duration_secs: raw.duration.map(|d| d.round() as u64).unwrap_or(0),
            formats: raw
                .formats
                .into_iter()
                .filter_map(|f| f.format_id.parse::<u32>().ok().map(|itag| (itag, f)))
                .map(|(itag, f)| Format {
                    itag,
                    ext: f.ext.unwrap_or_else(|| "bin".to_string()),
                    quality_label: f.format_note.or_else(|| f.height.map(|h| format!("{h}p"))),
                    width: f.width,
                    height: f.height,
                    fps: f.fps.map(|v| v.round() as u32),
                    bitrate: f.tbr.map(|kbps| (kbps * 1000.0).round() as u64),
                    filesize_bytes: f.filesize.or(f.filesize_approx.map(|v| v as u64)),
                    vcodec: f.vcodec,
                    acodec: f.acodec,
                })
                .collect(),
        }
    }
}

/// Where the yt-dlp binary is and (optionally) a cookies file to pass it —
/// both resolved by the caller (settings override, bundled sidecar, or
/// PATH; see `core::stream::resolve_ytdlp_config`). Cheap to construct;
/// every call spawns a fresh short-lived yt-dlp process.
#[derive(Debug, Clone)]
pub struct YtDlp {
    binary_path: PathBuf,
    cookies_path: Option<PathBuf>,
}

impl YtDlp {
    pub fn new(binary_path: impl Into<PathBuf>, cookies_path: Option<PathBuf>) -> Self {
        Self {
            binary_path: binary_path.into(),
            cookies_path,
        }
    }

    pub fn binary_path(&self) -> &Path {
        &self.binary_path
    }

    fn command(&self) -> tokio::process::Command {
        let mut cmd = tokio::process::Command::new(&self.binary_path);
        if let Some(cookies) = &self.cookies_path {
            cmd.arg("--cookies").arg(cookies);
        }
        cmd.arg("--no-warnings").arg("--no-playlist");
        cmd.stdin(Stdio::null());
        #[cfg(windows)]
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW: no console flash
        cmd
    }

    /// Fetches metadata and the full available format list for a video URL
    /// or bare 11-character id.
    pub async fn fetch_video(&self, url_or_id: &str) -> Result<Video, Error> {
        if !self.binary_path.is_file() {
            return Err(Error::BinaryNotFound(self.binary_path.clone()));
        }
        let target = normalize_target(url_or_id)?;

        let mut cmd = self.command();
        cmd.arg("-J")
            .arg("--skip-download")
            .arg(&target)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        let output = cmd.output().await?;
        if !output.status.success() {
            return Err(classify_error(&output.stderr));
        }
        let raw: RawVideoInfo = serde_json::from_slice(&output.stdout)?;
        Ok(raw.into())
    }

    /// Downloads `format_id` (a yt-dlp format id — pass the itag as a
    /// string) of `url_or_id` to the exact path `dest` (no output-template
    /// substitution: give it a literal final filename). `on_progress` is
    /// called with `(downloaded_bytes, total_bytes)` for every progress line
    /// yt-dlp emits — unthrottled, since throttling is a UI concern the
    /// caller owns. `total_bytes` is `0` when yt-dlp couldn't report a size.
    /// Returns the number of bytes written.
    pub async fn download(
        &self,
        url_or_id: &str,
        format_id: &str,
        dest: &Path,
        mut on_progress: impl FnMut(u64, u64) + Send,
    ) -> Result<u64, Error> {
        if !self.binary_path.is_file() {
            return Err(Error::BinaryNotFound(self.binary_path.clone()));
        }
        let target = normalize_target(url_or_id)?;

        let mut cmd = self.command();
        cmd.arg("-f")
            .arg(format_id)
            .arg("--newline")
            .arg("--progress-template")
            .arg("download:%(progress.downloaded_bytes)s %(progress.total_bytes,progress.total_bytes_estimate)s")
            .arg("-o")
            .arg(dest)
            .arg(&target)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        let mut child = cmd.spawn()?;
        let stdout = child.stdout.take().expect("stdout was piped");
        let mut stderr = child.stderr.take().expect("stderr was piped");

        let stderr_task = tokio::spawn(async move {
            let mut buf = String::new();
            let _ = stderr.read_to_string(&mut buf).await;
            buf
        });

        let mut lines = BufReader::new(stdout).lines();
        let mut last_downloaded = 0u64;
        while let Some(line) = lines.next_line().await? {
            if let Some((downloaded, total)) = parse_progress_line(&line) {
                last_downloaded = downloaded;
                on_progress(downloaded, total);
            }
        }

        let status = child.wait().await?;
        let stderr_text = stderr_task.await.unwrap_or_default();

        if !status.success() {
            return Err(classify_error(stderr_text.as_bytes()));
        }
        Ok(last_downloaded)
    }
}

fn parse_progress_line(line: &str) -> Option<(u64, u64)> {
    let rest = line.strip_prefix("download:")?;
    let mut parts = rest.split_whitespace();
    let downloaded = parts.next()?.parse::<u64>().ok()?;
    let total = parts
        .next()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);
    Some((downloaded, total))
}

fn classify_error(stderr: &[u8]) -> Error {
    let text = String::from_utf8_lossy(stderr).trim().to_string();
    if text.contains("Sign in to confirm") || text.contains("not a bot") {
        Error::BotCheckRequired
    } else if text.contains("Video unavailable")
        || text.contains("Private video")
        || text.contains("has been removed")
        || text.contains("This video is")
    {
        Error::VideoUnavailable(text)
    } else {
        Error::Process(text)
    }
}

/// Accepts a full YouTube URL (passed through as-is) or a bare 11-character
/// video id (expanded to a `watch` URL) — the same input shape the old
/// `y7dl`-based client accepted.
fn normalize_target(input: &str) -> Result<String, Error> {
    let trimmed = input.trim();
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        return Ok(trimmed.to_string());
    }
    let is_bare_id = trimmed.len() == 11
        && trimmed
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    if is_bare_id {
        Ok(format!("https://www.youtube.com/watch?v={trimmed}"))
    } else {
        Err(Error::InvalidVideoId(input.to_string()))
    }
}

/// Locates a yt-dlp binary without any configuration: next to the current
/// executable first (Tauri stages `externalBin` sidecars there in installed
/// builds), then on PATH (the dev fallback — `tauri dev` doesn't stage
/// sidecars next to the debug binary).
pub fn locate_ytdlp() -> Option<PathBuf> {
    let name = if cfg!(windows) {
        "yt-dlp.exe"
    } else {
        "yt-dlp"
    };

    if let Ok(exe) = std::env::current_exe() {
        if let Some(candidate) = exe.parent().map(|dir| dir.join(name)) {
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }

    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .map(|dir| dir.join(name))
            .find(|p| p.is_file())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_target_expands_a_bare_id() {
        assert_eq!(
            normalize_target("dQw4w9WgXcQ").unwrap(),
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ"
        );
    }

    #[test]
    fn normalize_target_passes_urls_through() {
        let url = "https://www.youtube.com/watch?v=dQw4w9WgXcQ&list=PL1";
        assert_eq!(normalize_target(url).unwrap(), url);
    }

    #[test]
    fn normalize_target_rejects_garbage() {
        assert!(normalize_target("not a video").is_err());
        assert!(normalize_target("short").is_err());
    }

    #[test]
    fn parse_progress_line_reads_both_numbers() {
        assert_eq!(
            parse_progress_line("download:1234 5678"),
            Some((1234, 5678))
        );
    }

    #[test]
    fn parse_progress_line_treats_missing_total_as_zero() {
        assert_eq!(parse_progress_line("download:1234 NA"), Some((1234, 0)));
    }

    #[test]
    fn parse_progress_line_ignores_unrelated_lines() {
        assert_eq!(parse_progress_line("[youtube] Extracting URL"), None);
    }

    #[test]
    fn classify_error_recognizes_bot_check() {
        assert!(matches!(
            classify_error(b"ERROR: [youtube] xyz: Sign in to confirm you're not a bot"),
            Error::BotCheckRequired
        ));
    }

    #[test]
    fn classify_error_recognizes_video_unavailable() {
        assert!(matches!(
            classify_error(b"ERROR: [youtube] xyz: Video unavailable"),
            Error::VideoUnavailable(_)
        ));
    }

    #[test]
    fn raw_video_info_drops_non_numeric_format_ids() {
        let raw = RawVideoInfo {
            id: "abc".to_string(),
            title: Some("Title".to_string()),
            uploader: Some("Author".to_string()),
            channel: None,
            duration: Some(125.4),
            formats: vec![
                RawFormat {
                    format_id: "137".to_string(),
                    ext: Some("mp4".to_string()),
                    vcodec: Some("avc1".to_string()),
                    acodec: Some("none".to_string()),
                    width: Some(1920),
                    height: Some(1080),
                    fps: Some(30.0),
                    tbr: Some(2500.0),
                    filesize: Some(1000),
                    filesize_approx: None,
                    format_note: None,
                },
                RawFormat {
                    format_id: "sb0".to_string(),
                    ext: Some("mhtml".to_string()),
                    vcodec: Some("none".to_string()),
                    acodec: Some("none".to_string()),
                    width: None,
                    height: None,
                    fps: None,
                    tbr: None,
                    filesize: None,
                    filesize_approx: None,
                    format_note: None,
                },
            ],
        };
        let video: Video = raw.into();
        assert_eq!(video.formats.len(), 1);
        assert_eq!(video.duration_secs, 125);
        let format = &video.formats[0];
        assert_eq!(format.itag, 137);
        assert!(format.is_video());
        assert!(!format.has_audio());
        assert_eq!(format.quality_label.as_deref(), Some("1080p"));
        assert_eq!(format.bitrate, Some(2_500_000));
    }
}
