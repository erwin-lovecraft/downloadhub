import { invoke } from "@tauri-apps/api/core";

export const startDownload = (queueId: number) => invoke<void>("start_download", { queueId });

export const cancelDownload = (queueId: number) => invoke<void>("cancel_download", { queueId });

/**
 * `download_all` spawns the batch as a single background worker task and
 * returns this job id immediately rather than awaiting the whole batch —
 * its outcome arrives later as a `download-batch-done` event carrying the
 * same id (see `useBatchDownloadListener`).
 */
export const downloadAll = () => invoke<number>("download_all");

export const stopDownloadAll = (jobId: number) => invoke<void>("stop_download_all", { jobId });
