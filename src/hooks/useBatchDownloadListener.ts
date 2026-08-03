import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { useQueryClient } from "@tanstack/react-query";
import { useBatchDownloadStore, type BatchDownloadDoneEvent } from "@/lib/batchDownload";

/** Subscribes to `download-batch-done` events for the lifetime of the app. */
export function useBatchDownloadListener() {
  const setDone = useBatchDownloadStore((s) => s.setDone);
  const queryClient = useQueryClient();

  useEffect(() => {
    const unlisten = listen<BatchDownloadDoneEvent>("download-batch-done", (event) => {
      setDone(event.payload);
      queryClient.invalidateQueries({ queryKey: ["queue", "list"] });
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [setDone, queryClient]);
}
