//! Progress types and the throttled `AsyncWrite` wrapper that produces them.

use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

use tokio::io::{self, AsyncWrite};

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

/// Wraps a destination writer, invoking `on_progress` (throttled) after
/// each successful write so callers get incremental progress without
/// modifying `y7dl` itself.
pub(crate) struct ProgressWriter<'a, W, F> {
    pub(crate) inner: W,
    pub(crate) queue_id: i64,
    pub(crate) total_bytes: u64,
    pub(crate) written: u64,
    pub(crate) last_emit: Instant,
    pub(crate) on_progress: &'a mut F,
}

impl<W, F> AsyncWrite for ProgressWriter<'_, W, F>
where
    W: AsyncWrite + Unpin,
    F: FnMut(DownloadProgress),
{
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let this = self.get_mut();
        let poll = Pin::new(&mut this.inner).poll_write(cx, buf);
        if let Poll::Ready(Ok(n)) = &poll {
            this.written += *n as u64;
            let now = Instant::now();
            if now.duration_since(this.last_emit) >= PROGRESS_EMIT_INTERVAL {
                this.last_emit = now;
                (this.on_progress)(DownloadProgress {
                    queue_id: this.queue_id,
                    bytes_written: this.written,
                    total_bytes: this.total_bytes,
                    phase: DownloadPhase::Downloading,
                });
            }
        }
        poll
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().inner).poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().inner).poll_shutdown(cx)
    }
}
