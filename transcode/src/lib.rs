//! Audio transcoding via an external `ffmpeg` process (not linked as a library).
//! The path comes from the caller: a settings override, the bundled sidecar, or
//! PATH.

use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::process::Stdio;

use downloadhub_core::download::{BoxError, Transcode};

#[derive(Debug, thiserror::Error)]
pub enum TranscodeError {
    #[error("ffmpeg binary not found at {0}")]
    FfmpegNotFound(PathBuf),
    #[error("failed to run ffmpeg: {0}")]
    Spawn(#[from] std::io::Error),
    #[error("ffmpeg exited with {status}: {stderr}")]
    Ffmpeg {
        status: std::process::ExitStatus,
        stderr: String,
    },
}

/// Wraps one ffmpeg binary path. Cheap to construct and clone; every call
/// spawns a fresh short-lived ffmpeg process.
#[derive(Debug, Clone)]
pub struct Transcoder {
    ffmpeg_path: PathBuf,
}

impl Transcoder {
    pub fn new(ffmpeg_path: impl Into<PathBuf>) -> Self {
        Self {
            ffmpeg_path: ffmpeg_path.into(),
        }
    }

    pub fn ffmpeg_path(&self) -> &Path {
        &self.ffmpeg_path
    }

    /// Transcodes `input` (anything ffmpeg can demux; in practice the m4a
    /// itag-140 stream) to an MP3 at `output`, using LAME VBR quality 2
    /// (~190 kbps) — transparent for a 128 kbps AAC source without wasting
    /// space on a fixed 320k bitrate. Overwrites `output` if it exists;
    /// `input` is left in place for the caller to delete.
    pub async fn to_mp3(&self, input: &Path, output: &Path) -> Result<(), TranscodeError> {
        if !self.ffmpeg_path.is_file() {
            return Err(TranscodeError::FfmpegNotFound(self.ffmpeg_path.clone()));
        }

        let mut cmd = tokio::process::Command::new(&self.ffmpeg_path);
        cmd.arg("-y")
            .arg("-hide_banner")
            .arg("-loglevel")
            .arg("error")
            .arg("-i")
            .arg(input)
            .arg("-vn")
            .arg("-codec:a")
            .arg("libmp3lame")
            .arg("-q:a")
            .arg("2")
            .arg(output)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            // If the future driving this is dropped (download cancelled),
            // kill the child instead of orphaning a running ffmpeg.
            .kill_on_drop(true);
        #[cfg(windows)]
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW: no console flash

        let result = cmd.output().await?;
        if result.status.success() {
            Ok(())
        } else {
            Err(TranscodeError::Ffmpeg {
                status: result.status,
                stderr: String::from_utf8_lossy(&result.stderr).trim().to_string(),
            })
        }
    }
}

/// The `core`-facing seam: `core::download` orchestrates *when* a
/// conversion happens through this trait, without depending on this crate.
impl Transcode for Transcoder {
    fn to_mp3<'a>(
        &'a self,
        input: &'a Path,
        output: &'a Path,
    ) -> Pin<Box<dyn Future<Output = Result<(), BoxError>> + Send + 'a>> {
        Box::pin(async move {
            Transcoder::to_mp3(self, input, output)
                .await
                .map_err(BoxError::from)
        })
    }
}

/// Locates an ffmpeg binary without any configuration: next to the current
/// executable first (Tauri stages `externalBin` sidecars there in installed
/// builds), then on PATH (the dev fallback — `tauri dev` doesn't stage
/// sidecars next to the debug binary).
pub fn locate_ffmpeg() -> Option<PathBuf> {
    let name = if cfg!(windows) {
        "ffmpeg.exe"
    } else {
        "ffmpeg"
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

    #[tokio::test]
    async fn missing_binary_reports_ffmpeg_not_found() {
        let transcoder = Transcoder::new("/nonexistent/ffmpeg");
        let err = transcoder
            .to_mp3(Path::new("in.m4a"), Path::new("out.mp3"))
            .await
            .unwrap_err();
        assert!(matches!(err, TranscodeError::FfmpegNotFound(_)));
    }

    // Exercising the spawn/exit-status paths needs an executable, not a
    // real ffmpeg — a tiny shell script standing in for it keeps these
    // tests hermetic. Windows has no equivalent one-liner executable, so
    // they're unix-only; the missing-binary test above still runs there.
    #[cfg(unix)]
    mod with_fake_ffmpeg {
        use super::*;
        use std::os::unix::fs::PermissionsExt;

        struct FakeFfmpeg {
            dir: PathBuf,
        }

        impl FakeFfmpeg {
            fn new(name: &str, script: &str) -> Self {
                let dir = std::env::temp_dir().join(format!(
                    "downloadhub-transcode-test-{}-{name}",
                    std::process::id()
                ));
                std::fs::create_dir_all(&dir).unwrap();
                let path = dir.join("ffmpeg");
                std::fs::write(&path, script).unwrap();
                std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
                Self { dir }
            }

            fn path(&self) -> PathBuf {
                self.dir.join("ffmpeg")
            }
        }

        impl Drop for FakeFfmpeg {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.dir);
            }
        }

        #[tokio::test]
        async fn successful_exit_is_ok() {
            let fake = FakeFfmpeg::new("ok", "#!/bin/sh\nexit 0\n");
            let transcoder = Transcoder::new(fake.path());
            transcoder
                .to_mp3(Path::new("in.m4a"), Path::new("out.mp3"))
                .await
                .unwrap();
        }

        #[tokio::test]
        async fn failing_exit_surfaces_stderr() {
            let fake = FakeFfmpeg::new(
                "fail",
                "#!/bin/sh\necho 'in.m4a: Invalid data found' >&2\nexit 1\n",
            );
            let transcoder = Transcoder::new(fake.path());
            let err = transcoder
                .to_mp3(Path::new("in.m4a"), Path::new("out.mp3"))
                .await
                .unwrap_err();
            match err {
                TranscodeError::Ffmpeg { stderr, .. } => {
                    assert!(stderr.contains("Invalid data found"), "stderr: {stderr}");
                }
                other => panic!("expected Ffmpeg error, got {other:?}"),
            }
        }
    }
}
