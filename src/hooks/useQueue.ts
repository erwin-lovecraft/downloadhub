import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { addToQueue, listQueue, removeFromQueue } from "@/lib/queue";
import { cancelDownload, startDownload } from "@/lib/download";
import { useDownloadProgressStore } from "@/lib/downloadProgress";

const queueQueryKey = ["queue", "list"] as const;

export function useQueue() {
  const queryClient = useQueryClient();
  const clearProgress = useDownloadProgressStore((s) => s.clearProgress);

  const list = useQuery({
    queryKey: queueQueryKey,
    queryFn: listQueue,
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

  return { list, add, start, cancel, remove };
}
