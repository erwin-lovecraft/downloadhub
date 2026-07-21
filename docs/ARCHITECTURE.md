# Architecture

## Cargo workspace layout

The project is a Cargo workspace with four members:

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
- **`transcode/`** (package `downloadhub-transcode`) — audio transcoding
  via an external `ffmpeg` binary (bundled as a Tauri sidecar; see "MP3
  download" under Phase 2). A separate crate rather than a `core` module so
  the process-spawning boundary stays isolated and independently testable.
  It depends on `downloadhub-core` and implements `core`'s
  `download::Transcode` trait (the transcoding seam), so the dependency
  arrow points the same way as `mcp-server`'s: `core` defines the contract,
  satellite crates implement it. `core` has no dependency on this crate.
- **`mcp-server/`** (package `downloadhub-mcp-server`, binary `mcp-server`)
  — Phase 3 binary exposing MCP tools over stdio. Depends on
  `downloadhub-core` so it reuses the exact same queue manager and
  persistence as the desktop app rather than duplicating logic. See
  "Phase 3" below and [`MCP_SETUP.md`](MCP_SETUP.md).

`Cargo.toml` at the repo root is the workspace manifest (`members = ["core",
"src-tauri", "mcp-server", "transcode"]`) and defines shared `[workspace.package]` values
(`version`, `edition`, `license`) that each member inherits with
`field.workspace = true`.

Cargo builds the whole workspace to a single `/target/` directory at the
repo root (not `src-tauri/target/`); the root `.gitignore` accounts for this.

### `app` ↔ `mcp-server` shared state (decided: shared SQLite database)

Both `src-tauri` and `mcp-server` operate on the same download queue. Both
depend on `downloadhub-core`'s queue manager and share the same SQLite
database file rather than one process calling into the other over local
IPC — this avoids needing either binary to be running for the other to
make progress (an agent can queue videos while the app is closed; they sit
there until the user opens it), and keeps `downloadhub-core` as the single
source of truth for queue state and transitions. Finalized in Phase 3:
live progress events for the MCP client turned out not to be needed,
because the MCP server never runs downloads at all (see "Where the agent
boundary sits" below) — agents poll `list_queue` for status, which reads
the same rows the app updates.

To make two processes sharing one file safe, `QueueStore::open` puts the
database in WAL journal mode and sets a 5-second busy timeout; with the
default rollback journal, one process's write would fail with
`SQLITE_BUSY` the instant the other held any lock. (`open_in_memory`, used
by tests, skips this — WAL doesn't apply to in-memory databases.)

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

### Ranged download execution and progress events

`core::download` (`run_download`) is the orchestrator: given a queue entry
id, it re-fetches the video via `StreamClient::fetch_video` (stream URLs
from InnerTube are time-limited, so a stored URL can't be reused), resolves
the requested itag, derives the destination filename (see "Muxing
extension point" below), and streams the format through
`StreamClient::download` — which is `y7dl::Client::download` itself doing
the ranged chunk requests; `core::download` doesn't reimplement chunking.
It also transitions the entry's status in `QueueStore`
(`Queued`/`Failed` → `Downloading` → `Completed`/`Failed`) as it runs.

Progress reporting is a plain `FnMut(DownloadProgress)` callback, not a
Tauri event — `core` stays Tauri-agnostic. The callback is invoked from a
`ProgressWriter`, an `AsyncWrite` wrapper placed between `y7dl`'s chunked
writes and the destination file; since `y7dl::Client::download` writes
each network chunk via `AsyncWrite::write_all` (chunks well under the 10 MB
`Range` request size), wrapping the destination gives fine-grained progress
without touching `y7dl` itself. The wrapper throttles callback invocations
to ~5/sec internally (time-based, not chunk-count-based) so any sink —
Tauri events, a channel, a test spy — gets a bounded rate regardless of
chunk size.

`src-tauri/src/commands/download.rs`'s `start_download` command is where
Tauri-specific plumbing lives: it spawns `run_download` on
`tauri::async_runtime` and returns immediately (the command's `Result` only
reports whether the download could be *started*, not its outcome), wiring
the progress callback to `AppHandle::emit("download-progress", ...)`. The
frontend listens for that event (`useDownloadProgressListener`) and keeps a
small Zustand store of the latest progress per queue id — Zustand was
already a declared-but-unused dependency for exactly this kind of
cross-component transient state, so no new frontend dependency was needed.
On a terminal event (`completed`/`failed`) the listener invalidates the
`list_queue` query so the queue panel's persisted status/error message
(the source of truth) resyncs from SQLite.

Concurrency is intentionally unbounded for now — each `start_download` call
spawns its own task with no shared limiter. Phase 2 explicitly adds
"concurrent downloads (configurable limit, default ~3)"; adding a limiter
here would be building ahead of that step. Likewise, no resume support yet
(Phase 2: "resumable downloads via range requests") — a failed/interrupted
download must be restarted from scratch by starting it again.

### Queue controls (cancel/remove/retry)

Cancellation needs to reach into a task already spawned by an earlier
`start_download` call, so — unlike everything else in `core::download` —
it's necessarily Tauri-runtime state, not `core` logic: `AppState` holds a
`Mutex<HashMap<queue_id, tauri::async_runtime::JoinHandle<()>>>` of
in-flight download tasks, populated by `start_download` and consumed by
`cancel_download`/`remove_from_queue`. `JoinHandle::abort()` takes `&self`,
so the map only needs to be locked briefly to look the handle up — no
ownership juggling.

Aborting a task kills it mid-`.await`, so it never reaches the code in
`run_download` that would set a terminal `QueueStore` status. `cancel_download`
therefore sets the entry to `Cancelled` itself after aborting — but only if
the entry's current status is still `Queued`/`Downloading`, guarding against
a race where the download actually finished (`Completed`/`Failed`) in the
brief window between the user clicking Cancel and the command running;
cancelling shouldn't retroactively overwrite a real outcome.

`remove_from_queue` aborts any running task for that id *before* deleting
the row, for the same reason `cancel_download` exists at all: without it, a
still-running download would keep writing to disk after its queue record —
the only way to see or stop it — was already gone.

"Retry" isn't a separate command: `start_download` already accepts
`Failed`/`Cancelled` entries (it only rejects a call while the entry is
already `Downloading`, added alongside this step as a double-start guard),
so retrying a failed download is exactly the same call as starting a
queued one — the UI just relabels the button. The `download-progress`
Tauri event only covers `start_download`'s async lifecycle; `cancel_download`
and `remove_from_queue` are synchronous commands (they only return once the
DB/task-registry updates are done), so their mutations invalidate the
`list_queue` query directly in `onSuccess` rather than needing an event
round-trip — cancelling additionally clears that queue id's entry from the
progress Zustand store client-side, since nothing else would (no
`download-progress` event fires for a cancellation) and a stale
`status: "downloading"` entry there would otherwise keep the UI showing a
live progress bar and a Cancel button indefinitely.

### App shell layout

No `docs/design/` directory exists yet (the Fluent design mentioned in
`CLAUDE.md` hasn't been exported into the repo), so per that same doc's
fallback — "otherwise use clean shadcn/ui defaults for now" — this step is
a usability pass on the existing shadcn-default UI, not a redesign. A
separate, much earlier branch (`feat/fluent-design-frontend-scaffold`,
PR #1) attempted the Fluent port directly off `Initial project`, before any
of steps 2–8 existed; it's an abandoned WIP snapshot now (last commit
predates all of them, `mergeStateStatus: DIRTY` against current `main`) and
was left alone rather than reconciled into this step.

The previous layout stacked every panel in a single centered column with
no bounded height, so a long search-results or queue list just grew the
whole window instead of scrolling in place, and the 800×600 default window
(`tauri.conf.json`) was too small to show search results and the queue at
once. `App.tsx` now renders a fixed header (title + `AuthPanel`) over a
two-column body — search on the left, a `QueuePanel` sidebar on the right —
inside an `h-screen` root, with each column independently
`overflow-y-auto` inside a `min-h-0 flex-1` list so it scrolls in place
rather than expanding its container. The window default grew to 1100×750
with a 760×480 floor (`minWidth`/`minHeight`) that still holds up at that
size, verified in a browser preview resized to match.

`VideoDetailPanel` moved from an inline block appended below the search
results (pushing everything below it down the page as results and formats
piled up) to a `Dialog` (shadcn/`@base-ui/react`), mounted unconditionally
in `SearchPanel` and controlled by the dialog's own `open`/`onOpenChange`
rather than conditional mounting — `useVideoFormats` already tolerated a
`null` id (query stays `disabled`), so no hook changes were needed, just
widening its `videoId` prop type to match.

Each `QueuePanel` entry's title also moved to its own row instead of
sharing a line with the status badge and action buttons: in the 320px
sidebar, a title sharing a row with "downloading" + Cancel + Remove had
almost no width left and truncated after a handful of characters on any
real (non-mock) title — verified by mocking `list_queue` with a
deliberately long title in the browser preview before and after this fix.

### Muxing extension point

No `ffmpeg`/transcoding dependency yet. DASH adaptive downloads save
video-only and audio-only streams as separate, clearly labeled files. A
`Muxer` trait (or equivalent TODO-marked seam) will be added to
`downloadhub-core`'s download-completion path once a transcode crate is
selected, so muxing can be plugged in without a rewrite.

## Phase 2

### Playlist import (bulk add to queue)

`playlistItems.list` (`core::youtube::list_playlist_items`, paginated up
to a 200-item cap so one import can't turn into an unbounded number of API
calls) only returns metadata — no format/itag info, same as `search.list`.
A single-video queue entry needs one concrete resolved itag, and which
itags a video actually offers varies video to video, so asking the user to
pick one itag for an entire playlist import isn't meaningful the way it is
for a single video's "view formats" flow.

Instead, `core::stream::FormatPreference` (`BestProgressive` /
`BestAudioOnly` / `Mp3`) is a quality *shortcut*: `core::enqueue` resolves
each selected video's own format list against it individually
(`StreamClient::resolve_queue_format`, reusing the exact same
`get_video_formats` call the single-video flow already makes) and enqueues
whatever itag that resolves to for that video — sequentially, one call per
video. This was a deliberate choice over either (a) hardcoding a
"universal" itag like 18/140 across the whole playlist, which is fragile
(not guaranteed present on every video) and wouldn't adapt to actually
picking the *highest* available quality, or (b) storing an unresolved
preference on the queue entry itself and resolving it later at download
time, which would mean widening `QueueEntry`'s `itag: u32` — an
already-shipped, tested Phase 1 field — into some kind of
resolved-or-preference union for a Phase 2 feature. `BestProgressive`
deliberately does *not* fall back to a video-only format when no
progressive one exists; silently producing a video with no audio would
violate what "video + audio" quality was asked for. Resolution is
sequential, one video at a time, matching the codebase's current
single-download-at-a-time reality.

A video that fails to resolve (deleted/private/region-locked, or no
format matching the preference) is skipped and reported with a reason
rather than aborting the whole import — resolving up front, at import
time rather than later at download time, means these show up immediately
in the "N added, M skipped" result instead of only surfacing much later as
a stuck `Failed` queue entry.

`list_playlist_items` (preview) and `import_playlist_to_queue` (bulk
resolve + add) are two separate commands/steps in the UI, mirroring the
existing single-video "search → view formats → add" pattern: the user
sees what's actually in the playlist (with individually-deselectable
checkboxes, all selected by default) before committing to adding
potentially dozens of queue entries in one action.

### Settings (default output folder, default quality)

Reprioritized ahead of the rest of the originally-planned Phase 2 order
(concurrent downloads, resumable downloads) at the user's explicit
request, since these defaults are useful immediately and don't depend on
anything else in Phase 2.

`core::settings` is a small JSON blob (`AppSettings { default_output_path,
default_quality }`) at `<platform-data-dir>/downloadhub/settings.json`,
alongside `queue.sqlite3` — `AppState::resolve_app_data_dir` (renamed from
the queue-only `open_queue_store`'s inline logic) now resolves that shared
directory once and hands it to both. A missing file (first run) loads as
`AppSettings::default()`; a *present but corrupt* file still errors rather
than silently discarding whatever the user had saved — those are different
situations and shouldn't be handled the same way. No caching: settings are
read/written directly against the file on each `get_settings`/
`save_settings` call, since both happen rarely (app startup, opening the
settings dialog) and a stale in-memory copy would be its own source of
bugs for essentially zero benefit at this frequency.

`default_quality` reuses `core::stream::FormatPreference` — the same
two-option quality shortcut playlist import already uses — rather than
inventing a separate settings-only quality type. `SearchPanel`'s
`VideoDetailPanel` and `PlaylistImportDialog` both seed their local output-
path (and, for the playlist dialog, quality) form state from
`get_settings` each time they open (not on every render, so it doesn't
clobber whatever the user's already typed/picked while a dialog stays
open) — pre-filling rather than forcing, since both flows still let the
user override per-add.

### Download all (sequential, continue past failures)

Reprioritized ahead of concurrent/resumable downloads at the user's
explicit request: a "Download all" action that processes every `Queued`
entry one at a time, continuing to the next entry when one fails rather
than aborting the batch.

`core::download::run_all_queued` is a thin sequential wrapper around the
existing `run_download`: list `Queued` entries, call `run_download` for
each in turn, tally `completed`/`failed` into a `BatchDownloadOutcome`. A
per-entry failure needs no special handling to "enqueue it again with an
error message" — `run_download` already leaves a failed entry `Failed`
with its error message in `QueueStore`, exactly as if it had been started
individually and failed; the batch loop just doesn't stop there. Only a
`QueueStore` failure itself (listing entries) stops the batch early, since
that affects every remaining entry too.

The Tauri `download_all` command takes two callbacks from
`run_all_queued` — `on_progress` (throttled in-progress updates, same
callback shape `start_download` already wires to `download-progress`
events) and a second `on_item_done` fired once per entry after its
`run_download` call resolves. Without that second callback, only
`run_all_queued`'s own return value would signal completion — but the
frontend's `useDownloadProgressListener` reacts to a per-*entry*
`completed`/`failed` event to clear that entry's progress bar and
resync its status from SQLite; `on_item_done` lets the command emit
that same per-entry event for batch-driven downloads as it already does
for individually-started ones, so the same listener code handles both
without modification.

Unlike `start_download`, `download_all` doesn't spawn and return
immediately — it's `async fn download_all(...)` awaiting the whole batch
directly, so the command only resolves once every entry has been
attempted. This was a deliberate simplification over the fire-and-forget
`start_download`/`running_downloads`-registry pattern: it means the
frontend's mutation `isPending` state already reflects "a batch is
running" for its true duration with no extra polling or state needed,
and per-entry `download-progress` events still stream out continuously
throughout via the callbacks above — nothing about the live progress UI
requires the command itself to return early.

`AppState.batch_running` (an `AtomicBool`, swapped instead of
locked-then-set to close the check-then-set race between two concurrent
`download_all` calls) guards against a second batch starting while one is
in flight, and `start_download`/`cancel_download`/`remove_from_queue` all
refuse to run while it's set (`AppState::ensure_no_batch_running`). This
isn't just "polish" — `download_all` calls `run_download` directly rather
than through the `running_downloads` abort-handle registry `start_download`
populates, so an individual command (especially `cancel_download`, which
would find no handle to abort and might still flip a DB status the batch
is actively about to overwrite) racing against the batch's own handling of
the *same* entries has no safe, correct outcome — refusing outright is far
simpler than trying to reconcile the two. `download_all` additionally
checks `running_downloads` is empty before starting, closing (though not
perfectly, given the inherent narrow TOCTOU race in checking then acting on
two independently-locked pieces of state) the reverse case of a batch
starting just as an individual download is getting under way.

### MP3 download (transcode via a bundled ffmpeg sidecar)

YouTube serves no MP3 stream, so the "Download MP3" button (one click per
search result, no format picker) queues the video's **itag 140** stream —
the standard 128 kbps AAC-LC m4a, present on virtually every video — with a
`convert_to_mp3` flag on the queue entry (a new SQLite column, added by an
idempotent `ALTER TABLE` migration for databases created before it existed;
`NewQueueEntry` deserializes the field with `serde(default)`). itag 140
was chosen over itag 139 (~48 kbps HE-AAC) deliberately: MP3 conversion is
a lossy-to-lossy transcode, so it should start from the best universally
available audio source rather than compounding 139's low bitrate.

The transcode itself lives in a fourth workspace crate, **`transcode/`**
(package `downloadhub-transcode`): a thin `tokio::process` wrapper around
an external `ffmpeg` binary (`ffmpeg -i in.m4a -vn -codec:a libmp3lame
-q:a 2 out.mp3`, LAME VBR ~190 kbps — transparent for a 128 kbps AAC
source without wasting space on a fixed 320k). ffmpeg is *not* linked as a
library: it's an external binary, resolved *per download start*
(`AppState::resolve_transcoder`, so a settings change needs no restart)
in priority order — the custom `ffmpeg_path` from settings, then a
bundled sidecar next to the app executable, then PATH. On Windows a
static GPL build is *vendored in the repo* at
`tools/ffmpeg-windows-x86_64.exe` (a deliberately committed binary
rather than a fetch-at-build-time dependency: an earlier iteration used
the `ffmpeg-static` npm package purely as a download mechanism though
nothing in the JS bundle used it; vendoring keeps every build
reproducible from a bare checkout with no external fetch) — `just
sidecar` copies it to `src-tauri/binaries/ffmpeg-<triple>.exe`, and it's
declared in `tauri.windows.conf.json`'s `externalBin` (the
platform-specific config that JSON-merges over `tauri.conf.json` on
Windows only, so array values there must repeat `binaries/mcp-server`).
macOS bundles no ffmpeg for now: a vendored unsigned binary gets blocked
by Gatekeeper, so the previously-vendored macOS binary was removed and
macOS users set the settings option (or rely on PATH) instead. `core::download` defines an
object-safe `Transcode` trait; the `transcode` crate depends on `core` and
implements it for its `Transcoder` (so `core` never depends on a concrete
transcoder). `src-tauri` depends on both, resolves the ffmpeg path per
download start (settings first, then `downloadhub_transcode::locate_ffmpeg`:
exe-adjacent sidecar, then PATH as a dev fallback since `tauri dev` doesn't
stage sidecars) and hands `core::download` the `Transcoder` as
`&dyn Transcode`. The child process is
spawned with `kill_on_drop`, so cancelling a download mid-transcode kills
ffmpeg rather than orphaning it.

The conversion is a step *inside* `run_download`, not a separate queue
state: after the m4a finishes downloading, the entry (still `Downloading`
in the DB) emits a `Transcoding`-phase progress event (the UI shows
"converting to mp3..."), ffmpeg writes `<title>.mp3`, and only after a
successful transcode is the m4a deleted — on failure the entry goes
`Failed` with ffmpeg's stderr and the m4a is kept so the downloaded data
isn't lost. Because the transcode is part of `run_download`, "Download
all" naturally finishes each entry completely (download → transcode →
delete m4a) before starting the next one. Prerequisites (audio-only
format, ffmpeg actually present) are validated *before* the download
starts, so a doomed entry fails without spending bandwidth. The growing
parameter list this added to `run_download`/`run_all_queued` was folded
into a `DownloadContext` struct (stream client + store + optional
transcoder) rather than a fourth positional argument.

MCP-proposed queue entries never set `convert_to_mp3` (the desktop UI is
the only writer of the flag today); exposing it as a tool parameter is a
straightforward later addition.

### Queue editing (selection + re-format)

Once an agent can fill the queue unattended, the queue stops being a
write-once list and becomes something the user edits before committing to
it. Two operations, deliberately different in kind:

- **One entry, exact format** (`set_queue_entry_format`): click the format
  line on a queue row to open `ChangeFormatDialog`, which lists that
  video's real formats — the same `get_video_formats` call and layout as
  the add-to-queue flow — and repoints the entry at the chosen itag.
  Audio-only rows additionally offer "Use as MP3", since `convert_to_mp3`
  is a property of the queue row rather than of the stream.
- **Many entries, one preference** (`set_queue_entries_quality`): check
  entries (or "Select all queued") and apply a `FormatPreference`. It
  can't take an itag, because a multi-select spans videos whose available
  itags differ — the same constraint that produced `FormatPreference` for
  playlist import, which is why `core::enqueue::reformat_entries` shares
  its per-video resolution and its "skip and report, don't abort" rule.

Both reset the entry to `Queued` with its `error_message` cleared
(`QueueStore::set_format`): the previous format's failure says nothing
about the new one, and re-formatting a `Completed` entry is a request to
fetch it again. Both refuse to touch a `Downloading` entry — changing the
format under a running task would leave it writing the old stream to a
path derived from the new one — and both are barred while a batch runs, by
the same `ensure_no_batch_running` guard the other per-entry commands use.

The shared resolution logic lives in `core::enqueue` (formerly
`core::playlist`, renamed when the MCP add tools and the bulk re-format
became the other two callers of the same "resolve a preference against
each video, one at a time" loop).

## Phase 3 (MCP)

### The `mcp-server` binary

Implemented with [`rmcp`](https://github.com/modelcontextprotocol/rust-sdk)
(the official Rust MCP SDK; MIT-licensed, so compatible with this project's
GPL obligation) over the stdio transport — MCP clients like Claude Desktop
spawn local servers as child processes and speak JSON-RPC over
stdin/stdout, so stdio is the transport that requires zero
networking/ports/auth setup. Nothing may be printed to stdout (it belongs
to the protocol); diagnostics go to stderr.

Tool surface:

- **Read-only:** `search_videos` (needs `YOUTUBE_API_KEY` passed via the
  MCP client's `env` config), `get_video_formats` (no configuration, same
  `StreamClient` as the app), `list_queue` (reads the shared database).
- **Queue-mutating, execute directly:** `add_to_queue`,
  `add_mp3_to_queue`, `remove_from_queue`.
- **Download-starting:** none. Deliberately — see below.

Every tool call re-reads `settings.json` and refuses to serve when the
user has turned off "Allow AI agent access (MCP server)"
(`AppSettings.mcp_enabled`, default true). Re-reading per call (a tiny
local file, called at human/agent frequency) means toggling the setting in
the app takes effect immediately with no restart or cross-process signal.
That switch is the one remaining gate, and it is wholesale: on or off.

### Where the agent boundary sits (decided: at the download, not the queue)

The original Phase 3 design routed every queue mutation through a
`pending_agent_actions` table that the user approved one row at a time in
an `AgentActionsPanel`. That was replaced: enqueueing now executes
immediately, and the boundary moved to the *download*.

The reasoning is that the two operations have very different blast radii.
Adding a queue row costs nothing irreversible — the entry is visible in the
sidebar, its format can be changed, and it can be removed, all before any
byte moves. Starting a download spends bandwidth, writes media files to
disk, and (for MP3 entries) runs ffmpeg. Gating the cheap, reversible,
high-frequency operation produced a steady stream of approval prompts that
carried little information and trained the user to click through them,
while the expensive operation was gated by the *same* prompt and got the
same reflexive click.

So the queue is now the review surface itself. An agent proposes by
filling it; the user reviews the actual proposal — real titles, real
formats, real destination — and either edits it, empties it, or clicks
"Download all". One deliberate action, taken with the whole batch in view,
replaces N prompts taken one at a time with no context.

Critically, this is enforced by the **tool surface**, not by a runtime
check: `mcp-server` exposes no tool that can start a transfer, so there is
no code path — buggy, malicious, or prompt-injected — by which an agent
starts one. The previous design's approval check was a condition that had
to hold; this is a capability that does not exist. `core::download` isn't
even reachable from the server binary's tool router.

Consequences elsewhere:

- `core::agent`, the `pending_agent_actions` table, the three
  `*_agent_action` commands, and the `AgentActionsPanel`/`useAgentActions`
  frontend are all deleted. Databases created by older versions keep the
  now-unused table; `ensure_schema` doesn't drop it, since a destructive
  migration for dead weight is a bad trade.
- The queue query polls every 3 seconds (`useQueue`). The writer is
  another *process*, so Tauri's event system can't reach across it and
  agent-added entries would otherwise not appear until something else
  triggered a refetch. Same reasoning the old agent-action poll used.
- `requested_by` is gone with the table. It was the MCP client's
  self-reported `clientInfo.name` — never an authenticated identity, and
  with no per-action prompt left to label, nothing to display it on.

### Batch tools and token cost

`add_to_queue` and `add_mp3_to_queue` both take `videos: [...]`, a list,
rather than a single video. An agent queueing a ten-track album spends one
tool call, not ten — ten round-trips of tool-call JSON plus ten result
payloads is a real token cost for the agent and a real latency cost for
the user, and nothing about the operation needs to be serialized per video.
Per-video failures don't sink the batch: `core::enqueue::enqueue_videos`
resolves each video independently and reports failures in `skipped`
alongside the entries that made it into `added`.

`add_mp3_to_queue` exists as its own tool rather than as
`add_to_queue(quality: "mp3")` (which also works) because MP3 is the
common request — "download these songs" — and a tool whose name and
description say exactly that gets selected more reliably than an enum
value buried in another tool's parameter schema.

Neither tool asks for an itag. They take a `FormatPreference`
(`best_progressive` / `best_audio_only` / `mp3`, defaulting to the user's
configured default quality) and resolve it against each video's real
format list server-side. Agents don't have to call `get_video_formats`
first — saving another round-trip per video — and can't queue an itag the
video doesn't offer. `output_path` falls back to the user's default output
folder, then to the OS Downloads folder, so the common case needs no path
at all.

### Packaging: mcp-server as a Tauri sidecar

The `mcp-server` binary ships *inside* the desktop app installer rather
than as a separate download, so "install DownloadHub" is a single step and
the binary is guaranteed to match the app version. It's declared as a
Tauri `externalBin` sidecar (`bundle.externalBin: ["binaries/mcp-server"]`
in `tauri.conf.json`); `tauri build` embeds `src-tauri/binaries/
mcp-server-<target-triple>` next to the main app executable in the bundle
(Tauri strips the triple suffix at bundle time). The `sidecar` recipe in
the root `justfile` (a dependency of `just release`, which is also what
`.github/workflows/build-windows.yml` runs) builds `mcp-server` in release
and copies it to that triple-suffixed path first — Tauri validates the file
exists at compile time, so it must run before `tauri build`. The staged
binaries are gitignored build artifacts.

The app doesn't *spawn* the sidecar itself (so no `tauri-plugin-shell`
dependency) — external MCP clients like Claude Desktop spawn it by absolute
path. The `mcp_server_path` command resolves that path from the running
app's own `current_exe()` location (correct wherever the user installed
it), which the Settings dialog surfaces along with a copy-paste client
config block. Under `tauri dev` the sidecar isn't staged next to the debug
binary, so the shown path won't exist until a real build — that's fine, the
connect UI is only actionable in an installed build anyway.

## License

This project depends on [`y7dl`](https://github.com/erwin-lovecraft/y7dl)
(GPL-3.0-or-later), so the whole project is licensed GPL-3.0-or-later — see
[`LICENSE`](../LICENSE). No proprietary/closed dependency may be added to
any crate that links against `y7dl` (i.e. `core/` and anything that depends
on it).
