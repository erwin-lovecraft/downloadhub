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

## Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)
