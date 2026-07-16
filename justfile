# Task runner for downloadhub. `just dev` / `just release` are the two entry
# points; `just --list` shows everything.
#
# `dotenv-load` exports the workspace-root `.env` (gitignored) into the
# environment of every recipe, so `YOUTUBE_API_KEY` is visible to cargo when
# it compiles `core` — that is what lets `option_env!` in core/src/secrets.rs
# embed it into a release binary. Real environment variables (e.g. CI's GitHub
# Actions secrets) are never overridden by `.env`, and a missing `.env` is not
# an error (CI has none).

set dotenv-load := true
set windows-shell := ["powershell.exe", "-NoProfile", "-Command"]

# Rust target triple for the Tauri sidecar filename. Assumes the default
# toolchain per OS (MSVC on Windows); matches what `rustc -vV` reports there.
triple := arch() + if os() == "windows" { "-pc-windows-msvc" } else if os() == "macos" { "-apple-darwin" } else { "-unknown-linux-gnu" }
exe := if os() == "windows" { ".exe" } else { "" }

default:
    @just --list

# Run the desktop app in dev mode (spawns Vite via beforeDevCommand).
dev:
    pnpm tauri dev

# Credentials from `.env` (or real env vars in CI) are embedded at compile
# time; the installer lands in target/release/bundle/.

# Build the release installer: sidecar first, then the Tauri bundle.
release: sidecar
    pnpm tauri build

# The sidecar is copied to src-tauri/binaries/mcp-server-<triple>[.exe] so
# `tauri build` bundles it inside the desktop app — one installer ships both.

# Build the mcp-server binary and stage it as the Tauri sidecar.
[windows]
sidecar: _build-mcp-server
    New-Item -ItemType Directory -Force src-tauri/binaries | Out-Null
    Copy-Item target/release/mcp-server{{exe}} src-tauri/binaries/mcp-server-{{triple}}{{exe}}

[unix]
sidecar: _build-mcp-server
    mkdir -p src-tauri/binaries
    cp target/release/mcp-server{{exe}} src-tauri/binaries/mcp-server-{{triple}}{{exe}}

_build-mcp-server:
    cargo build --release -p downloadhub-mcp-server
