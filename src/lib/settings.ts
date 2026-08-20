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
  /**
   * Custom path to a yt-dlp binary. null falls back to the bundled
   * sidecar or a yt-dlp found on PATH.
   */
  ytdlp_path: string | null;
  /**
   * Cookies to pass to yt-dlp (Netscape cookies.txt format), pasted by
   * the user. Works around YouTube's "confirm you're not a bot" gate.
   */
  ytdlp_cookies: string | null;
}

export const getSettings = () => invoke<AppSettings>("get_settings");

export const saveSettings = (settings: AppSettings) =>
  invoke<void>("save_settings", { newSettings: settings });
