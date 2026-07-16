//! Core business logic for downloadhub.
//!
//! This crate is intentionally free of any Tauri dependency so it can be
//! reused unmodified by both the `src-tauri` desktop app crate and the
//! `mcp-server` binary. Planned modules (added phase by phase, see the
//! project's build order):
//!
//! - `youtube`: YouTube Data API v3 client (search, playlists, metadata)
//! - `stream`: `y7dl` wrapper for format/quality resolution and download
//! - `queue`: download queue state machine + SQLite persistence
//! - `download`: download orchestrator, progress reporting, resume support
//! - `auth`: Google OAuth token acquisition/storage (via `keyring`)
//! - `agent`: pending agent actions (MCP requests awaiting user approval)
//! - `paths`: shared app-data-dir resolution for both binaries
//! - `secrets`: credential resolution (runtime env, then compile-time embed)

pub mod agent;
pub mod auth;
pub mod download;
/// Re-export of the `downloadhub-transcode` crate (ffmpeg wrapper), so the
/// binaries can construct a `Transcoder` without their own Cargo dependency.
pub use downloadhub_transcode as transcode;
pub mod paths;
pub mod playlist;
pub mod queue;
pub mod secrets;
pub mod settings;
pub mod stream;
pub mod youtube;
