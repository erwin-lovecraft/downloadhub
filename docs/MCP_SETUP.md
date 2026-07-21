# Registering the DownloadHub MCP server with an external agent

DownloadHub ships an MCP (Model Context Protocol) server binary so external
AI agents — Claude Desktop, Claude Code, Gemini CLI, Codex CLI, or anything
else that speaks MCP over stdio — can search YouTube and propose downloads.

**No agent can download anything.** Agents can search, and they can add
entries to your download queue — that part happens immediately, with no
approval prompt. But the server exposes no tool that starts a transfer:
nothing is downloaded until *you* open the DownloadHub desktop app, look at
what's queued, and click **Download all**. The queue is the review step.

## 1. Get the server binary

**If you installed DownloadHub from an installer** (`.msi`/`.exe`/`.dmg`),
the `mcp-server` binary is already bundled *inside* the app — you don't
build anything. Open the app, go to **Settings → Connect an AI agent**, and
it shows the exact binary path plus a ready-to-paste config block with a
"Copy config" button. That path is what the examples below call
`/path/to/mcp-server`; on Windows it's typically
`C:\Program Files\downloadhub\mcp-server.exe`.

**If you're working from source instead**, build it directly:

```sh
cargo build --release -p downloadhub-mcp-server
```

The binary lands at `target/release/mcp-server` (`.exe` on Windows).
Building the whole app installer with the sidecar bundled is `just release`.

## 2. Configuration

| What | Why | Required? |
|------|-----|-----------|
| `YOUTUBE_API_KEY` env var | Only for the `search_videos` tool (same key the desktop app uses — see the README). Pass it via the MCP client's `env` block; a `.env` file only works when the server is launched from the repo root. | Optional |
| "Allow AI agent access (MCP server)" in the app's Settings dialog | Master switch (default **on**). When off, every tool call returns an error telling the agent access is disabled. Re-read on each call, so toggling needs no restarts. | — |

Format lookup, queue listing, and download requests need no configuration.
The server finds the same queue database and settings file as the desktop
app via the platform data directory (e.g.
`~/Library/Application Support/downloadhub` on macOS,
`~/.local/share/downloadhub` on Linux, `%APPDATA%\downloadhub` on Windows).

## 3. Register with your agent

### Claude Desktop

Edit the config file (macOS:
`~/Library/Application Support/Claude/claude_desktop_config.json`; Windows:
`%APPDATA%\Claude\claude_desktop_config.json`) and add under `mcpServers`:

```json
{
  "mcpServers": {
    "downloadhub": {
      "command": "/path/to/downloadhub/target/release/mcp-server",
      "env": {
        "YOUTUBE_API_KEY": "your-api-key"
      }
    }
  }
}
```

Restart Claude Desktop; the tools appear under the `downloadhub` server.

### Claude Code

```sh
claude mcp add downloadhub --env YOUTUBE_API_KEY=your-api-key \
  -- /path/to/downloadhub/target/release/mcp-server
```

### Gemini CLI

Add to `~/.gemini/settings.json` (or the project's `.gemini/settings.json`):

```json
{
  "mcpServers": {
    "downloadhub": {
      "command": "/path/to/downloadhub/target/release/mcp-server",
      "env": {
        "YOUTUBE_API_KEY": "your-api-key"
      }
    }
  }
}
```

### Codex CLI

Add to `~/.codex/config.toml`:

```toml
[mcp_servers.downloadhub]
command = "/path/to/downloadhub/target/release/mcp-server"
env = { "YOUTUBE_API_KEY" = "your-api-key" }
```

Omit the `env` block/line everywhere if you don't need `search_videos`.

## 4. Available tools

Read-only (execute directly):

- `search_videos` — keyword search (`query`, optional `max_results` 1–25).
  Needs `YOUTUBE_API_KEY`.
- `get_video_formats` — every downloadable format (itag, mime type,
  resolution, size) for a video URL or id.
- `list_queue` — the download queue with per-entry status.

Queue-changing (take effect immediately — but download nothing):

- `add_to_queue` — `videos` (a **list** of URLs or ids), optional `quality`
  (`best_progressive` / `best_audio_only` / `mp3`, defaulting to your
  configured default quality), optional `output_path` (defaulting to your
  default output folder, then your OS Downloads folder).
- `add_mp3_to_queue` — same, fixed to MP3. The right tool for music.
- `remove_from_queue` — drop entries by id, to undo a mistaken add.

The add tools resolve each video's actual formats server-side, so agents
never pass an itag and can't queue one a video doesn't offer. Both take a
list so queueing an album costs one call rather than one per track.

There is intentionally **no** `start_download` or `download_all` tool.

## 5. How a session goes

1. You ask your agent for something ("queue the top 5 lo-fi mixes as MP3").
2. It calls `search_videos`, then `add_mp3_to_queue` once with all five.
   The tool result confirms what was queued and tells the agent to send you
   to the app.
3. You open DownloadHub. The five entries are in the queue sidebar with
   real titles and formats. Change a format, drop one you don't want, then
   click **Download all**.

The queue lives in a database shared by both processes, so the desktop app
doesn't need to be running when the agent queues things — the entries are
simply waiting the next time you open it.

To turn agent access off entirely, uncheck **Allow AI agent access (MCP
server)** in Settings; every tool then refuses, with no restart needed.
