import { invoke } from "@tauri-apps/api/core";

export type QueueStatus = "queued" | "downloading" | "completed" | "failed" | "cancelled";

export interface QueueEntry {
  id: number;
  video_id: string;
  title: string;
  itag: number;
  quality_label: string | null;
  output_path: string;
  status: QueueStatus;
  error_message: string | null;
  created_at: number;
}

export interface AddToQueueParams {
  videoId: string;
  title: string;
  itag: number;
  qualityLabel: string | null;
  outputPath: string;
  [key: string]: unknown;
}

export const addToQueue = (params: AddToQueueParams) => invoke<QueueEntry>("add_to_queue", params);

export const listQueue = () => invoke<QueueEntry[]>("list_queue");

export const removeFromQueue = (queueId: number) =>
  invoke<void>("remove_from_queue", { queueId });
