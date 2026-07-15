import { invoke } from "@tauri-apps/api/core";

export const startDownload = (queueId: number) => invoke<void>("start_download", { queueId });
