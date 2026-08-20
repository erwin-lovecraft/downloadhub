# CLAUDE.md

Guidance for Claude Code (claude.ai/code) working in this repository.

Each crate has its own `CLAUDE.md` with the detail for that layer:
[`core/`](core/CLAUDE.md), [`src-tauri/`](src-tauri/CLAUDE.md),
[`mcp-server/`](mcp-server/CLAUDE.md), [`transcode/`](transcode/CLAUDE.md),
[`ytdlp/`](ytdlp/CLAUDE.md). Design decisions and their rationale live in
[`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).

## What this project is

An AI-powered YouTube downloader desktop app (Tauri + React): keyword search via
the YouTube Data API, playlist import, a SQLite-backed download queue,
downloads via an external `yt-dlp` process, and optional MP3 conversion via
ffmpeg. A separate MCP server binary lets external AI agents search and fill
the queue — but never start a download.

`core::auth` implements a Google OAuth installed-app/loopback flow with keychain
token storage, but **nothing currently calls it**: there are no `auth_*` Tauri
commands and no sign-in UI. Search runs on the API key alone. Treat the module
as dormant, not as a shipped feature.

## Workspace layout

Cargo workspace with five members plus a Vite/React frontend in top-level
`src/`:

| Path | Package | Role |
| --- | --- | --- |
| `core/` | `downloadhub-core` | Business logic. No `tauri` dependency. |
| `src-tauri/` | `downloadhub` | Tauri app: thin commands + event emission. |
| `transcode/` | `downloadhub-transcode` | ffmpeg process wrapper (MP3). |
| `ytdlp/` | `downloadhub-ytdlp` | yt-dlp process wrapper (metadata + download). |
| `mcp-server/` | `downloadhub-mcp-server` | MCP tools over stdio. |

`core` depends on `ytdlp` directly (metadata/download lookup isn't optional
the way transcoding is); `transcode` depends on `core`, implementing a trait
`core` defines, since MP3 conversion *is* optional and mcp-server never needs
it. Both binaries share one SQLite database rather than talking over IPC.

## Hard constraints

- **License:** the project is GPL-3.0-or-later (see [`LICENSE`](LICENSE)).
  `yt-dlp` and `ffmpeg` are external processes, not linked libraries — see
  `ytdlp/CLAUDE.md` and `transcode/CLAUDE.md` for why each is vendored the way
  it is. yt-dlp itself is Unlicense/public domain; the vendored ffmpeg build is
  GPL, which is why a GPL build was chosen for it. Do not add proprietary or
  closed dependencies without checking this still holds.
- **The MCP server must never be able to start a download.** This is enforced by
  the tool surface — no tool exists that starts a transfer — not by a runtime
  check. Do not add one. See `docs/ARCHITECTURE.md`, "Where the agent boundary
  sits".
- **Respect YouTube's terms of service.** Don't add features that circumvent
  rate limits or work around API restrictions beyond what yt-dlp already does.

## Commands

Task runner (`just`, at the workspace root — the preferred entry points; loads
`.env` into every recipe's environment via `set dotenv-load`):

- `just dev` — full Tauri app in dev mode
- `just release` — production installer: builds the `mcp-server` sidecar into
  `src-tauri/binaries/`, then `pnpm tauri build`, with `.env` credentials
  embedded at compile time
- `just sidecar` — the sidecar build/copy step alone

Frontend (pnpm, workspace root):

- `pnpm dev` — Vite dev server only
- `pnpm build` — `tsc` typecheck + Vite production build
- `pnpm tauri dev` / `pnpm tauri build` — note that `tauri build` does *not*
  stage the sidecar or load `.env`; use `just release` for shippable builds

Rust (from the repo root, against the workspace):

- `cargo build`
- `cargo clippy` / `cargo fmt` — must stay clean
- `cargo test`

## Conventions

- No `unwrap()`/`panic!` on I/O or network paths; use the `Result`-based error
  pattern already used throughout `core`.
- Prioritize unit tests for queue state transitions in `core/`; keep at least a
  smoke test for Tauri commands in `src-tauri/`.
- TypeScript strict mode is on; no `any` without a justifying comment.
- Frontend stack: TanStack Query, Zustand, Tailwind CSS, shadcn/ui. No test
  runner is configured on the frontend yet.
- Dev server is fixed to port 1420 (strict) with HMR on 1421; Vite ignores
  `src-tauri/` for watching.
- Commit incrementally with clear messages rather than one giant commit per
  feature.
- Keep comments and docs describing *what the code is and why*, not what changed
  or in what order things were built — that's what git history is for.

## Open work

- Concurrent downloads (configurable limit, default ~3). Today each
  `start_download` spawns an unbounded task; "Download all" is strictly
  sequential.
- A `docs/design/` export of the Fluent design reference. Until it exists, use
  clean shadcn/ui defaults rather than improvising a different visual direction.

## Setup

See [`README.md`](README.md) for the `.env` setup needed to exercise login and
search, and [`docs/MCP_SETUP.md`](docs/MCP_SETUP.md) for registering the MCP
server with Claude Desktop / Claude Code / Gemini CLI / Codex CLI.
