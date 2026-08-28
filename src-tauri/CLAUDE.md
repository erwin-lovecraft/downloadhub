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
  `download`, `playlist`, `settings`, `cookies`, `mcp`.

## Commands

| Module | Commands |
| --- | --- |
| `youtube` | `search_videos`, `list_playlist_items` |
| `video` | `get_video_formats` |
| `queue` | `add_to_queue`, `list_queue`, `remove_from_queue`, `clear_queue`, `set_queue_entry_format`, `set_queue_entries_quality` |
| `download` | `start_download`, `cancel_download`, `download_all`, `stop_download_all` |
| `playlist` | `import_playlist_to_queue` |
| `settings` | `get_settings`, `save_settings` |
| `cookies` | `check_ytdlp_cookies` |
| `mcp` | `mcp_server_path` |

Queue command names deliberately match the MCP tool names so both surfaces
expose the same operations under the same vocabulary. A new command must be
added to `generate_handler!` in `lib.rs` or it won't be callable.

There are no `auth_*` commands — `core::auth` is dormant.

## `AppState`

Holds, for the app's lifetime: the YouTube API key, one reused `StreamClient`
(constructed at startup with a `downloadhub_ytdlp::YtDlpProvider` — yt-dlp is a
subprocess spawned fresh per call, so there's no connection or cache to hold
onto; `StreamClient` itself just adds format-selection on top of whatever
`StreamProvider` it's given, see `core/CLAUDE.md`), the `QueueStore`
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
- `resolve_transcoder` and `resolve_ytdlp_config` both run **per call**, not
  once at startup, so changing the ffmpeg path, yt-dlp path, or cookies file
  in settings takes effect without a restart.

## Events

Two events. `download-progress` reports per-entry progress/completion/failure;
both `start_download` and `download_all` spawn onto `tauri::async_runtime` and
return immediately, so this is the only way either surfaces how a download
actually ended — `download_all` emits the same per-entry events via
`on_item_done`, so the frontend listener handles both unchanged. `download_all`
additionally emits `download-batch-done` once, when its spawned batch task
finishes, carrying the job id it originally returned plus the tallied
`BatchDownloadOutcome` — see `docs/ARCHITECTURE.md`, "One-worker job model,
and stopping mid-batch", for the job-id/`CancellationToken` registry behind
`stop_download_all`.

## Frontend

Lives in top-level `src/` (Vite/React), not here. TanStack Query owns server
state; small Zustand stores hold transient per-entry download progress and
batch-job status. `useQueue` polls every 3 seconds because `mcp-server` writes
the queue from
another *process*, which Tauri events can't reach across.

## Capabilities

New plugin permissions go in `capabilities/default.json`. Currently:
`dialog:default` (folder picker) and `opener:allow-open-path` (open a completed
download's folder).

## Sidecars

`mcp-server` (all platforms), `yt-dlp` (Windows and macOS), and `ffmpeg`
(Windows only) are declared as `externalBin`. `tauri.windows.conf.json` and
`tauri.macos.conf.json` JSON-*merge* over `tauri.conf.json`, so each platform
file's `externalBin` array must repeat `binaries/mcp-server`. Sidecars are
staged by `just sidecar`; `pnpm tauri build` alone does not stage them, and
`tauri dev` never does.

The yt-dlp macOS binary is vendored the same way ffmpeg's Windows one is
(`tools/yt-dlp_macos`, committed rather than fetched). Unlike the previously-
vendored ffmpeg macOS binary (removed after Gatekeeper flatly rejected it —
see `transcode/CLAUDE.md`), yt-dlp's release build carries an ad-hoc code
signature (`codesign -dv` shows `Signature=adhoc`), which changes the
Gatekeeper story: `spctl -a -vv` still reports it rejected, but in local
testing the *first* direct execution of a freshly-quarantined copy hung/was
killed (macOS running its on-demand security check) while every execution
after that succeeded normally, including runs from `tokio::process::Command`.
That's a rougher edge than "just works," but a materially different failure
mode than ffmpeg's outright block — worth re-verifying against a real signed
or notarized `.app` bundle before trusting this note further. Bundled anyway
because a user past that first hiccup gets a working sidecar with no extra
setup; anyone who doesn't falls back to the `ytdlp_path` setting or PATH,
same as ffmpeg on macOS.
