//! Download execution, split by responsibility:
//!
//! - [`runner`]: the orchestrator (`run_download`/`run_all_queued`) tying
//!   stream fetch, file write, status transitions, and transcoding together
//! - [`progress`]: progress types + the throttled `AsyncWrite` reporter
//! - [`output`]: destination-path/filename derivation
//! - [`transcode`]: the [`Transcode`] trait — the seam a concrete
//!   transcoder (the `downloadhub-transcode` crate) plugs into, so `core`
//!   never depends on one
//!
//! This module has no Tauri dependency: progress is reported through a
//! plain callback so the Tauri layer can wire it to event emission without
//! `core` knowing anything about Tauri's event system.

mod output;
mod progress;
mod runner;
mod transcode;

pub use progress::{DownloadPhase, DownloadProgress};
pub use runner::{
    run_all_queued, run_download, BatchDownloadOutcome, DownloadContext, DownloadError,
};
pub use transcode::{BoxError, Transcode};
