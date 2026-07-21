import { invoke } from "@tauri-apps/api/core";
import type { FormatPreference } from "@/lib/enqueue";

export interface AppSettings {
  default_output_path: string | null;
  default_quality: FormatPreference;
  /** Whether the MCP server serves external AI agents (default true). */
  mcp_enabled: boolean;
  /**
   * Custom path to an ffmpeg binary for MP3 conversion. null falls back
   * to the bundled sidecar (Windows) or an ffmpeg found on PATH.
   */
  ffmpeg_path: string | null;
}

export const getSettings = () => invoke<AppSettings>("get_settings");

export const saveSettings = (settings: AppSettings) =>
  invoke<void>("save_settings", { newSettings: settings });
