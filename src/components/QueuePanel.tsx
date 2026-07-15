import { useQueue } from "@/hooks/useQueue";
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
  const { list } = useQueue();

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
        {list.data?.map((entry) => (
          <li key={entry.id} className="flex flex-col gap-1 rounded-md border p-2 text-xs">
            <div className="flex items-center justify-between gap-2">
              <span className="truncate text-sm font-medium">{entry.title}</span>
              <span className={statusClassName(entry.status)}>{entry.status}</span>
            </div>
            <span className="text-muted-foreground">
              {entry.quality_label ?? "audio"} (itag {entry.itag})
            </span>
            <span className="truncate text-muted-foreground">{entry.output_path}</span>
            {entry.error_message && <span className="text-destructive">{entry.error_message}</span>}
          </li>
        ))}
      </ul>
    </div>
  );
}
