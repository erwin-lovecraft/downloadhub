# downloadhub

An AI-powered YouTube downloader desktop app: search YouTube, build a
download queue (video + format + quality), and download videos. Also
exposes an MCP server so external AI agents can propose
playlists and queue downloads on the user's behalf, subject to explicit
user approval.

Built with Tauri v2, React + TypeScript, and Rust. See
[`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) for the Cargo workspace
layout and architecture decisions.

## License

GPL-3.0-or-later — see [`LICENSE`](LICENSE).

This project's download engine depends on
[`y7dl`](https://github.com/erwin-lovecraft/y7dl) (GPL-3.0-or-later), which
in turn is built on [`kkdai/youtube`](https://github.com/kkdai/youtube).
Because of this, the whole project is GPL-licensed; no proprietary/closed
dependency may be added to any crate that links against `y7dl`.

Installers additionally bundle a static GPL build of
[FFmpeg](https://ffmpeg.org) as a sidecar, used to convert downloaded
audio to MP3 (see [MP3 conversion](#mp3-conversion-ffmpeg-sidecar)).
FFmpeg is a trademark of Fabrice Bellard; its GPL license matches this
project's.

## YouTube search setup (dev)

Keyword search and playlist import call `search.list`/`videos.list`/
`playlistItems.list` with a plain API key (no OAuth needed for this).

1. In the same [Google Cloud Console project](https://console.cloud.google.com/apis/credentials),
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
it with a bundled **ffmpeg** sidecar. The binaries are static GPL builds
*vendored in the repo* (`tools/`) rather than fetched by any package
manager or build step:

| Platform | Vendored binary |
| --- | --- |
| macOS (x86_64; Rosetta on Apple Silicon) | `tools/ffmpeg-macos-x86_64` |
| Windows x64 | `tools/ffmpeg-windows-x86_64.exe` |

`just sidecar` (and therefore `just release`, locally and in the
[`build-windows`](.github/workflows/build-windows.yml) workflow) copies
the current platform's binary to
`src-tauri/binaries/ffmpeg-<triple>[.exe]`, where `tauri build` picks it
up as an `externalBin` sidecar. To upgrade ffmpeg, replace the file in
`tools/` (keeping the name) and commit.

Under `tauri dev` no sidecar staging happens; the app falls back to any
`ffmpeg` found on PATH (e.g. Homebrew's), and if none is found, MP3
entries fail with a clear message while everything else keeps working.

## Video format/quality lookup

Fetching a video's available formats/qualities uses `y7dl` directly against
YouTube's InnerTube API — no API key or OAuth needed, and nothing to
configure.

## Download queue

The queue is a local SQLite database at
`<platform-data-dir>/downloadhub/queue.sqlite3` (bundled SQLite via
`rusqlite`, no separate install needed). Nothing to configure; the file and
its parent directory are created automatically on first run. The output
folder for each entry can be typed directly or picked via the native
folder dialog ("Browse..." next to the field).

Each queued entry has a "Start" button that downloads it via `y7dl`'s
ranged `Range`-request chunking, with a live progress bar streamed from the
backend. No config needed. A DASH (adaptive) format saves as
`<title>.video.<ext>` and/or `<title>.audio.<ext>` in the output folder
(no muxing yet); a progressive format saves as a single `<title>.<ext>`.

## AI agent access (MCP server)

External AI agents (Claude Desktop, Claude Code, Gemini CLI, Codex CLI, …)
can search YouTube and propose downloads through the bundled MCP server
(`cargo build --release -p downloadhub-mcp-server`). Anything an agent
requests that would change the queue or start a download waits as a
pending request in the app's "AI agent requests" panel until you approve
or reject it — nothing runs unattended. Agent access can be switched off
entirely in the app's Settings dialog (it's on by default). Registration
instructions per agent live in [`docs/MCP_SETUP.md`](docs/MCP_SETUP.md).

## Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)
