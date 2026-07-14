# Architecture

## Cargo workspace layout

The project is a Cargo workspace with three members:

- **`core/`** (package `downloadhub-core`, lib name `downloadhub_core`) — pure
  business logic: YouTube Data API client, `y7dl` wrapper, queue manager,
  download orchestrator, SQLite persistence. Has no dependency on `tauri`.
  The package is named `downloadhub-core` rather than `core` because `core`
  is always present in Rust's extern prelude (it's the sysroot crate every
  Rust program implicitly links against); reusing that name for a workspace
  member causes ambiguous-name resolution errors. The directory is still
  named `core/` as specified.
- **`src-tauri/`** (package `downloadhub`, lib name `downloadhub_lib`) — this
  is the "app" layer described in the project spec: thin Tauri command
  handlers that call into `downloadhub-core` and emit events to the frontend
  for progress updates. The directory keeps the name `src-tauri` (rather
  than `app/`) because the Tauri CLI hardcodes that folder name when
  resolving `tauri.conf.json` (`tauri init --directory` defaults to it, and
  `-D/--frontend-dist` is documented as relative to `<project-dir>/src-tauri`)
  — renaming it would require fighting the CLI's config-resolution for no
  functional benefit. It depends on `downloadhub-core` via a path dependency.
- **`mcp-server/`** (package `downloadhub-mcp-server`, binary `mcp-server`)
  — Phase 3 binary exposing MCP tools over stdio/socket. Depends on
  `downloadhub-core` so it reuses the exact same queue manager and
  persistence as the desktop app rather than duplicating logic. Currently a
  stub (`main.rs` just prints a not-yet-implemented notice) — real tool
  implementations land in Phase 3.

`Cargo.toml` at the repo root is the workspace manifest (`members = ["core",
"src-tauri", "mcp-server"]`) and defines shared `[workspace.package]` values
(`version`, `edition`, `license`) that each member inherits with
`field.workspace = true`.

Cargo builds the whole workspace to a single `/target/` directory at the
repo root (not `src-tauri/target/`); the root `.gitignore` accounts for this.

### `app` ↔ `mcp-server` shared state (decide before Phase 3)

Both `src-tauri` and `mcp-server` need to operate on the same download
queue. The plan is for both to depend on `downloadhub-core`'s queue manager
and share the same SQLite database file rather than one process calling
into the other over local IPC — this avoids needing either binary to be
running for the other to make progress, and keeps `downloadhub-core` as the
single source of truth for queue state and transitions. This decision will
be revisited and finalized when Phase 3 work starts if a shared-DB approach
turns out to be insufficient (e.g. if live progress events need to reach the
MCP client while the desktop app is doing the actual downloading).

### YouTube Data API client

Decided: direct `reqwest` + `serde` calls against the REST API
(`core::youtube`), not the generated `google-youtube3` crate.
`search.list`/`videos.list` are plain API-key-authenticated GET requests
with a small response shape we only need a few fields from; the generated
client would additionally pull in `yup-oauth2` and its own
authenticator/hyper-connector setup to make the same calls — a second,
redundant OAuth stack alongside the `oauth2`-based one `core::auth` already
uses, for no benefit on unauthenticated API-key calls. `reqwest` was
already a `core` dependency for the Google userinfo call in `core::auth`.

`search.list` doesn't return video duration, so `search_videos` makes a
follow-up `videos.list` call (`part=contentDetails`) batched over all
result IDs and merges `contentDetails.duration` (ISO 8601, e.g. `PT4M13S`)
back in via a small hand-rolled parser (`core::youtube::duration`) — not
worth a dependency for a four-field, well-specified format.

### Muxing extension point

No `ffmpeg`/transcoding dependency yet. DASH adaptive downloads save
video-only and audio-only streams as separate, clearly labeled files. A
`Muxer` trait (or equivalent TODO-marked seam) will be added to
`downloadhub-core`'s download-completion path once a transcode crate is
selected, so muxing can be plugged in without a rewrite.

## License

This project depends on [`y7dl`](https://github.com/erwin-lovecraft/y7dl)
(GPL-3.0-or-later), so the whole project is licensed GPL-3.0-or-later — see
[`LICENSE`](../LICENSE). No proprietary/closed dependency may be added to
any crate that links against `y7dl` (i.e. `core/` and anything that depends
on it).
