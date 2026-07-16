import { invoke } from "@tauri-apps/api/core";

/** Absolute path to the bundled mcp-server binary (next to the app). */
export const mcpServerPath = () => invoke<string>("mcp_server_path");

/**
 * A ready-to-paste MCP client config block (Claude Desktop / Gemini CLI
 * shape) pointing at this install's mcp-server. `YOUTUBE_API_KEY` is left as
 * a placeholder for the user to fill (only needed for search).
 */
export function buildAgentConfig(serverPath: string): string {
  return JSON.stringify(
    {
      mcpServers: {
        downloadhub: {
          command: serverPath,
        },
      },
    },
    null,
    2,
  );
}
