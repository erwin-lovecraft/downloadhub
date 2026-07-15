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
`BestAudioOnly`) is a two-option quality *shortcut*: `core::playlist`
resolves each selected video's own format list against it individually
(`StreamClient::resolve_preferred_format`, reusing the exact same
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

## License

This project depends on [`y7dl`](https://github.com/erwin-lovecraft/y7dl)
(GPL-3.0-or-later), so the whole project is licensed GPL-3.0-or-later — see
[`LICENSE`](../LICENSE). No proprietary/closed dependency may be added to
any crate that links against `y7dl` (i.e. `core/` and anything that depends
on it).
