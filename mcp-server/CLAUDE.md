# mcp-server/ — `downloadhub-mcp-server`

A standalone binary exposing downloadhub to external AI agents over MCP/stdio.
Depends on `downloadhub-core` so it reuses the exact same queue manager and
persistence as the desktop app; it does **not** depend on `transcode` and links
no ffmpeg code.

Registration for Claude Desktop / Claude Code / Gemini CLI / Codex CLI is in
[`../docs/MCP_SETUP.md`](../docs/MCP_SETUP.md).

## The one rule

**This server can search and it can queue. It must never be able to download.**

There is deliberately no tool that starts a transfer. The queue is a *proposal*
the user reviews in the desktop app, where "Download all" is the single human
action that spends bandwidth and writes media files. Adding a queue row is cheap
and reversible; starting a download is neither.

This is enforced by the **tool surface**, not a runtime check — an approval check
is a condition that must hold, whereas a missing capability cannot be bypassed by
a bug, a jailbreak, or a prompt injection. `core::download` is not even reachable
from the tool router. Do not add a download tool, and do not add a dependency
that would make one easy.

## Tools

| Tool | Kind |
| --- | --- |
| `search_videos` | read-only; needs `YOUTUBE_API_KEY` via the client's `env` block |
| `get_video_formats` | read-only; no configuration |
| `list_queue` | read-only; reads the shared database |
| `add_to_queue` | mutating; executes immediately |
| `add_mp3_to_queue` | mutating; executes immediately |
| `remove_from_queue` | mutating; executes immediately |

Design constraints to preserve when touching these:

- **Add tools take a list of videos**, not one per call. An agent queueing a
  ten-track album should spend one round-trip, not ten. Per-video failures go in
  `skipped` and never sink the batch.
- **Add tools take a `FormatPreference`, never an itag.** They resolve it against
  each video's real format list server-side via `core::enqueue`, so agents don't
  need `get_video_formats` first and can't queue an itag a video doesn't offer.
- **`add_mp3_to_queue` stays its own tool** rather than collapsing into
  `add_to_queue(quality: "mp3")` (which also works). MP3 is the common request,
  and a purpose-named tool gets selected far more reliably than an enum value
  buried in another tool's schema.
- **`output_path` is optional**, falling back to the settings default, then the
  OS Downloads folder.

## Gating

Every tool calls `ensure_enabled()` first, which re-reads `settings.json` on
**every call** rather than caching at startup — so toggling "Allow AI agent
access (MCP server)" in the running app takes effect immediately with no restart
or cross-process signal. That switch is wholesale: on or off.

## stdio discipline

stdout belongs to the JSON-RPC protocol. **Never print to stdout** — no
`println!`, no logger defaulting there. Diagnostics go to stderr.

## rmcp API (2.x)

The SDK's pre-1.0 docs floating around online do not apply. Current pattern:

- The struct holds `tool_router: ToolRouter<Self>`; `#[tool_router(router =
  tool_router)]` on the impl carrying `#[tool]` methods; `#[tool_handler(router
  = self.tool_router)]` on `impl ServerHandler`.
- Params use the `Parameters<T>` wrapper, `T: Deserialize + schemars::JsonSchema`
  (schemars 1.x, a direct dependency). Put agent-facing text in
  `#[schemars(description = ...)]` — it's what the model actually reads.
- Tool fns return `Result<String, String>`; an `Err` becomes an `isError` tool
  result, so error strings should tell the agent what to do next.
- `ServerInfo::new(caps)`, then mutate `.instructions`/`.server_info`.
  `Implementation` is `non_exhaustive` — use `Implementation::new(name,
  version)`.
- stdio transport is behind the `transport-io` feature.

## Packaging

Ships as a Tauri `externalBin` sidecar inside the desktop installer, so it always
matches the app version. `just sidecar` builds it in release and copies it to
`src-tauri/binaries/mcp-server-<target-triple>` — Tauri validates that file
exists at compile time, so it must run before `tauri build`. The app never spawns
it; MCP clients do, by absolute path, which the Settings dialog surfaces via the
`mcp_server_path` command.
