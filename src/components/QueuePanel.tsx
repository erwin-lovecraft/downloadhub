import { useQueue } from "@/hooks/useQueue";
import { useDownloadProgressStore } from "@/lib/downloadProgress";
import { Button } from "@/components/ui/button";
import { formatBytes } from "@/lib/format";
import type { QueueStatus } from "@/lib/queue";

function statusClassName(status: QueueStatus): string {
  switch (status) {
    case "completed":
      return "text-green-600 dark:text-green-400";
    case "failed":
      return "text-destructive";
    case "cancelled":
      return "text-muted-foreground";
    default:
      return "text-foreground";
  }
}

export function QueuePanel() {
  const { list, start, cancel, remove, downloadAll } = useQueue();
  const progressByQueueId = useDownloadProgressStore((s) => s.progress);

  const hasQueued = list.data?.some((entry) => entry.status === "queued") ?? false;
  const batchRunning = downloadAll.isPending;

  return (
    <div className="flex h-full flex-col gap-3">
      <div className="flex shrink-0 items-center justify-between gap-2">
        <span className="text-sm font-medium">Download queue</span>
        <Button
          size="xs"
          variant="outline"
          disabled={!hasQueued || batchRunning}
          onClick={() => downloadAll.mutate()}
        >
          {batchRunning ? "Downloading..." : "Download all"}
        </Button>
      </div>

      {downloadAll.error && (
        <p className="shrink-0 text-sm text-destructive">
          {downloadAll.error instanceof Error ? downloadAll.error.message : String(downloadAll.error)}
        </p>
      )}

      {downloadAll.data && !batchRunning && (
        <p className="shrink-0 text-sm text-muted-foreground">
          Batch finished: {downloadAll.data.completed} completed
          {downloadAll.data.failed > 0 ? `, ${downloadAll.data.failed} failed` : ""}.
        </p>
      )}

      {list.isLoading && (
        <p className="shrink-0 text-sm text-muted-foreground">Loading queue...</p>
      )}

      {list.error && (
        <p className="shrink-0 text-sm text-destructive">
          {list.error instanceof Error ? list.error.message : String(list.error)}
        </p>
      )}

      {list.data && list.data.length === 0 && (
        <p className="shrink-0 text-sm text-muted-foreground">Queue is empty.</p>
      )}

      <ul className="flex min-h-0 flex-1 flex-col gap-2 overflow-y-auto">
        {list.data?.map((entry) => {
          const progress = progressByQueueId[entry.id];
          const isDownloading = entry.status === "downloading" || progress?.status === "downloading";
          const percent =
            progress && progress.status === "downloading" && progress.total_bytes > 0
              ? Math.min(100, Math.round((progress.bytes_written / progress.total_bytes) * 100))
              : null;
          const canStart = entry.status === "queued" || entry.status === "failed" || entry.status === "cancelled";
          const startLabel = entry.status === "failed" || entry.status === "cancelled" ? "Retry" : "Start";

          return (
            <li key={entry.id} className="flex flex-col gap-1 rounded-md border p-2 text-xs">
              <span className="truncate text-sm font-medium" title={entry.title}>
                {entry.title}
              </span>
              <div className="flex items-center justify-between gap-2">
                <span className={statusClassName(entry.status)}>{entry.status}</span>
                <div className="flex shrink-0 items-center gap-2">
                  {isDownloading ? (
                    <Button
                      size="xs"
                      variant="outline"
                      disabled={cancel.isPending || batchRunning}
                      onClick={() => cancel.mutate(entry.id)}
                    >
                      Cancel
                    </Button>
                  ) : (
                    <Button
                      size="xs"
                      variant="outline"
                      disabled={!canStart || start.isPending || batchRunning}
                      onClick={() => start.mutate(entry.id)}
                    >
                      {startLabel}
                    </Button>
                  )}
                  <Button
                    size="xs"
                    variant="outline"
                    disabled={isDownloading || remove.isPending || batchRunning}
                    onClick={() => remove.mutate(entry.id)}
                  >
                    Remove
                  </Button>
                </div>
              </div>
              <span className="text-muted-foreground">
                {entry.quality_label ?? "audio"} (itag {entry.itag})
              </span>
              <span className="truncate text-muted-foreground" title={entry.output_path}>
                {entry.output_path}
              </span>
              {isDownloading && (
                <div className="flex items-center gap-2">
                  {percent !== null && (
                    <div className="h-1.5 flex-1 overflow-hidden rounded-full bg-muted">
                      <div className="h-full bg-primary transition-all" style={{ width: `${percent}%` }} />
                    </div>
                  )}
                  <span className="text-muted-foreground">
                    {progress
                      ? `${formatBytes(progress.bytes_written)}${
                          progress.total_bytes ? ` / ${formatBytes(progress.total_bytes)}` : ""
                        }`
                      : "starting..."}
                  </span>
                </div>
              )}
              {entry.error_message && <span className="text-destructive">{entry.error_message}</span>}
            </li>
          );
        })}
      </ul>
    </div>
  );
}
