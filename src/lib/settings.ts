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
   * Path to a Netscape cookies.txt file handed to yt-dlp via --cookies.
   * Works around YouTube's "confirm you're not a bot" gate. A path rather
   * than the cookie text: yt-dlp rewrites the file with whatever YouTube
   * rotated, and keeping our own copy threw those refreshed cookies away.
   */
  ytdlp_cookies_path: string | null;
}

/** The result of checking a cookies file, from the `check_ytdlp_cookies` command. */
export interface CookieCheck {
  /** The file parsed and YouTube served a probe request without challenging it. */
  ok: boolean;
  summary: string;
  /** What's wrong with the file, if anything — worth showing even when `ok`. */
  problems: string[];
}

/**
 * Checks a cookies file before it's saved: its Netscape-format shape, then
 * one real yt-dlp request to see whether YouTube actually accepts it.
 */
export const checkYtdlpCookies = (path: string) =>
  invoke<CookieCheck>("check_ytdlp_cookies", { path });

export const getSettings = () => invoke<AppSettings>("get_settings");

export const saveSettings = (settings: AppSettings) =>
  invoke<void>("save_settings", { newSettings: settings });
