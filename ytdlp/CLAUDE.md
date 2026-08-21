# ytdlp/ — `downloadhub-ytdlp`

Video metadata lookup and downloads via an external `yt-dlp` process — not
linked as a library. Replaced `y7dl` (a Rust InnerTube client) after YouTube's
extraction changes broke it; yt-dlp is actively maintained against exactly
this kind of breakage.

## Why a subprocess, not a library

Every other option here is a Rust crate re-implementing YouTube's InnerTube
protocol (what `y7dl` did) — a protocol YouTube changes without notice and
that breaks the moment it does. yt-dlp is maintained upstream specifically to
track those changes; wrapping its CLI trades a small amount of process-spawn
overhead for not owning that maintenance burden ourselves.

## Dependency direction

**This crate depends on `core`** (`ytdlp/Cargo.toml`), the same direction
`transcode` uses for ffmpeg: `core::stream` defines the object-safe
`StreamProvider` trait, and `YtDlpProvider` (in `src/provider.rs`) implements
it. `core` has no dependency on this crate at all — it only knows the trait.

This used to run the other way (`core` depended on `ytdlp` directly, the
position `y7dl` used to hold), on the reasoning that metadata/format lookup
isn't optional the way MP3 transcoding is, since `mcp-server` needs video
lookups but never needs a transcoder. That's still true, but it's not actually
in tension with the trait seam: `mcp-server` and `src-tauri` each depend on
`ytdlp` directly and construct `YtDlpProvider` themselves at startup (see
`AppState::from_env` in `src-tauri`, `DownloadHub::new` in `mcp-server`) —
there are two wiring sites instead of one, which the trait indirection doesn't
avoid, but `core` stays free of any concrete process-spawning dependency in
exchange.

## Shape

- `YtDlp` wraps one resolved binary path plus an optional cookies file path
  (both resolved by the caller — see `YtDlpProvider::resolve` in
  `src/provider.rs`, which reads a `core::stream::YtDlpConfig`). Cheap to
  construct; every call spawns a fresh short-lived process.
- `fetch_video` runs `yt-dlp -J --skip-download` and parses the JSON blob into
  private `Video`/`Format` types. Formats whose `format_id` doesn't parse as a
  plain `u32` (storyboards, `sb0`/`sb1`) are dropped during conversion —
  YouTube's real stream formats all have a numeric id, which not coincidentally
  is the same number the old itag was. `YtDlpProvider::get_video` maps these
  into `core`'s `VideoDetail`/`FormatSummary` DTOs, which is the only place
  this crate's types cross into `core`'s.
- `download` runs `yt-dlp -f <itag> -o <dest>` and parses `--progress-template`
  output lines for byte counts, calling back unthrottled — `core::download`
  owns the throttling policy, this crate just reports what yt-dlp says.
- `locate_ytdlp()` mirrors `downloadhub_transcode::locate_ffmpeg()`: next to
  the current executable (where Tauri stages the sidecar), then PATH.
- `YtDlpProvider` (`src/provider.rs`) is the `StreamProvider` impl: it resolves
  a `YtDlp` fresh per call from the caller's `YtDlpConfig`, calls
  `fetch_video`/`download`, and maps this crate's `Error` into
  `core::stream::StreamError` via a plain function (`map_error`) — not a
  `From` impl, since the orphan rules don't allow `impl From<local::Error> for
  foreign::StreamError` from this side.

## Cookies

YouTube sometimes requires sign-in verification ("confirm you're not a bot")
before serving formats or streams at all. The user pastes a Netscape
`cookies.txt` export into Settings; `core::stream::resolve_ytdlp_config`
writes it to `<app-data-dir>/cookies.txt` and this crate passes `--cookies
<path>` whenever a path is given. `classify_error` recognizes yt-dlp's
bot-check stderr text and surfaces `Error::BotCheckRequired` with a message
pointing at Settings, rather than a generic process-failure error.

## Errors

`Error` crosses into `core::stream::StreamError` via `map_error` in
`src/provider.rs` (a plain function, not a `From` impl — see "Dependency
direction" above). `classify_error` does best-effort pattern matching on
yt-dlp's stderr text (there's no structured error output) to distinguish
bot-check, video-unavailable, and generic process failures — a coarse
heuristic, not something to lean on for anything beyond a clearer user-facing
message.

## Testing

Progress-line parsing, target normalization (bare id vs. URL), stderr
classification, and the JSON→`Video` conversion (including the numeric-id
filter) are all pure and unit-tested without a real yt-dlp binary. There's no
fake-binary integration test here the way `transcode` has one for ffmpeg —
yt-dlp's actual CLI behavior against real YouTube isn't something a shell
script stand-in can usefully simulate.
