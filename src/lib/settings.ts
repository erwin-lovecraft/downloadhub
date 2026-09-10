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
  /**
   * Which JavaScript engine yt-dlp may use for YouTube's "n" challenge.
   * YouTube hides playable format URLs behind a challenge that has to be
   * run, and yt-dlp ships the solver scripts but no engine — only Deno is
   * enabled by default, so a machine with Node and no Deno loses formats,
   * and a request carrying cookies fails outright.
   */
  ytdlp_js_runtime: JsRuntime;
  /**
   * The itag recorded on every newly queued entry, instead of the one the
   * chosen quality presumes (0 = let yt-dlp pick any audio stream for MP3,
   * 140 for audio-only, 18 for video). null uses those defaults.
   *
   * Adding to the queue never checks a video's real format list — that
   * would cost a yt-dlp launch per video, and the download re-fetches the
   * list anyway. A wrong itag surfaces as a failed download, fixed by
   * picking a real format from the entry's format list.
   */
  enqueue_itag: number | null;
}

/**
 * "auto" enables Node alongside yt-dlp's own default and lets it use
 * whichever engine it finds; "node"/"deno" force one; "off" passes no flag
 * at all, for a yt-dlp too old to know the option.
 */
export type JsRuntime = "auto" | "node" | "deno" | "off";

export const JS_RUNTIME_LABELS: Record<JsRuntime, string> = {
  auto: "Auto",
  node: "Node",
  deno: "Deno",
  off: "Off",
};

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
