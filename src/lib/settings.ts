import { invoke } from "@tauri-apps/api/core";
import type { FormatPreference } from "@/lib/playlist";

export interface AppSettings {
  default_output_path: string | null;
  default_quality: FormatPreference;
  /** Whether the MCP server serves external AI agents (default true). */
  mcp_enabled: boolean;
}

export const getSettings = () => invoke<AppSettings>("get_settings");

export const saveSettings = (settings: AppSettings) =>
  invoke<void>("save_settings", { newSettings: settings });
