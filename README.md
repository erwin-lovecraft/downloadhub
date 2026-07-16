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

For **release builds** the credentials don't ship as a `.env` — they're
baked into the binary at compile time instead. See
[Release builds](#release-builds-embedded-credentials) below.

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

A shipped installer can't rely on a user-provided `.env`, so the three
credentials above are resolved in two steps (see
[`core::secrets`](core/src/secrets.rs)):

1. **Runtime environment first** — a local `.env` (via `dotenvy`) or any real
   env var. This is what dev uses; nothing changes there.
2. **Compile-time fallback** — whatever `GOOGLE_OAUTH_CLIENT_ID`,
   `GOOGLE_OAUTH_CLIENT_SECRET`, and `YOUTUBE_API_KEY` are set to *when cargo
   compiles* is embedded into the binary (`option_env!`). A release build
   with no `.env` present still carries them.

The [`build-windows`](.github/workflows/build-windows.yml) workflow supplies
these at build time from **GitHub Actions repository secrets** of the same
names (Settings → Secrets and variables → Actions). No credential is
committed to the repo.

> **Note:** embedding is not encryption — the values are recoverable from the
> shipped binary (e.g. `strings`). For a desktop app this is an accepted
> tradeoff: a Google "Desktop app" OAuth client id/secret are not
> confidential by design ([RFC 8252](https://datatracker.ietf.org/doc/html/rfc8252)),
> and the YouTube API key is protected by API/quota restrictions set in the
> Google Cloud Console rather than by secrecy.

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
