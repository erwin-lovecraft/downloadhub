export type NavKey =
  | "search"
  | "queue"
  | "history"
  | "agent"
  | "settings"
  | "components";

export interface Video {
  id: string;
  title: string;
  channel: string;
  duration: string;
  views: string;
  uploaded: string;
  thumbGradient: string;
  subs: string;
  desc: string;
}

export type FormatKey = "p2160" | "p1080" | "p720" | "p360" | "audio";

export interface FormatOption {
  key: FormatKey;
  res: string;
  type: "Video only" | "Video + Audio" | "Audio only";
  itag: string;
  size: string;
}

export type QueueStatus =
  | "downloading"
  | "queued"
  | "paused"
  | "failed"
  | "completed";

export interface QueueItem {
  id: string;
  title: string;
  channel: string;
  thumbGradient: string;
  format: string;
  status: QueueStatus;
  progress: number;
  speed?: string;
  eta?: string;
  size?: string;
  error?: string;
}

export type AgentPermission = "auto" | "manual";

export interface Agent {
  key: string;
  name: string;
  statusLabel: string;
  permission: AgentPermission;
}

export interface AgentProposal {
  id: string;
  agentName: string;
  query: string;
  videoTitle: string;
  thumbGradient: string;
  format: string;
  time: string;
}

export interface ActivityLogEntry {
  id: string;
  text: string;
  time: string;
}

export interface HistoryItem {
  id: string;
  title: string;
  channel: string;
  thumbGradient: string;
  format: string;
  path: string;
  date: string;
  size: string;
}

export type DefaultQuality = "2160p" | "1080p" | "720p" | "360p";
