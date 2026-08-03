# CLAUDE.md

Guidance for Claude Code (claude.ai/code) working in this repository.

Each crate has its own `CLAUDE.md` with the detail for that layer:
[`core/`](core/CLAUDE.md), [`src-tauri/`](src-tauri/CLAUDE.md),
[`mcp-server/`](mcp-server/CLAUDE.md), [`transcode/`](transcode/CLAUDE.md).
Design decisions and their rationale live in
[`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).

## What this project is

An AI-powered YouTube downloader desktop app (Tauri + React): keyword search via
the YouTube Data API, playlist import, a SQLite-backed download queue, chunked
downloads via `y7dl`, and optional MP3 conversion via ffmpeg. A separate MCP
server binary lets external AI agents search and fill the queue — but never
start a download.

`core::auth` implements a Google OAuth installed-app/loopback flow with keychain
token storage, but **nothing currently calls it**: there are no `auth_*` Tauri
commands and no sign-in UI. Search runs on the API key alone. Treat the module
as dormant, not as a shipped feature.

## Workspace layout

Cargo workspace with four members plus a Vite/React frontend in top-level
`src/`:

| Path | Package | Role |
| --- | --- | --- |
| `core/` | `downloadhub-core` | Business logic. No `tauri` dependency. |
| `src-tauri/` | `downloadhub` | Tauri app: thin commands + event emission. |
| `transcode/` | `downloadhub-transcode` | ffmpeg process wrapper (MP3). |
| `mcp-server/` | `downloadhub-mcp-server` | MCP tools over stdio. |

Dependency arrows all point at `core`; `core` depends on none of the others.
Both binaries share one SQLite database rather than talking over IPC.

## Hard constraints

- **License:** the download engine depends on
  [`y7dl`](https://github.com/erwin-lovecraft/y7dl) (GPL-3.0-or-later), so the
  whole project is GPL-3.0-or-later. Do not add proprietary or closed
  dependencies to any crate linking against `y7dl`. Attribution to `y7dl` and
  its upstream (`kkdai/youtube`) in the README is required.
- **The MCP server must never be able to start a download.** This is enforced by
  the tool surface — no tool exists that starts a transfer — not by a runtime
  check. Do not add one. See `docs/ARCHITECTURE.md`, "Where the agent boundary
  sits".
- **Respect YouTube's terms of service** as noted in `y7dl`'s own README. Don't
  add features that circumvent rate limits or work around API restrictions
  beyond what `y7dl` already does.

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
  pattern `y7dl` itself uses.
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
- Resumable downloads via range requests. An interrupted download restarts from
  scratch.
- A `docs/design/` export of the Fluent design reference. Until it exists, use
  clean shadcn/ui defaults rather than improvising a different visual direction.

## Setup

See [`README.md`](README.md) for the `.env` setup needed to exercise login and
search, and [`docs/MCP_SETUP.md`](docs/MCP_SETUP.md) for registering the MCP
server with Claude Desktop / Claude Code / Gemini CLI / Codex CLI.
