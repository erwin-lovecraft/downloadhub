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

# The root VERSION file is the single source of truth; scripts/bump-version.mjs
# copies it into the three places a version is otherwise duplicated:
# package.json, the workspace `[workspace.package] version` in Cargo.toml
# (which every crate's `version.workspace = true` inherits), and
# src-tauri/tauri.conf.json (what Tauri actually builds/updates against —
# package.json's copy is otherwise unused). Doesn't commit — review with
# `git diff` and commit yourself, matching this repo's existing one-line
# "Update version to X.Y.Z" commits.

# Bump the app version everywhere it's declared. No argument bumps the patch
# number; pass "major"/"minor"/"patch" or an explicit "x.y.z" to control it.
# E.g. `just bump-version`, `just bump-version minor`, `just bump-version 2.0.0`.
bump-version version="":
    node scripts/bump-version.mjs {{version}}
    cargo update -p downloadhub -p downloadhub-core -p downloadhub-transcode -p downloadhub-mcp-server

# Sidecars live at src-tauri/binaries/<name>-<triple>[.exe] so `tauri
# build` bundles them inside the desktop app — one installer ships
# everything. mcp-server is built from this workspace; ffmpeg (used for
# the MP3 transcode step) is a static GPL build vendored in tools/ —
# deliberately a committed binary rather than a fetch-at-build-time
# dependency (see README "MP3 conversion (ffmpeg sidecar)"). Windows only
# for now (`tauri.windows.conf.json` adds it to `externalBin` there): a
# vendored unsigned macOS binary gets blocked by Gatekeeper, so macOS
# relies on the custom ffmpeg path setting or PATH instead.

# Build the mcp-server binary and stage it (plus, on Windows, the vendored
# ffmpeg) as Tauri sidecars.
[windows]
sidecar: _build-mcp-server
    New-Item -ItemType Directory -Force src-tauri/binaries | Out-Null
    Copy-Item target/release/mcp-server{{exe}} src-tauri/binaries/mcp-server-{{triple}}{{exe}}
    Copy-Item tools/ffmpeg-windows-x86_64.exe src-tauri/binaries/ffmpeg-{{triple}}{{exe}}

[unix]
sidecar: _build-mcp-server
    mkdir -p src-tauri/binaries
    cp target/release/mcp-server{{exe}} src-tauri/binaries/mcp-server-{{triple}}{{exe}}

_build-mcp-server:
    cargo build --release -p downloadhub-mcp-server
