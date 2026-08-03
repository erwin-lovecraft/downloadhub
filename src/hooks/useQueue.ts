import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  addToQueue,
  clearQueue,
  listQueue,
  removeFromQueue,
  setQueueEntriesQuality,
  setQueueEntryFormat,
} from "@/lib/queue";
import { cancelDownload, downloadAll, startDownload, stopDownloadAll } from "@/lib/download";
import { useDownloadProgressStore } from "@/lib/downloadProgress";
import { useBatchDownloadStore } from "@/lib/batchDownload";

export const queueQueryKey = ["queue", "list"] as const;

/**
 * How often to re-read the queue. Polling, not events: an external AI
 * agent adds entries through the mcp-server *process*, which the app gets
 * no in-process signal from — the shared SQLite database is the only
 * channel, so agent-added entries would otherwise not appear until the
 * user happened to trigger a refetch.
 */
const POLL_INTERVAL_MS = 3000;

export function useQueue() {
  const queryClient = useQueryClient();
  const clearProgress = useDownloadProgressStore((s) => s.clearProgress);
  const clearAllProgress = useDownloadProgressStore((s) => s.clearAllProgress);
  const batchJobId = useBatchDownloadStore((s) => s.runningJobId);
  const batchOutcome = useBatchDownloadStore((s) => s.lastOutcome);
  const setBatchRunning = useBatchDownloadStore((s) => s.setRunning);

  const list = useQuery({
    queryKey: queueQueryKey,
    queryFn: listQueue,
    refetchInterval: POLL_INTERVAL_MS,
  });

  const invalidate = () => queryClient.invalidateQueries({ queryKey: queueQueryKey });

  const add = useMutation({ mutationFn: addToQueue, onSuccess: invalidate });
  const start = useMutation({ mutationFn: startDownload, onSuccess: invalidate });
  const cancel = useMutation({
    mutationFn: cancelDownload,
    onSuccess: (_data, queueId) => {
      // The cancel command doesn't emit a download-progress event, so the
      // last "downloading" event for this id would otherwise linger and
      // keep the UI showing a stale progress bar / Cancel button forever.
      clearProgress(queueId);
      invalidate();
    },
  });
  const remove = useMutation({ mutationFn: removeFromQueue, onSuccess: invalidate });
  const clear = useMutation({
    mutationFn: clearQueue,
    onSuccess: () => {
      // Every entry (and any progress event still lingering for it) is gone.
      clearAllProgress();
      invalidate();
    },
  });
  const downloadAllMutation = useMutation({
    mutationFn: downloadAll,
    // The command only starts the batch worker and hands back its job id;
    // `useBatchDownloadListener` learns the outcome later from the
    // `download-batch-done` event carrying that same id.
    onSuccess: (jobId) => setBatchRunning(jobId),
  });
  const stopDownloadAllMutation = useMutation({ mutationFn: stopDownloadAll });
  const setFormat = useMutation({ mutationFn: setQueueEntryFormat, onSuccess: invalidate });
  const setQuality = useMutation({ mutationFn: setQueueEntriesQuality, onSuccess: invalidate });

  return {
    list,
    add,
    start,
    cancel,
    remove,
    clear,
    downloadAll: downloadAllMutation,
    stopDownloadAll: stopDownloadAllMutation,
    batchJobId,
    batchOutcome,
    setFormat,
    setQuality,
  };
}
