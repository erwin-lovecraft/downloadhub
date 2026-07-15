import { invoke } from "@tauri-apps/api/core";

export interface FormatSummary {
  itag: number;
  mime_type: string;
  quality: string | null;
  quality_label: string | null;
  width: number | null;
  height: number | null;
  fps: number | null;
  bitrate: number | null;
  content_length_bytes: number | null;
  has_video: boolean;
  has_audio: boolean;
}

export interface VideoDetail {
  video_id: string;
  title: string;
  author: string;
  duration_seconds: number;
  formats: FormatSummary[];
}

export const getVideoFormats = (videoId: string) =>
  invoke<VideoDetail>("get_video_formats", { videoId });
