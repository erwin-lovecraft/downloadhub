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
