//! Progress types and the time-based throttle used while forwarding yt-dlp's
//! download callbacks.

use std::time::{Duration, Instant};

/// Which step of an entry's pipeline a progress report describes. There is
/// no percentage for `Transcoding` (ffmpeg's duration isn't predicted);
/// callers show it as an indeterminate "converting" state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownloadPhase {
    Downloading,
    Transcoding,
}

#[derive(Debug, Clone, Copy)]
pub struct DownloadProgress {
    pub queue_id: i64,
    pub bytes_written: u64,
    /// 0 when the stream's total size isn't known upfront.
    pub total_bytes: u64,
    pub phase: DownloadPhase,
}

pub(crate) const PROGRESS_EMIT_INTERVAL: Duration = Duration::from_millis(200);

/// Time-based rate limiter: `should_emit()` returns `true` at most once per
/// [`PROGRESS_EMIT_INTERVAL`], regardless of how often it's called — so a
/// caller getting a raw progress callback per yt-dlp output line can forward
/// a bounded rate to a UI event emitter.
pub(crate) struct Throttle {
    last_emit: Instant,
}

impl Throttle {
    pub(crate) fn new() -> Self {
        Self {
            last_emit: Instant::now() - PROGRESS_EMIT_INTERVAL,
        }
    }

    pub(crate) fn should_emit(&mut self) -> bool {
        let now = Instant::now();
        if now.duration_since(self.last_emit) >= PROGRESS_EMIT_INTERVAL {
            self.last_emit = now;
            true
        } else {
            false
        }
    }
}
