# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Current state

Phase 1 is in progress. The Cargo workspace split (`core` / `src-tauri` / `mcp-server`) described below is done, as are the root `LICENSE` and README attribution. Step 2 (Google OAuth login) is implemented: `core::auth` does the installed-app/loopback flow via `oauth2` + token storage via `keyring`, `src-tauri/src/commands/auth.rs` exposes `auth_login`/`auth_logout`/`auth_status` commands, and the frontend (Tailwind + shadcn/ui + TanStack Query) shows signed-in state. Step 3 (keyword search) is implemented: `core::youtube` calls `search.list`/`videos.list` directly via `reqwest` (see `docs/ARCHITECTURE.md` for why, over the generated client), exposed as the `search_videos` command, with a search box + results list (thumbnail/title/channel/duration) in the UI. Step 4 (video format/quality lookup) is implemented: `core::stream` wraps `y7dl::Client` (pulled in as a pinned `git` dependency — not published to crates.io) behind a `StreamClient` reused for the app's lifetime via `AppState`, exposed as the `get_video_formats` command, with a "View formats" action per search result opening a panel listing itag/quality/resolution/size for every available stream. Step 5 (download queue) is implemented: `core::queue` persists entries to SQLite via `rusqlite` (`bundled` feature, no system SQLite dependency), exposed as `add_to_queue`/`list_queue` commands (names chosen to match the Phase 3 MCP tool list) backed by one `QueueStore` opened once in `AppState` against `<platform-data-dir>/downloadhub/queue.sqlite3`; the UI has an output-path text field and an "Add to queue" button per format row in the video detail panel, plus a `QueuePanel` listing queued entries with status. Step 6 (output folder picker) is implemented: `tauri-plugin-dialog` (Rust + `@tauri-apps/plugin-dialog` JS binding, `dialog:default` capability) backs a "Browse..." button next to the output-path field that opens a native folder picker; the field stays freely editable as text too. Step 7 (ranged download execution with progress events) is implemented: `core::download`'s `run_download` re-fetches the video (stream URLs expire), resolves the queued itag, derives `<title>[.video|.audio].<ext>` from the format's mime type, and streams it via `StreamClient::download` (`y7dl`'s own ranged `Range`-request chunking) through a throttled `AsyncWrite` progress wrapper, transitioning the entry's `QueueStore` status as it runs; `core` stays Tauri-agnostic by taking a plain progress callback. `src-tauri/src/commands/download.rs`'s `start_download` command spawns that on `tauri::async_runtime` and emits `download-progress` events, which the frontend (`useDownloadProgressListener`, a small Zustand store) turns into a live progress bar per queue entry behind a "Start" button. No concurrency limit or resume support yet — both are explicitly Phase 2. See [`README.md`](README.md) for the `.env` setup needed to exercise login and search (format lookup, the queue, the folder picker, and downloading need no configuration). Step 8 (queue controls: start/cancel/remove/retry — "start" already landed as part of step 7) is not yet fully implemented.

## What this project is

An AI-powered YouTube downloader desktop app: Google OAuth login, keyword search via the YouTube Data API, a download queue (video + format + quality), and chunked downloads via `y7dl`. Phase 3 adds an MCP server so external AI agents can propose playlists/queue downloads subject to explicit user approval in the running app.

**License constraint:** the download engine depends on [`y7dl`](https://github.com/erwin-lovecraft/y7dl) (GPL-3.0-or-later). The whole project is GPL-licensed as a result — do not add proprietary/closed dependencies to any crate that links against `y7dl`. A root `LICENSE` (GPL-3.0-or-later) and attribution to `y7dl` and its upstream (`kkdai/youtube`) in the README are required.

## Target architecture (build toward this)

Cargo **workspace**, not a single Tauri crate:

- `core/` — pure business logic, no Tauri dependency: YouTube Data API client, `y7dl` wrapper, queue manager, download orchestrator, SQLite persistence.
- `app/` — the Tauri application: thin command handlers calling into `core`, event emission to the frontend for progress updates.
- `mcp-server/` — separate binary (Phase 3), also depends on `core`, exposes MCP tools over stdio/socket. Must reuse `app`'s queue manager (via shared local IPC or shared DB, not duplicated logic) — document the chosen approach in `docs/ARCHITECTURE.md`.

Frontend lives in top-level `src/` (current Vite/React layout).

Key Rust dependency choices (see `docs/ARCHITECTURE.md` once written for the actual decision made on the YouTube API client — official generated crate vs. direct `reqwest`+`serde`):
- `y7dl` for stream resolution and chunked download
- `oauth2` crate for Google OAuth (installed-app/loopback flow); tokens stored via `keyring` (OS keychain) — never plaintext
- `tokio` for async orchestration
- `sqlx` or `rusqlite` + SQLite for queue/history persistence
- No `ffmpeg`/muxing dependency yet — DASH adaptive downloads must save video-only and audio-only streams as separate, clearly labeled files (e.g. `title.video.webm` + `title.audio.webm`). Leave an explicit extension seam (e.g. a `Muxer` trait) in `core` for a future transcode crate rather than muxing inline.

UI reference: a Fluent-style design lives at a Claude Design link the user will export into `docs/design/` (screenshots or similar) — once present, treat it as the source of truth for layout/spacing/styling rather than improvising a different visual direction.

## Build order

Work proceeds in three phases and should **not be skipped ahead**; commit incrementally per numbered step with clear messages rather than delivering a phase in one giant commit:

1. **Phase 1 (MVP):** workspace scaffold → Google OAuth login → keyword search (`search.list`) → video format/quality lookup via `y7dl` → download queue (SQLite-backed) → output folder picker (Tauri `dialog` plugin) → ranged download execution with progress events → queue controls (start/cancel/remove/retry) → basic usable UI.
2. **Phase 2 (enhanced downloading):** playlist import (`playlistItems.list`) → concurrent downloads (configurable limit, default ~3) → resumable downloads via range requests → settings screen (output folder, default quality, concurrency, theme).
3. **Phase 3 (MCP):** `mcp-server` binary exposing `search_videos`, `get_video_info`, `add_to_queue`, `list_queue`, `start_download`, `get_download_status`. Any queue-mutating or download-starting tool call must land in a "pending agent action" state requiring explicit user approval in the running desktop app — the MCP server must never trigger downloads unattended. Document external-agent registration (Claude Desktop MCP config, Gemini/Codex equivalents) in `docs/MCP_SETUP.md`.

## Commands

Frontend (pnpm, workspace root):
- `pnpm dev` — Vite dev server only
- `pnpm build` — `tsc` typecheck + Vite production build
- `pnpm tauri dev` — full Tauri app in dev mode (spawns Vite via `beforeDevCommand`)
- `pnpm tauri build` — production desktop bundle

Rust (run from `src-tauri/` until the workspace split lands, then from repo root against the workspace):
- `cargo build`
- `cargo clippy` / `cargo fmt` — must be kept clean; no `unwrap()`/`panic!` on I/O or network paths, use the `Result`-based error pattern `y7dl` itself uses
- `cargo test` — once `core/` exists, prioritize unit tests for queue state transitions there; add at least a smoke test for Tauri commands in `app/`

No test runner is configured on the frontend yet.

## Conventions

- TypeScript strict mode is on (`tsconfig.json`); no `any` without a justifying comment.
- Frontend stack per the spec (not yet installed): TanStack Query, Zustand, Tailwind CSS, shadcn/ui.
- Respect YouTube's terms-of-service risk noted in `y7dl`'s own README — don't add features that circumvent rate limits or work around API restrictions beyond what `y7dl` already does.
- Dev server is fixed to port 1420 (strict) with HMR on 1421; Vite is configured to ignore `src-tauri/` for watching.
