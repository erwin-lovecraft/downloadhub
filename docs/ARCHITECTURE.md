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

### Video format/quality lookup

`core::stream` wraps `y7dl::Client` (`StreamClient`) behind a
`get_video_formats(video_id)` call used by the `get_video_formats` Tauri
command. `y7dl` isn't published to crates.io, so it's pulled in as a `git`
dependency in `core/Cargo.toml`, pinned to a specific commit (`rev`) rather
than tracking a branch, so builds stay reproducible.

`y7dl::Video`/`Format` derive `Deserialize` (they're built by parsing
YouTube's InnerTube response) but not `Serialize`, so they can't cross the
Tauri IPC boundary directly. `core::stream` maps them into `VideoDetail`/
`FormatSummary` DTOs that do derive `Serialize`, the same shape-translation
`core::youtube` already does for `VideoSummary`.

One `StreamClient` is created once in `AppState` and reused for the app's
lifetime (held in Tauri-managed state, same pattern as the YouTube API key)
rather than constructed per call: `y7dl::Client` caches parsed player JS and
pools HTTP connections internally, both of which are wasted if rebuilt on
every lookup.

### Download queue persistence

`core::queue` (`QueueStore`) persists queue entries to SQLite via
`rusqlite` with the `bundled` feature, rather than `sqlx`. Queue operations
are simple single-row CRUD with no need for `sqlx`'s async-native driver or
compile-time query checking, and `bundled` vendors SQLite's C source
directly (compiled by `cc`, already a build dependency transitively) so
there's no system SQLite install to require on any target platform —
verified by a clean build in this environment. `rusqlite::Connection` is
blocking, so every `QueueStore` method wraps its work in
`tokio::task::spawn_blocking` rather than blocking the async runtime
directly.

The database file lives at `<platform-data-dir>/downloadhub/queue.sqlite3`
(`dirs::data_dir()`, falling back to `dirs::home_dir()`), resolved and
opened once in `AppState::from_env` in `src-tauri`, not in `core` itself —
`core` stays platform-path-agnostic and just takes a `&Path` in
`QueueStore::open`. If the directory can't be created or the database can't
be opened, `AppState.queue_store` is `None` and queue commands report that
rather than the app failing to start, mirroring how a missing
`YOUTUBE_API_KEY`/OAuth config degrades instead of panicking.

`app` ↔ `mcp-server` shared state (see above) already commits to both
processes sharing this same SQLite file rather than one calling into the
other — this module is that shared source of truth.

### Output folder picker

The output-path field added in step 5 is filled via the official
`tauri-plugin-dialog`'s native folder picker (`open({ directory: true })`)
rather than a custom file-browser widget, plus the plugin's own permission
(`dialog:default` in `src-tauri/capabilities/default.json`) — same pattern
already used for `tauri-plugin-opener` (Google OAuth's browser launch). The
field stays freely editable as plain text too, so a folder can still be
pasted/typed directly.

`queue_entries.output_path` holds a destination *folder*, not a full file
path: `y7dl` doesn't mux, so a DASH download can produce two files
(`<title>.video.<ext>` + `<title>.audio.<ext>`) for one queue entry — the
download orchestrator (a later step) derives filenames from the video title
and format inside that folder rather than the queue storing a single
pre-decided filename.

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
