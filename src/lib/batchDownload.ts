import { create } from "zustand";

export interface BatchDownloadDoneEvent {
  job_id: number;
  completed: number;
  failed: number;
  stopped: boolean;
  error_message: string | null;
}

interface BatchDownloadState {
  /** The batch job `download_all` most recently started, until its `download-batch-done` event arrives. */
  runningJobId: number | null;
  lastOutcome: BatchDownloadDoneEvent | null;
  setRunning: (jobId: number) => void;
  setDone: (event: BatchDownloadDoneEvent) => void;
}

export const useBatchDownloadStore = create<BatchDownloadState>((set) => ({
  runningJobId: null,
  lastOutcome: null,
  setRunning: (jobId) => set({ runningJobId: jobId, lastOutcome: null }),
  setDone: (event) =>
    set((state) =>
      // Guards against a stale event from a job that isn't the one we're
      // currently tracking (shouldn't happen — only one batch runs at a
      // time — but cheap to make impossible rather than assumed).
      state.runningJobId === event.job_id ? { runningJobId: null, lastOutcome: event } : state
    ),
}));
