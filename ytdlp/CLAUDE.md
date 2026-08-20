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

**`core` depends on this crate directly** (`core/Cargo.toml`), the same
position `y7dl` used to hold — metadata/format lookup isn't optional the way
MP3 transcoding is (`transcode` depends on `core` instead, the other
direction), since `mcp-server` needs video lookups but never needs a
transcoder.

## Shape

- `YtDlp` wraps one resolved binary path plus an optional cookies file path
  (both resolved by the caller — see `core::stream::resolve_ytdlp_config`).
  Cheap to construct; every call spawns a fresh short-lived process.
- `fetch_video` runs `yt-dlp -J --skip-download` and parses the JSON blob into
  `Video`/`Format`. Formats whose `format_id` doesn't parse as a plain `u32`
  (storyboards, `sb0`/`sb1`) are dropped during conversion — YouTube's real
  stream formats all have a numeric id, which not coincidentally is the same
  number the old itag was.
- `download` runs `yt-dlp -f <itag> -o <dest>` and parses `--progress-template`
  output lines for byte counts, calling back unthrottled — `core::download`
  owns the throttling policy, this crate just reports what yt-dlp says.
- `locate_ytdlp()` mirrors `downloadhub_transcode::locate_ffmpeg()`: next to
  the current executable (where Tauri stages the sidecar), then PATH.

## Cookies

YouTube sometimes requires sign-in verification ("confirm you're not a bot")
before serving formats or streams at all. The user pastes a Netscape
`cookies.txt` export into Settings; `core::stream::resolve_ytdlp_config`
writes it to `<app-data-dir>/cookies.txt` and this crate passes `--cookies
<path>` whenever a path is given. `classify_error` recognizes yt-dlp's
bot-check stderr text and surfaces `Error::BotCheckRequired` with a message
pointing at Settings, rather than a generic process-failure error.

## Errors

`Error` crosses into `core::stream::StreamError` via a `From` impl
(`core/src/stream/mod.rs`) the same way `y7dl::Error` used to. `classify_error`
does best-effort pattern matching on yt-dlp's stderr text (there's no
structured error output) to distinguish bot-check, video-unavailable, and
generic process failures — a coarse heuristic, not something to lean on for
anything beyond a clearer user-facing message.

## Testing

Progress-line parsing, target normalization (bare id vs. URL), stderr
classification, and the JSON→`Video` conversion (including the numeric-id
filter) are all pure and unit-tested without a real yt-dlp binary. There's no
fake-binary integration test here the way `transcode` has one for ffmpeg —
yt-dlp's actual CLI behavior against real YouTube isn't something a shell
script stand-in can usefully simulate.
