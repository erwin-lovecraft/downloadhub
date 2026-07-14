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

## Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)
