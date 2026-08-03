# src-tauri/ — `downloadhub`

The Tauri application layer. Command handlers are **thin**: they wire arguments
into `downloadhub-core`, map its errors to `String`, and emit events. Logic that
isn't Tauri-specific belongs in `core`.

The directory keeps the name `src-tauri` (rather than `app/`) because the Tauri
CLI hardcodes that folder when resolving `tauri.conf.json`.

## Layout

- `lib.rs` — builder configuration and `generate_handler!` registration. `run()`
  additionally installs the updater/process plugins; `configure()` is kept
  separate so tests can drive it against a mock context with no updater config.
- `main.rs` — binary entry point.
- `state.rs` — `AppState`: everything the app holds for its lifetime.
- `commands/` — one module per surface: `youtube`, `video`, `queue`,
  `download`, `playlist`, `settings`, `mcp`.

## Commands

| Module | Commands |
| --- | --- |
| `youtube` | `search_videos`, `list_playlist_items` |
| `video` | `get_video_formats` |
| `queue` | `add_to_queue`, `list_queue`, `remove_from_queue`, `clear_queue`, `set_queue_entry_format`, `set_queue_entries_quality` |
| `download` | `start_download`, `cancel_download`, `download_all` |
| `playlist` | `import_playlist_to_queue` |
| `settings` | `get_settings`, `save_settings` |
| `mcp` | `mcp_server_path` |

Queue command names deliberately match the MCP tool names so both surfaces
expose the same operations under the same vocabulary. A new command must be
added to `generate_handler!` in `lib.rs` or it won't be callable.

There are no `auth_*` commands — `core::auth` is dormant.

## `AppState`

Holds, for the app's lifetime: the YouTube API key, one reused `StreamClient`
(`y7dl::Client` caches parsed player JS and pools connections), the `QueueStore`
(`Option` — if the data dir or DB can't be opened, queue commands report it
rather than the app failing to start), the app data dir, a
`Mutex<HashMap<queue_id, JoinHandle>>` of in-flight downloads, and
`batch_running: AtomicBool`.

Two invariants worth preserving:

- `batch_running` is **swapped**, never checked-then-set, to close the race
  between two concurrent `download_all` calls.
  `start_download`/`cancel_download`/`remove_from_queue`/the re-format commands
  all call `ensure_no_batch_running` first, because `download_all` bypasses the
  handle registry and a per-entry command racing it has no safe outcome.
- `resolve_transcoder` runs **per download start**, not once at startup, so
  changing the ffmpeg path in settings takes effect without a restart.

## Events

`download-progress` is the only event. `start_download` spawns onto
`tauri::async_runtime` and returns immediately — its `Result` says whether the
download could be *started*, not how it ended — while `download_all` awaits the
whole batch and emits the same per-entry events via `on_item_done`, so the
frontend listener handles both unchanged.

## Frontend

Lives in top-level `src/` (Vite/React), not here. TanStack Query owns server
state; a small Zustand store holds transient per-entry download progress.
`useQueue` polls every 3 seconds because `mcp-server` writes the queue from
another *process*, which Tauri events can't reach across.

## Capabilities

New plugin permissions go in `capabilities/default.json`. Currently:
`dialog:default` (folder picker) and `opener:allow-open-path` (open a completed
download's folder).

## Sidecars

`mcp-server` (all platforms) and `ffmpeg` (Windows only) are declared as
`externalBin`. `tauri.windows.conf.json` JSON-*merges* over `tauri.conf.json`,
so its `externalBin` array must repeat `binaries/mcp-server`. Sidecars are
staged by `just sidecar`; `pnpm tauri build` alone does not stage them, and
`tauri dev` never does.
