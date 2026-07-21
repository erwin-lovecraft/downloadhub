import { invoke } from "@tauri-apps/api/core";
import type { VideoSummary } from "@/lib/youtube";
import type { EnqueueOutcome, FormatPreference } from "@/lib/enqueue";

export const listPlaylistItems = (playlistUrlOrId: string) =>
  invoke<VideoSummary[]>("list_playlist_items", { playlistUrlOrId });

export interface ImportPlaylistParams {
  videoIds: string[];
  preference: FormatPreference;
  outputPath: string;
  [key: string]: unknown;
}

export const importPlaylistToQueue = (params: ImportPlaylistParams) =>
  invoke<EnqueueOutcome>("import_playlist_to_queue", params);
