import { invoke } from "@tauri-apps/api/core";
import type { FormatPreference, ReformatOutcome } from "@/lib/enqueue";

export type QueueStatus = "queued" | "downloading" | "completed" | "failed" | "cancelled";

export interface QueueEntry {
  id: number;
  video_id: string;
  title: string;
  itag: number;
  quality_label: string | null;
  output_path: string;
  convert_to_mp3: boolean;
  status: QueueStatus;
  error_message: string | null;
  created_at: number;
}

/**
 * Stand-in itag on an MP3 entry whose source stream yt-dlp picks at download
 * time, because nothing in the video's format list matched (mirrors
 * `core::stream::AUTO_AUDIO_ITAG`). Not a real itag, so it isn't worth
 * showing the user a number for.
 */
export const AUTO_AUDIO_ITAG = 0;

/** What an entry's format line reads as. */
export function formatDescription(entry: QueueEntry): string {
  const source = entry.itag === AUTO_AUDIO_ITAG ? "best audio" : `itag ${entry.itag}`;
  return entry.convert_to_mp3
    ? `mp3 (from ${source})`
    : `${entry.quality_label ?? "audio"} (${source})`;
}

export interface AddToQueueParams {
  videoId: string;
  title: string;
  itag: number;
  qualityLabel: string | null;
  outputPath: string;
  convertToMp3?: boolean;
  [key: string]: unknown;
}

export const addToQueue = (params: AddToQueueParams) => invoke<QueueEntry>("add_to_queue", params);

export const listQueue = () => invoke<QueueEntry[]>("list_queue");

export const removeFromQueue = (queueId: number) =>
  invoke<void>("remove_from_queue", { queueId });

export const clearQueue = () => invoke<void>("clear_queue");

export interface SetQueueEntryFormatParams {
  queueId: number;
  itag: number;
  qualityLabel: string | null;
  convertToMp3: boolean;
  [key: string]: unknown;
}

/** Repoints one entry at an exact itag the user picked for that video. */
export const setQueueEntryFormat = (params: SetQueueEntryFormatParams) =>
  invoke<QueueEntry>("set_queue_entry_format", params);

export interface SetQueueEntriesQualityParams {
  queueIds: number[];
  preference: FormatPreference;
  [key: string]: unknown;
}

/**
 * Re-resolves several entries against one quality preference. A preference
 * rather than an itag, because a multi-select spans videos whose available
 * itags differ — the backend resolves each video individually.
 */
export const setQueueEntriesQuality = (params: SetQueueEntriesQualityParams) =>
  invoke<ReformatOutcome>("set_queue_entries_quality", params);
