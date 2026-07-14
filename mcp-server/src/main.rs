//! MCP server binary (Phase 3, not yet implemented).
//!
//! Will expose tools `search_videos`, `get_video_info`, `add_to_queue`,
//! `list_queue`, `start_download`, `get_download_status` over stdio/socket,
//! reusing `downloadhub_core`'s queue manager. Any tool call that mutates
//! the queue or starts a download must land in a "pending agent action"
//! state requiring explicit user approval in the running desktop app —
//! this binary must never trigger downloads unattended.

fn main() {
    eprintln!("downloadhub mcp-server: not yet implemented (see project Phase 3)");
}
