# downloadhub

An AI-powered YouTube downloader desktop app: search YouTube, build a
download queue (video + format + quality), and download videos. Also
exposes an MCP server so external AI agents can search and fill the queue
on the user's behalf — while never being able to start a download.

Built with Tauri v2, React + TypeScript, and Rust. See
[`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) for the Cargo workspace
layout and architecture decisions.

## License

GPL-3.0-or-later — see [`LICENSE`](LICENSE).

The download engine uses [`yt-dlp`](https://github.com/yt-dlp/yt-dlp)
(Unlicense/public domain) as an external process, not a linked library —
see [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) for why. yt-dlp doesn't
require this project to be GPL-licensed; GPL-3.0-or-later is a choice the
project keeps regardless (see the License section of `ARCHITECTURE.md` for
the history — an earlier, now-removed dependency did force it).

Windows installers additionally bundle a static GPL build of
[FFmpeg](https://ffmpeg.org) as a sidecar, used to convert downloaded
audio to MP3 (see [MP3 conversion](#mp3-conversion-ffmpeg-sidecar)).
FFmpeg is a trademark of Fabrice Bellard; its GPL license matches this
project's.

## YouTube search setup (dev)

Keyword search and playlist import call `search.list`/`videos.list`/
`playlistItems.list` with a plain API key (no OAuth needed for this).

1. In a [Google Cloud Console project](https://console.cloud.google.com/apis/credentials),
   create an **API key** and enable the **YouTube Data API v3** for the
   project.
2. Set `YOUTUBE_API_KEY` in your `.env` (see [`.env.example`](.env.example)).

Without it set, the app still runs — search reports that it isn't
configured. In release builds this key is embedded at compile time (see
[Release builds](#release-builds-embedded-credentials)).

## Release builds (embedded credentials)

A shipped installer can't rely on a user-provided `.env`, so
`YOUTUBE_API_KEY` is resolved in two steps (see
[`core::secrets`](core/src/secrets.rs)):

1. **Runtime environment first** — a local `.env` (via `dotenvy`) or any real
   env var. This is what dev uses; nothing changes there.
2. **Compile-time fallback** — whatever `YOUTUBE_API_KEY` is set to *when
   cargo compiles* is embedded into the binary (`option_env!`). A release
   build with no `.env` present still carries it.

Note that `option_env!` reads the **process environment at compile time**,
not the `.env` file — the [`justfile`](justfile) bridges that gap: `just
release` (via just's `dotenv-load` setting) exports the workspace-root
`.env` into the environment before invoking cargo, so a local release build
embeds your `.env` value with no manual `export`/`$env:` step. Real env
vars always take precedence over `.env`.

The [`build-windows`](.github/workflows/build-windows.yml) workflow runs the
same `just release` and supplies the value as a real env var from a
**GitHub Actions repository secret** of the same name (Settings → Secrets
and variables → Actions). No credential is committed to the repo.

> **Note:** embedding is not encryption — the value is recoverable from the
> shipped binary (e.g. `strings`). For a desktop app this is an accepted
> tradeoff: the YouTube API key is protected by API/quota restrictions set in
> the Google Cloud Console rather than by secrecy.

## MP3 conversion (ffmpeg sidecar)

The "Download MP3" button downloads the itag-140 m4a stream and transcodes
it with an external **ffmpeg**, resolved per download (no restart needed)
in this order:

1. **Custom path from Settings** — "ffmpeg path (MP3 conversion)" in the
   Settings dialog. This is the way to enable MP3 on macOS today.
2. **Bundled sidecar** — Windows installers ship a static GPL build
   vendored at `tools/ffmpeg-windows-x86_64.exe` (committed rather than
   fetched by any package manager or build step; `just sidecar` copies it
   to `src-tauri/binaries/ffmpeg-<triple>.exe`, and
   [`tauri.windows.conf.json`](src-tauri/tauri.windows.conf.json) adds it
   to `externalBin` on Windows only). To upgrade ffmpeg, replace the file
   in `tools/` keeping the name, and commit.
3. **PATH** — any `ffmpeg` found on PATH (e.g. Homebrew's), the usual dev
   fallback.

macOS builds deliberately bundle no ffmpeg for now: an unsigned vendored
binary gets blocked by Gatekeeper, so a previously-vendored macOS binary
was removed — macOS users point the Settings option at their own ffmpeg
(e.g. `brew install ffmpeg`) instead. If no ffmpeg is found anywhere, MP3
entries fail with a clear message while everything else keeps working.

## Video download engine (yt-dlp sidecar)

All video/format lookup and every download runs through an external
**yt-dlp**, resolved per call (no restart needed) in the same order as
ffmpeg above:

1. **Custom path from Settings** — "yt-dlp path" in the Settings dialog.
2. **Bundled sidecar** — Windows and macOS installers ship the vendored
   binaries at `tools/yt-dlp.exe` / `tools/yt-dlp_macos` (committed rather
   than fetched by any package manager or build step; `just sidecar` copies
   the right one to `src-tauri/binaries/yt-dlp-<triple>[.exe]`, declared in
   `externalBin` by
   [`tauri.windows.conf.json`](src-tauri/tauri.windows.conf.json) /
   [`tauri.macos.conf.json`](src-tauri/tauri.macos.conf.json)). To upgrade
   yt-dlp, replace the file in `tools/` keeping the name, and commit.
3. **PATH** — any `yt-dlp` found on PATH (e.g. `pip install yt-dlp` or
   Homebrew's), the usual dev fallback and the only option on Linux, which
   has no vendored binary.

The bundled macOS binary is ad-hoc signed but not notarized. In local
testing this was a milder problem than the previously-vendored (fully
unsigned) ffmpeg build: the very first execution of a freshly-downloaded
copy could hang or fail while macOS ran its on-demand security check, but
every run after that succeeded normally with no user action needed. If
you hit a stuck or failed first search/download after installing, try
again, approve the binary once in System Settings → Privacy & Security if
prompted, or skip the sidecar entirely by setting "yt-dlp path" to a `brew
install yt-dlp` (or `pip install yt-dlp`) binary instead. If no yt-dlp is
found anywhere, search, format lookup, and downloads all fail with a clear
message.

### Cookies (working around "confirm you're not a bot")

YouTube sometimes demands sign-in verification before it will serve
formats or streams — yt-dlp reports this distinctly, and the app surfaces
it as a clear error pointing at Settings rather than a generic failure.
The workaround: export your `youtube.com` cookies from a signed-in browser
session in Netscape `cookies.txt` format (e.g. with a "Get cookies.txt
LOCALLY" browser extension) and paste the file's contents into "yt-dlp
cookies" in the Settings dialog. The app writes them to
`<platform-data-dir>/downloadhub/cookies.txt` and passes `--cookies` to
yt-dlp on every call; leave the field empty if you never hit this.

## Video format/quality lookup

Fetching a video's available formats/qualities uses `yt-dlp` (see above) —
no API key or OAuth needed, and nothing to configure beyond the sidecar.

## Download queue

The queue is a local SQLite database at
`<platform-data-dir>/downloadhub/queue.sqlite3` (bundled SQLite via
`rusqlite`, no separate install needed). Nothing to configure; the file and
its parent directory are created automatically on first run. The output
folder for each entry can be typed directly or picked via the native
folder dialog ("Browse..." next to the field).

Each queued entry has a "Start" button that downloads it via `yt-dlp`,
with a live progress bar streamed from the backend. No config needed
beyond the yt-dlp sidecar above. A DASH (adaptive) format saves as
`<title>.video.<ext>` and/or `<title>.audio.<ext>` in the output folder
(no muxing yet); a progressive format saves as a single `<title>.<ext>`.
An interrupted download resumes from where it left off the next time it's
started, rather than restarting from scratch — yt-dlp's own default
behavior.

## AI agent access (MCP server)

External AI agents (Claude Desktop, Claude Code, Gemini CLI, Codex CLI, …)
can search YouTube and fill the download queue through the bundled MCP
server (`cargo build --release -p downloadhub-mcp-server`).

**The server exposes no tool that can start a download.** Agents add
entries to the queue; you review them in the queue sidebar — real titles,
formats, and destinations — and change, remove, or download them. Clicking
"Download all" is the only thing that ever spends bandwidth or writes media
files. Agent access can also be switched off entirely in the app's Settings
dialog (it's on by default).

Registration instructions per agent live in
[`docs/MCP_SETUP.md`](docs/MCP_SETUP.md).

## Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)
