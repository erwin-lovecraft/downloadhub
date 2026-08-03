# Architecture

A record of the design decisions behind downloadhub, written as "why it is
this way" rather than "how it got here".

## Cargo workspace layout

Four members:

- **`core/`** (package `downloadhub-core`, lib `downloadhub_core`) — business
  logic with no `tauri` dependency: YouTube Data API client, `y7dl` wrapper,
  queue manager, download orchestrator, SQLite persistence. Named
  `downloadhub-core` rather than `core` because `core` is always in Rust's
  extern prelude and reusing the name causes ambiguous-name resolution errors;
  the *directory* is still `core/`.
- **`src-tauri/`** (package `downloadhub`, lib `downloadhub_lib`) — the app
  layer: thin Tauri command handlers calling into `core`, plus event emission
  to the frontend. Keeps the name `src-tauri` because the Tauri CLI hardcodes
  that folder when resolving `tauri.conf.json`.
- **`transcode/`** (package `downloadhub-transcode`) — audio transcoding via an
  external `ffmpeg` binary. A separate crate so the process-spawning boundary
  stays isolated and independently testable. It depends on `core` and
  implements `core`'s `download::Transcode` trait; `core` has no dependency on
  it.
- **`mcp-server/`** (package `downloadhub-mcp-server`, binary `mcp-server`) —
  MCP tools over stdio. Depends on `core` so it reuses the same queue manager
  and persistence as the desktop app. See [`MCP_SETUP.md`](MCP_SETUP.md).

Dependency arrows all point at `core`: it defines the contracts, satellite
crates implement them.

The root `Cargo.toml` is the workspace manifest and defines shared
`[workspace.package]` values each member inherits. Cargo builds the whole
workspace into `/target/` at the repo root, not `src-tauri/target/`.

### `app` ↔ `mcp-server` shared state: a shared SQLite database

Both binaries operate on the same download queue through `core`'s queue
manager and the same SQLite file, rather than one process calling the other
over local IPC. Neither has to be running for the other to make progress — an
agent can queue videos while the app is closed and they sit there until the
user opens it — and `core` stays the single source of truth for queue state.
Live progress events for the MCP client aren't needed, because the MCP server
never runs downloads at all (see "Where the agent boundary sits"); agents poll
`list_queue`, which reads the same rows the app updates.

To make two processes sharing one file safe, `QueueStore::open` uses WAL
journal mode with a 5-second busy timeout; with the default rollback journal,
one process's write fails with `SQLITE_BUSY` the instant the other holds any
lock. (`open_in_memory`, used by tests, skips this — WAL doesn't apply to
in-memory databases.)

## YouTube Data API client

Direct `reqwest` + `serde` calls against the REST API (`core::youtube`), not
the generated `google-youtube3` crate. `search.list`/`videos.list`/
`playlistItems.list` are plain API-key-authenticated GETs with a small response
shape we need a few fields from; the generated client would pull in
`yup-oauth2` and its own authenticator/hyper-connector setup — a second,
redundant OAuth stack alongside the `oauth2`-based one `core::auth` already
carries, for no benefit on unauthenticated calls. `reqwest` was already a `core`
dependency for the Google userinfo call.

Note that `core::auth` itself is currently dormant: it implements the Google
OAuth loopback flow and keychain token storage, but no Tauri command or UI
invokes it, and search runs on the API key alone.

`search.list` doesn't return duration, so `search_videos` makes a follow-up
`videos.list` call (`part=contentDetails`) batched over all result IDs and
merges `contentDetails.duration` (ISO 8601) back in via a small hand-rolled
parser (`core::youtube::duration`) — not worth a dependency for a four-field,
well-specified format.

## Video format/quality lookup

`core::stream` wraps `y7dl::Client` as `StreamClient`. `y7dl` isn't published
to crates.io, so it's a `git` dependency pinned to a commit (`rev`) rather than
tracking a branch, keeping builds reproducible.

`y7dl::Video`/`Format` derive `Deserialize` but not `Serialize`, so they can't
cross the Tauri IPC boundary directly. `core::stream` maps them into
`VideoDetail`/`FormatSummary` DTOs that do — the same shape-translation
`core::youtube` does for `VideoSummary`.

One `StreamClient` is created once in `AppState` and reused for the app's
lifetime: `y7dl::Client` caches parsed player JS and pools HTTP connections,
both wasted if rebuilt per lookup.

## Download queue persistence

`core::queue` (`QueueStore`) uses `rusqlite` with the `bundled` feature rather
than `sqlx`. Queue operations are single-row CRUD with no need for an
async-native driver or compile-time query checking, and `bundled` vendors
SQLite's C source so no system SQLite install is required on any target.
`rusqlite::Connection` is blocking, so every `QueueStore` method wraps its work
in `tokio::task::spawn_blocking`.

Inside the module, SQL lives only in `QueueRepository` (synchronous, on an
already-locked connection, constructed per operation and holding no state);
`QueueStore` owns the connection and handles locking, `spawn_blocking`, and
error wrapping. Locking and SQL are never in the same file.

The database lives at `<platform-data-dir>/downloadhub/queue.sqlite3`, resolved
by `core::paths` — shared by both binaries, which must agree on it. If the
directory can't be created or the database can't be opened,
`AppState.queue_store` is `None` and queue commands report that rather than the
app failing to start, mirroring how missing credentials degrade instead of
panicking.

`queue_entries.output_path` holds a destination *folder*, not a full file path:
`y7dl` doesn't mux, so a DASH download can produce two files for one entry. The
download orchestrator derives filenames from the video title and format inside
that folder.

Databases created by older versions may still carry a `pending_agent_actions`
table. Nothing reads or writes it; `ensure_schema` deliberately doesn't drop
it, since a destructive migration for dead weight is a bad trade.

## Output folder picker

The output-path field is filled via `tauri-plugin-dialog`'s native folder
picker (`open({ directory: true })`) with the plugin's own permission
(`dialog:default`), rather than a custom file-browser widget — the same pattern
as `tauri-plugin-opener`. The field stays freely editable as text, so a folder
can be pasted or typed.

## Ranged download execution and progress events

`core::download::run_download` is the orchestrator: given a queue entry id it
re-fetches the video via `StreamClient::fetch_video` (InnerTube stream URLs are
time-limited, so a stored URL can't be reused), resolves the requested itag,
derives the destination filename, and streams the format through
`StreamClient::download` — which is `y7dl` itself doing the ranged chunk
requests; `core::download` doesn't reimplement chunking. It transitions the
entry's status in `QueueStore` (`Queued`/`Failed` → `Downloading` →
`Completed`/`Failed`) as it runs.

Progress is a plain `FnMut(DownloadProgress)` callback, not a Tauri event —
`core` stays Tauri-agnostic. The callback is invoked from `ProgressWriter`, an
`AsyncWrite` wrapper between `y7dl`'s chunked writes and the destination file;
`y7dl` writes each network chunk via `write_all`, so wrapping the destination
gives fine-grained progress without touching `y7dl`. The wrapper throttles to
~5 callbacks/sec (time-based, not chunk-count-based) so any sink gets a bounded
rate regardless of chunk size.

`src-tauri/src/commands/download.rs` holds the Tauri-specific plumbing:
`start_download` spawns `run_download` on `tauri::async_runtime` and returns
immediately (its `Result` reports whether the download could be *started*, not
its outcome), wiring the progress callback to
`AppHandle::emit("download-progress", ...)`. The frontend
(`useDownloadProgressListener`) keeps the latest progress per queue id in a
small Zustand store. On a terminal event the listener invalidates the
`list_queue` query so the persisted status/error message — the source of truth
— resyncs from SQLite.

Concurrency is unbounded: each `start_download` spawns its own task with no
shared limiter. There is no resume support; an interrupted download restarts
from scratch. Both are open work.

## Queue controls (cancel/remove/retry)

Cancellation has to reach a task spawned by an earlier `start_download`, so —
unlike everything else in `core::download` — it's necessarily Tauri-runtime
state: `AppState` holds a `Mutex<HashMap<queue_id, JoinHandle<()>>>` of
in-flight tasks, populated by `start_download` and consumed by
`cancel_download`/`remove_from_queue`. `JoinHandle::abort()` takes `&self`, so
the map is locked only briefly to look the handle up.

Aborting kills the task mid-`.await`, so it never reaches the code that sets a
terminal status. `cancel_download` therefore sets `Cancelled` itself after
aborting — but only if the status is still `Queued`/`Downloading`, guarding the
race where the download actually finished in the window between the click and
the command running. Cancelling shouldn't retroactively overwrite a real
outcome.

`remove_from_queue` aborts any running task *before* deleting the row:
otherwise a still-running download would keep writing to disk after its queue
record — the only way to see or stop it — was gone.

"Retry" isn't a separate command. `start_download` accepts `Failed`/`Cancelled`
entries (rejecting only a call while the entry is already `Downloading`, as a
double-start guard), so retrying is the same call as starting; the UI just
relabels the button.

`cancel_download` and `remove_from_queue` are synchronous — they return once
the DB and task-registry updates are done — so their mutations invalidate
`list_queue` directly in `onSuccess` rather than needing an event round-trip.
Cancelling additionally clears that id from the progress store client-side,
since no `download-progress` event fires for a cancellation and a stale
`downloading` entry there would keep the UI showing a live progress bar and a
Cancel button indefinitely.

## App shell layout

The UI uses shadcn/ui defaults. No `docs/design/` directory exists, so the
Fluent design referenced in `CLAUDE.md` is not yet the source of truth.

`App.tsx` renders a fixed header (title, update checker, Settings) over a
two-column body — search left, `QueuePanel` sidebar right — inside an
`h-screen` root, each
column independently `overflow-y-auto` inside a `min-h-0 flex-1` list so it
scrolls in place rather than expanding its container. The window defaults to
1100×750 with a 760×480 floor. Panels must not stack in one unbounded centered
column: a long results or queue list then grows the whole window instead of
scrolling.

`VideoDetailPanel` is a `Dialog` mounted unconditionally in `SearchPanel` and
controlled by `open`/`onOpenChange`, not conditionally mounted — as an inline
block it pushed everything below it down the page. `useVideoFormats` tolerates
a `null` id (the query stays disabled), which is what makes unconditional
mounting work.

Each `QueuePanel` entry's title gets its own row rather than sharing a line
with the status badge and action buttons: in the 320px sidebar a shared row
leaves almost no width and truncates any real title after a few characters.

## Playlist import (bulk add to queue)

`playlistItems.list` (`core::youtube::list_playlist_items`, paginated up to a
200-item cap so one import can't become an unbounded number of API calls)
returns metadata only — no format/itag info. A queue entry needs one concrete
itag, and which itags a video offers varies video to video, so picking one itag
for an entire playlist isn't meaningful the way it is for a single video.

Instead `core::stream::FormatPreference` (`BestProgressive` / `BestAudioOnly` /
`Mp3`) is a quality *shortcut*: `core::enqueue` resolves each selected video's
own format list against it individually, sequentially, one call per video, and
enqueues whatever itag that resolves to. This was chosen over:

- hardcoding a "universal" itag like 18/140, which isn't guaranteed present on
  every video and wouldn't adapt to picking the *highest* available quality; or
- storing an unresolved preference on the entry and resolving at download time,
  which would widen `QueueEntry`'s `itag: u32` into a resolved-or-preference
  union.

Resolving up front means failures surface immediately in the "N added, M
skipped" result rather than later as stuck `Failed` entries.

`BestProgressive` deliberately does *not* fall back to a video-only format when
no progressive one exists; silently producing a video with no audio violates
what was asked for.

A video that fails to resolve (deleted, private, region-locked, no matching
format) is skipped and reported with a reason rather than aborting the import —
one bad video in a fifty-video playlist shouldn't cost the other forty-nine.

`list_playlist_items` (preview) and `import_playlist_to_queue` (resolve + add)
are two separate steps in the UI, mirroring the single-video "search → view
formats → add" pattern: the user sees what's in the playlist, with
individually-deselectable checkboxes, before committing to dozens of entries.

## Settings

`core::settings` is a small JSON blob (`AppSettings`) at
`<platform-data-dir>/downloadhub/settings.json`, alongside `queue.sqlite3`.
A missing file (first run) loads as `AppSettings::default()`; a *present but
corrupt* file errors rather than silently discarding what the user saved —
those are different situations. No caching: settings are read and written
directly against the file per call, since both happen rarely (startup, opening
the dialog) and a stale in-memory copy would be its own bug source.

`default_quality` reuses `FormatPreference` rather than inventing a
settings-only quality type. `VideoDetailPanel` and `PlaylistImportDialog` seed
their form state from `get_settings` each time they open — not on every render,
so it doesn't clobber what the user has already typed while a dialog stays
open. Pre-filling, not forcing: both flows still allow a per-add override.

## Download all (sequential, continue past failures)

`core::download::run_all_queued` is a thin sequential wrapper around
`run_download`: list `Queued` entries, call `run_download` for each in turn,
tally `completed`/`failed` into a `BatchDownloadOutcome`. A per-entry failure
needs no special handling — `run_download` already leaves it `Failed` with its
error message, exactly as an individually-started failure would; the loop just
doesn't stop there. Only a `QueueStore` failure stops the batch early, since
that affects every remaining entry too.

The `download_all` command takes two callbacks: `on_progress` (the same
throttled shape `start_download` wires to `download-progress`) and
`on_item_done`, fired once per entry after its `run_download` resolves. Without
the second, only the batch's return value would signal completion — but
`useDownloadProgressListener` reacts to a per-*entry* `completed`/`failed`
event to clear that entry's progress bar and resync its status, so
`on_item_done` lets the command emit the same per-entry event for batch
downloads as for individual ones, and the same listener handles both unchanged.

Unlike `start_download`, `download_all` awaits the whole batch rather than
spawning and returning. This is a deliberate simplification: the frontend's
mutation `isPending` then reflects "a batch is running" for its true duration
with no extra polling or state, and per-entry progress events still stream out
continuously via the callbacks — nothing about the live progress UI requires
the command to return early.

`AppState.batch_running` (an `AtomicBool`, *swapped* rather than
checked-then-set, to close the race between two concurrent `download_all`
calls) guards against a second batch, and
`start_download`/`cancel_download`/`remove_from_queue` all refuse to run while
it's set (`AppState::ensure_no_batch_running`). This isn't polish:
`download_all` calls `run_download` directly rather than through the
`running_downloads` abort-handle registry, so an individual command racing
against the batch's handling of the *same* entries has no safe outcome —
especially `cancel_download`, which would find no handle to abort and might
flip a status the batch is about to overwrite. Refusing outright is far simpler
than reconciling the two. `download_all` also checks `running_downloads` is
empty before starting, closing the reverse case (imperfectly, given the
inherent TOCTOU race across two independently-locked pieces of state).

## MP3 download (transcode via ffmpeg)

YouTube serves no MP3 stream, so the one-click "Download MP3" button queues the
video's **itag 140** stream — the standard 128 kbps AAC-LC m4a, present on
virtually every video — with a `convert_to_mp3` flag on the queue entry (a
SQLite column added by an idempotent `ALTER TABLE` migration; `NewQueueEntry`
deserializes it with `serde(default)`). itag 140 over itag 139 (~48 kbps
HE-AAC) deliberately: MP3 conversion is a lossy-to-lossy transcode, so it
should start from the best universally available source rather than compounding
139's low bitrate.

The transcode lives in the `transcode/` crate: a thin `tokio::process` wrapper
around `ffmpeg -i in.m4a -vn -codec:a libmp3lame -q:a 2 out.mp3` (LAME VBR ~190
kbps — transparent for a 128 kbps AAC source without wasting space on a fixed
320k). The child is spawned with `kill_on_drop`, so cancelling mid-transcode
kills ffmpeg rather than orphaning it.

ffmpeg is *not* linked as a library. It's an external binary resolved *per
download start* (`AppState::resolve_transcoder`, so a settings change needs no
restart) in priority order: the `ffmpeg_path` setting, then a bundled sidecar
next to the app executable, then PATH (the last two via
`downloadhub_transcode::locate_ffmpeg`; PATH matters under `tauri dev`, which
doesn't stage sidecars).

On Windows a static GPL build is vendored in the repo at
`tools/ffmpeg-windows-x86_64.exe` — a deliberately committed binary rather than
a fetch-at-build-time dependency, so every build is reproducible from a bare
checkout. `just sidecar` copies it to `src-tauri/binaries/ffmpeg-<triple>.exe`
and it's declared in `tauri.windows.conf.json`'s `externalBin` (a
platform-specific config that JSON-*merges* over `tauri.conf.json`, so array
values there must repeat `binaries/mcp-server`). macOS bundles no ffmpeg: a
vendored unsigned binary gets blocked by Gatekeeper, so macOS users set the
settings path or rely on PATH.

`core::download` defines an object-safe `Transcode` trait; `transcode`
implements it for `Transcoder`; `src-tauri` depends on both and hands
`core::download` a `&dyn Transcode`. So `core` never depends on a concrete
transcoder, and `mcp-server` never links ffmpeg code it doesn't use.

The conversion is a step *inside* `run_download`, not a separate queue state:
after the m4a finishes, the entry (still `Downloading`) emits a
`Transcoding`-phase progress event (the UI shows "converting to mp3..."),
ffmpeg writes `<title>.mp3`, and the m4a is deleted only after a successful
transcode — on failure the entry goes `Failed` with ffmpeg's stderr and the m4a
is kept, so downloaded data isn't lost. Because it's part of `run_download`,
"Download all" finishes each entry completely (download → transcode → delete)
before starting the next. Prerequisites (audio-only format, ffmpeg present) are
validated *before* the download starts, so a doomed entry fails without
spending bandwidth.

`run_download`/`run_all_queued` take a `DownloadContext` struct (stream client +
store + optional transcoder) rather than a growing positional parameter list.

MCP-proposed entries never set `convert_to_mp3`; the desktop UI is its only
writer today.

## Queue editing (selection + re-format)

Once an agent can fill the queue unattended, the queue stops being a write-once
list and becomes something the user edits before committing. Two operations,
deliberately different in kind:

- **One entry, exact format** (`set_queue_entry_format`): click the format line
  on a queue row to open `ChangeFormatDialog`, which lists that video's real
  formats — the same `get_video_formats` call and layout as the add flow — and
  repoints the entry at the chosen itag. Audio-only rows additionally offer
  "Use as MP3", since `convert_to_mp3` is a property of the queue row rather
  than of the stream.
- **Many entries, one preference** (`set_queue_entries_quality`): check entries
  (or "Select all queued") and apply a `FormatPreference`. It *can't* take an
  itag, because a multi-select spans videos whose available itags differ — the
  same constraint that produced `FormatPreference` for playlist import, which
  is why `core::enqueue::reformat_entries` shares its per-video resolution and
  its "skip and report, don't abort" rule.

Both reset the entry to `Queued` with `error_message` cleared
(`QueueStore::set_format`): the previous format's failure says nothing about
the new one, and re-formatting a `Completed` entry is a request to fetch it
again. Both refuse to touch a `Downloading` entry — changing the format under a
running task would leave it writing the old stream to a path derived from the
new one — and both are barred while a batch runs, by the same
`ensure_no_batch_running` guard.

## MCP

### The `mcp-server` binary

Implemented with [`rmcp`](https://github.com/modelcontextprotocol/rust-sdk)
(the official Rust MCP SDK; MIT-licensed, compatible with this project's GPL
obligation) over stdio — MCP clients spawn local servers as child processes and
speak JSON-RPC over stdin/stdout, so stdio requires zero networking, ports, or
auth setup. Nothing may be printed to stdout (it belongs to the protocol);
diagnostics go to stderr.

Tool surface:

- **Read-only:** `search_videos` (needs `YOUTUBE_API_KEY` via the MCP client's
  `env` config), `get_video_formats` (no configuration), `list_queue`.
- **Queue-mutating, executed directly:** `add_to_queue`, `add_mp3_to_queue`,
  `remove_from_queue`.
- **Download-starting:** none. Deliberately — see below.

Every tool call re-reads `settings.json` and refuses to serve when
`AppSettings.mcp_enabled` ("Allow AI agent access") is off. Re-reading per call
(a tiny local file, at human/agent frequency) means toggling the setting takes
effect immediately with no restart or cross-process signal. That switch is the
one remaining gate, and it is wholesale: on or off.

### Where the agent boundary sits: at the download, not the queue

Enqueueing executes immediately; the boundary is the *download*.

The two operations have very different blast radii. Adding a queue row costs
nothing irreversible — the entry is visible in the sidebar, its format can be
changed, and it can be removed, all before any byte moves. Starting a download
spends bandwidth, writes media files, and may run ffmpeg. Gating the cheap,
reversible, high-frequency operation produces a stream of low-information
approval prompts that train the user to click through them — and then the
expensive operation gets that same reflexive click.

So the queue *is* the review surface. An agent proposes by filling it; the user
reviews the actual proposal — real titles, formats, destinations — and either
edits it, empties it, or clicks "Download all". One deliberate action taken
with the whole batch in view, instead of N prompts taken one at a time with no
context.

Critically, this is enforced by the **tool surface**, not a runtime check:
`mcp-server` exposes no tool that can start a transfer, so there is no code path
— buggy, malicious, or prompt-injected — by which an agent starts one. An
approval check is a condition that must hold; this is a capability that does not
exist. `core::download` isn't even reachable from the server's tool router.

Two consequences worth knowing:

- `useQueue` polls every 3 seconds. The writer is another *process*, so Tauri's
  event system can't reach across it and agent-added entries would otherwise
  not appear until something else triggered a refetch.
- There is no `requested_by`. It would be the MCP client's self-reported
  `clientInfo.name` — never an authenticated identity — and with no per-action
  prompt there is nothing to display it on.

### Batch tools and token cost

`add_to_queue` and `add_mp3_to_queue` both take `videos: [...]`, a list, rather
than a single video. An agent queueing a ten-track album spends one tool call,
not ten — ten round-trips of tool-call JSON plus ten result payloads is a real
token cost for the agent and a real latency cost for the user, and nothing about
the operation needs to be serialized per video. Per-video failures don't sink
the batch: `core::enqueue::enqueue_videos` resolves each independently and
reports failures in `skipped` alongside `added`.

`add_mp3_to_queue` exists as its own tool rather than as
`add_to_queue(quality: "mp3")` (which also works) because MP3 is the common
request — "download these songs" — and a tool whose name and description say
exactly that gets selected more reliably than an enum value buried in another
tool's parameter schema.

Neither tool asks for an itag. They take a `FormatPreference`
(`best_progressive` / `best_audio_only` / `mp3`, defaulting to the user's
configured default quality) and resolve it against each video's real format list
server-side. Agents don't have to call `get_video_formats` first — another
round-trip per video saved — and can't queue an itag the video doesn't offer.
`output_path` falls back to the user's default output folder, then to the OS
Downloads folder, so the common case needs no path at all.

### Packaging: mcp-server as a Tauri sidecar

The `mcp-server` binary ships *inside* the desktop installer rather than as a
separate download, so installing is a single step and the binary is guaranteed
to match the app version. It's declared as a Tauri `externalBin` sidecar
(`bundle.externalBin: ["binaries/mcp-server"]`); `tauri build` embeds
`src-tauri/binaries/mcp-server-<target-triple>` next to the main app executable
(Tauri strips the triple suffix at bundle time). The `sidecar` recipe in the
root `justfile` — a dependency of `just release`, which is also what
`.github/workflows/build-windows.yml` runs — builds and copies it there first,
since Tauri validates the file exists at compile time. The staged binaries are
gitignored build artifacts.

The app doesn't *spawn* the sidecar (so no `tauri-plugin-shell` dependency) —
external MCP clients spawn it by absolute path. The `mcp_server_path` command
resolves that from the running app's own `current_exe()` location, which the
Settings dialog surfaces with a copy-paste client config block. Under `tauri
dev` the sidecar isn't staged next to the debug binary, so the shown path won't
exist until a real build; the connect UI is only actionable in an installed
build anyway.

## Credential embedding

`core::secrets` resolves each of `GOOGLE_OAUTH_CLIENT_ID`,
`GOOGLE_OAUTH_CLIENT_SECRET`, and `YOUTUBE_API_KEY` runtime-first, then
compile-time: `std::env::var` (a local `.env` via `dotenvy`, unchanged for dev)
falling back to `option_env!`, so a shipped installer with no `.env` carries the
values baked in. Real environment variables always beat `.env`.

Because `option_env!` reads the *process environment at compile time* (not
`.env`), the root `justfile` uses `set dotenv-load` to export the workspace
`.env` into every recipe's environment — a local `just release` embeds them with
no manual step. CI passes the three from GitHub Actions repository secrets of
the same names.

Embedding is not encryption: the values are recoverable from the binary via
`strings`. For a desktop app this is an accepted tradeoff — a Google "Desktop
app" OAuth client id/secret aren't confidential by design (RFC 8252), and the
YouTube key is protected by Google-side API/quota restrictions rather than by
secrecy.

## Muxing extension point

`y7dl` doesn't mux DASH streams, so a video-only or audio-only format is saved
to its own clearly-labeled file (`<title>.video.<ext>` / `<title>.audio.<ext>`);
a progressive format is saved as `<title>.<ext>`. `core::download::output` owns
this naming.

The `Transcode` trait is the seam a real muxer would plug into — `core` decides
*when* post-processing happens, satellite crates decide *how* — so muxing can be
added without a rewrite.

## License

This project depends on [`y7dl`](https://github.com/erwin-lovecraft/y7dl)
(GPL-3.0-or-later), so the whole project is licensed GPL-3.0-or-later — see
[`LICENSE`](../LICENSE). No proprietary/closed dependency may be added to any
crate that links against `y7dl` (i.e. `core/` and anything depending on it).
