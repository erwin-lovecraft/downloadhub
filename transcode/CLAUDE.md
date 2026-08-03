# transcode/ — `downloadhub-transcode`

Audio transcoding for the MP3 download feature. A thin `tokio::process` wrapper
around an external `ffmpeg` binary — that is the whole crate.

## Why it's a separate crate

So the process-spawning boundary stays isolated and independently testable, and
so `mcp-server` (which depends only on `core`) never links code it doesn't use.

## Dependency direction

`core::download` defines the object-safe `Transcode` trait. **This crate depends
on `core`** and implements that trait for `Transcoder`; `core` has no dependency
on this crate. `src-tauri` depends on both and hands `core::download` a
`&dyn Transcode`.

Keep the arrow pointing this way. `core` decides *when* a conversion happens;
this crate decides *how*.

## ffmpeg is not linked

It is an external binary invoked as a child process, never a linked library.
`locate_ffmpeg()` looks next to the app executable (the bundled sidecar), then
on PATH; the caller (`AppState::resolve_transcoder`) tries the user's
`ffmpeg_path` setting first and re-resolves **per download start**, so a settings
change needs no restart.

Command: `ffmpeg -i in.m4a -vn -codec:a libmp3lame -q:a 2 out.mp3` — LAME VBR
~190 kbps, transparent for a 128 kbps AAC source without wasting space on a fixed
320k.

The child is spawned with `kill_on_drop`, so cancelling a download mid-transcode
kills ffmpeg rather than orphaning it. Don't remove that.

## Platform notes

- **Windows:** a static GPL build is vendored in the repo at
  `tools/ffmpeg-windows-x86_64.exe` — a deliberately committed binary, so builds
  are reproducible from a bare checkout with no fetch step. `just sidecar` copies
  it to `src-tauri/binaries/ffmpeg-<triple>.exe`.
- **macOS:** nothing is bundled. An unsigned vendored binary gets blocked by
  Gatekeeper, so macOS users set the `ffmpeg_path` setting or rely on PATH.

## Errors

`TranscodeError` crosses the `Transcode` seam as a boxed error that `core` only
displays (as the queue entry's failure message) and never matches on. On failure
the source m4a is deliberately kept, so downloaded data isn't lost — that
deletion is `core`'s call, made only after a successful transcode.

## License

ffmpeg GPL builds match this project's GPL-3.0-or-later license, which is why
using a GPL build is fine here.
