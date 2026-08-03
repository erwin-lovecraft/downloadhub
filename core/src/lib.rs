//! Core business logic for downloadhub. Free of any Tauri dependency so both
//! the `src-tauri` desktop app and the `mcp-server` binary can reuse it.

pub mod auth;
pub mod download;
pub mod enqueue;
pub mod paths;
pub mod queue;
pub mod secrets;
pub mod settings;
pub mod stream;
pub mod youtube;
