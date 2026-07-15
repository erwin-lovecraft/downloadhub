# downloadhub

An AI-powered YouTube downloader desktop app: log in with Google, search
YouTube, build a download queue (video + format + quality), and download
videos. Also exposes an MCP server so external AI agents can propose
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

## Google OAuth setup (dev)

Login uses the installed-app/loopback flow, so no server-side redirect URI
needs to be pre-registered — any `http://127.0.0.1:<port>` is accepted by a
"Desktop app" OAuth client.

1. In the [Google Cloud Console](https://console.cloud.google.com/apis/credentials),
   create an OAuth 2.0 Client ID with application type **Desktop app**.
2. Copy [`.env.example`](.env.example) to `.env` (gitignored) and fill in
   `GOOGLE_OAUTH_CLIENT_ID` / `GOOGLE_OAUTH_CLIENT_SECRET` from that client.
3. Run `pnpm tauri dev`. The `.env` file is loaded automatically on startup.

Tokens are stored in the OS keychain via the `keyring` crate, never in a
plaintext file. Without a `.env`/env vars set, the app still runs — the
"Sign in with Google" button will just report that OAuth isn't configured.

## YouTube search setup (dev)

Keyword search and playlist import call `search.list`/`videos.list`/
`playlistItems.list` with a plain API key (no OAuth needed for this).

1. In the same [Google Cloud Console project](https://console.cloud.google.com/apis/credentials),
   create an **API key** and enable the **YouTube Data API v3** for the
   project.
2. Set `YOUTUBE_API_KEY` in your `.env` (see [`.env.example`](.env.example)).

Without it set, the app still runs — search reports that it isn't
configured.

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

## Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)
