import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { useQueryClient } from "@tanstack/react-query";
import { useDownloadProgressStore, type DownloadProgressEvent } from "@/lib/downloadProgress";

/** Subscribes to `download-progress` events for the lifetime of the app. */
export function useDownloadProgressListener() {
  const setProgress = useDownloadProgressStore((s) => s.setProgress);
  const queryClient = useQueryClient();

  useEffect(() => {
    const unlisten = listen<DownloadProgressEvent>("download-progress", (event) => {
      setProgress(event.payload);
      if (event.payload.status === "completed" || event.payload.status === "failed") {
        queryClient.invalidateQueries({ queryKey: ["queue", "list"] });
      }
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [setProgress, queryClient]);
}
