import { create } from "zustand";

export type DownloadStatus = "downloading" | "completed" | "failed";

export interface DownloadProgressEvent {
  queue_id: number;
  bytes_written: number;
  total_bytes: number;
  status: DownloadStatus;
  error_message: string | null;
}

interface DownloadProgressState {
  progress: Record<number, DownloadProgressEvent>;
  setProgress: (event: DownloadProgressEvent) => void;
  clearProgress: (queueId: number) => void;
}

export const useDownloadProgressStore = create<DownloadProgressState>((set) => ({
  progress: {},
  setProgress: (event) =>
    set((state) => ({ progress: { ...state.progress, [event.queue_id]: event } })),
  clearProgress: (queueId) =>
    set((state) => {
      const { [queueId]: _removed, ...rest } = state.progress;
      return { progress: rest };
    }),
}));
