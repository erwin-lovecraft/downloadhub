//! Download execution: orchestration, progress reporting, output naming, and the
//! `Transcode` seam. No Tauri dependency — progress goes through a plain callback.

mod output;
mod progress;
mod runner;
mod transcode;

pub use progress::{DownloadPhase, DownloadProgress};
pub use runner::{
    run_all_queued, run_download, BatchDownloadOutcome, DownloadContext, DownloadError,
};
pub use transcode::{BoxError, Transcode};

/// Re-exported so callers (e.g. `src-tauri`'s batch-job registry) can hand
/// `run_all_queued` a token and cancel it later without taking their own
/// direct dependency on `tokio-util` just for this one type.
pub use tokio_util::sync::CancellationToken;
