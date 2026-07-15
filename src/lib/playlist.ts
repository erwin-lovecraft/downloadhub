import { invoke } from "@tauri-apps/api/core";
import type { VideoSummary } from "@/lib/youtube";
import type { QueueEntry } from "@/lib/queue";

export type FormatPreference = "best_progressive" | "best_audio_only";

export interface PlaylistImportSkip {
  video_id: string;
  reason: string;
}

export interface PlaylistImportOutcome {
  added: QueueEntry[];
  skipped: PlaylistImportSkip[];
}

export const listPlaylistItems = (playlistUrlOrId: string) =>
  invoke<VideoSummary[]>("list_playlist_items", { playlistUrlOrId });

export interface ImportPlaylistParams {
  videoIds: string[];
  preference: FormatPreference;
  outputPath: string;
  [key: string]: unknown;
}

export const importPlaylistToQueue = (params: ImportPlaylistParams) =>
  invoke<PlaylistImportOutcome>("import_playlist_to_queue", params);
