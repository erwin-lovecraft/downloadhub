import type { QueueEntry } from "@/lib/queue";

/**
 * Mirrors core::stream::FormatPreference — a quality shortcut for anything
 * acting on more than one video at a time, since the exact itags on offer
 * differ video to video. `mp3` downloads the audio stream and converts it
 * (needs ffmpeg; see the Settings dialog).
 */
export type FormatPreference = "best_progressive" | "best_audio_only" | "mp3";

export const FORMAT_PREFERENCE_LABELS: Record<FormatPreference, string> = {
  best_progressive: "Best quality (video + audio)",
  best_audio_only: "Audio only",
  mp3: "MP3",
};

/** One video that couldn't be resolved, and why. */
export interface EnqueueSkip {
  video_id: string;
  reason: string;
}

export interface EnqueueOutcome {
  added: QueueEntry[];
  skipped: EnqueueSkip[];
}

export interface ReformatOutcome {
  updated: QueueEntry[];
  skipped: EnqueueSkip[];
}
