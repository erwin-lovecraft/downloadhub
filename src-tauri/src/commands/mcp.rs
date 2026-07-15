//! Command exposing where the bundled `mcp-server` sidecar lives, so the
//! Settings dialog can show the user the exact path (and a ready-to-paste
//! MCP client config) to connect an external AI agent.
//!
//! The sidecar is bundled via `tauri.conf.json`'s `externalBin`, so at
//! runtime it sits next to the main app executable (Tauri strips the
//! target-triple suffix on bundling): `downloadhub.app/Contents/MacOS/`
//! on macOS, the install directory on Windows/Linux.

use std::path::PathBuf;

/// Absolute path to the bundled `mcp-server` binary, or a user-facing error
/// if the app's own executable location can't be resolved. The path is
/// computed from the running app, so it's correct wherever the user
/// installed it. Note the file only exists in a bundled/installed build —
/// under `tauri dev` the sidecar isn't copied next to the debug binary, so
/// this points at a not-yet-existing path (fine for showing the shape).
#[tauri::command]
pub fn mcp_server_path() -> Result<String, String> {
    let exe = std::env::current_exe()
        .map_err(|e| format!("couldn't resolve the app's own location: {e}"))?;
    let dir = exe
        .parent()
        .ok_or_else(|| "the app executable has no parent directory".to_string())?;
    let name = if cfg!(windows) {
        "mcp-server.exe"
    } else {
        "mcp-server"
    };
    let path: PathBuf = dir.join(name);
    Ok(path.to_string_lossy().into_owned())
}
