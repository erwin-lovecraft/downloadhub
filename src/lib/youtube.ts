import { invoke } from "@tauri-apps/api/core";

export interface VideoSummary {
  video_id: string;
  title: string;
  channel_title: string;
  thumbnail_url: string | null;
  published_at: string;
  duration_seconds: number | null;
}

export const searchVideos = (query: string) =>
  invoke<VideoSummary[]>("search_videos", { query });
