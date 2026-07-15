import { invoke } from "@tauri-apps/api/core";

export const startDownload = (queueId: number) => invoke<void>("start_download", { queueId });

export const cancelDownload = (queueId: number) => invoke<void>("cancel_download", { queueId });

export interface BatchDownloadOutcome {
  completed: number;
  failed: number;
}

export const downloadAll = () => invoke<BatchDownloadOutcome>("download_all");
