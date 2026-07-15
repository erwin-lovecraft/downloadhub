# Registering the DownloadHub MCP server with an external agent

DownloadHub ships an MCP (Model Context Protocol) server binary so external
AI agents — Claude Desktop, Claude Code, Gemini CLI, Codex CLI, or anything
else that speaks MCP over stdio — can search YouTube and propose downloads.

**Nothing an agent asks for runs unattended.** Tools that would change the
queue or start a download only record a *pending request*; you approve or
reject each one in the running DownloadHub desktop app (the "AI agent
requests" panel above the download queue). Read-only tools answer directly.

## 1. Build the server binary

From the repository root:

```sh
cargo build --release -p downloadhub-mcp-server
```

The binary lands at `target/release/mcp-server`. The examples below refer to
it as `/path/to/downloadhub/target/release/mcp-server` — substitute your
checkout's absolute path.

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

Approval-gated (record a pending request; nothing happens until you approve
it in the desktop app):

- `add_to_queue` — `video` + `itag` (+ optional `output_path`, defaulting
  to your configured default output folder). The server resolves the
  video's real title/quality itself, so the approval prompt describes the
  actual video rather than trusting the agent's wording.
- `start_download` — start one existing queue entry by `queue_id`.
- `download_all` — download every queued entry, sequentially.

## 5. The approval flow

1. The agent calls e.g. `add_to_queue`; the tool result says the request is
   `awaiting_user_approval`.
2. Open (or switch to) the DownloadHub desktop app. The "AI agent requests"
   panel lists the request with what it will do and which client asked.
3. **Approve** executes it exactly as if you'd clicked the equivalent
   button yourself (same guards — e.g. it's refused while a batch download
   runs); **Reject** discards it. Either way the decision is recorded, and
   an already-decided request can never execute again or twice.
4. The agent sees the outcome by calling `list_queue`.

Pending requests are stored in the shared queue database, so the desktop
app doesn't need to be running when the agent makes the request — the
requests simply wait until you next open the app.
