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
  const { list, start, cancel, remove } = useQueue();
  const progressByQueueId = useDownloadProgressStore((s) => s.progress);

  return (
    <div className="flex w-full max-w-2xl flex-col gap-3">
      <span className="text-sm font-medium">Download queue</span>

      {list.isLoading && <p className="text-sm text-muted-foreground">Loading queue...</p>}

      {list.error && (
        <p className="text-sm text-destructive">
          {list.error instanceof Error ? list.error.message : String(list.error)}
        </p>
      )}

      {list.data && list.data.length === 0 && (
        <p className="text-sm text-muted-foreground">Queue is empty.</p>
      )}

      <ul className="flex flex-col gap-2">
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
              <div className="flex items-center justify-between gap-2">
                <span className="truncate text-sm font-medium">{entry.title}</span>
                <div className="flex shrink-0 items-center gap-2">
                  <span className={statusClassName(entry.status)}>{entry.status}</span>
                  {isDownloading ? (
                    <Button
                      size="xs"
                      variant="outline"
                      disabled={cancel.isPending}
                      onClick={() => cancel.mutate(entry.id)}
                    >
                      Cancel
                    </Button>
                  ) : (
                    <Button
                      size="xs"
                      variant="outline"
                      disabled={!canStart || start.isPending}
                      onClick={() => start.mutate(entry.id)}
                    >
                      {startLabel}
                    </Button>
                  )}
                  <Button
                    size="xs"
                    variant="outline"
                    disabled={isDownloading || remove.isPending}
                    onClick={() => remove.mutate(entry.id)}
                  >
                    Remove
                  </Button>
                </div>
              </div>
              <span className="text-muted-foreground">
                {entry.quality_label ?? "audio"} (itag {entry.itag})
              </span>
              <span className="truncate text-muted-foreground">{entry.output_path}</span>
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
