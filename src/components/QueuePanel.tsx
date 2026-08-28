import { useState } from "react";
import { DownloadIcon, SquareStopIcon, Trash2Icon } from "lucide-react";
import { useQueue } from "@/hooks/useQueue";
import { useDownloadProgressStore } from "@/lib/downloadProgress";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { ChangeFormatDialog } from "@/components/ChangeFormatDialog";
import { formatBytes } from "@/lib/format";
import { openFolder } from "@/lib/opener";
import { FORMAT_PREFERENCE_LABELS, type FormatPreference } from "@/lib/enqueue";
import { formatDescription, type QueueEntry, type QueueStatus } from "@/lib/queue";

function statusClassName(status: QueueStatus): string {
  switch (status) {
    case "completed":
      return "text-success";
    case "failed":
      return "text-destructive";
    case "cancelled":
      return "text-muted-foreground";
    default:
      return "text-foreground";
  }
}

export function QueuePanel() {
  const {
    list,
    start,
    cancel,
    remove,
    clear,
    downloadAll,
    stopDownloadAll,
    batchJobId,
    batchOutcome,
    setQuality,
  } = useQueue();
  const progressByQueueId = useDownloadProgressStore((s) => s.progress);
  const [openFolderError, setOpenFolderError] = useState<string | null>(null);
  const [selected, setSelected] = useState<Set<number>>(new Set());
  const [bulkQuality, setBulkQuality] = useState<FormatPreference>("mp3");
  const [formatEntry, setFormatEntry] = useState<QueueEntry | null>(null);

  const entries = list.data ?? [];
  const queued = entries.filter((entry) => entry.status === "queued");
  const hasQueued = queued.length > 0;
  // True from the moment `download_all` is clicked (its own invoke is
  // in flight) through to the `download-batch-done` event landing for the
  // job it started — the worker task itself may still be running an entry
  // to completion well after the `stop_download_all` call that cancelled
  // its token has already resolved.
  const batchRunning = downloadAll.isPending || batchJobId !== null;
  const stopRequested = stopDownloadAll.isSuccess && stopDownloadAll.variables === batchJobId;

  // An entry can only be re-formatted while it isn't mid-transfer; the
  // backend enforces this too, but greying the checkbox explains why.
  const selectable = entries.filter((entry) => entry.status !== "downloading");
  // Entries can vanish (removed here, or by an agent over MCP) while still
  // selected, so always intersect with what's actually on screen.
  const selectedIds = selectable.filter((entry) => selected.has(entry.id)).map((e) => e.id);
  const allQueuedSelected = hasQueued && queued.every((entry) => selected.has(entry.id));

  function toggle(id: number, checked: boolean) {
    setSelected((prev) => {
      const next = new Set(prev);
      if (checked) next.add(id);
      else next.delete(id);
      return next;
    });
  }

  function toggleAllQueued(checked: boolean) {
    setSelected((prev) => {
      const next = new Set(prev);
      for (const entry of queued) {
        if (checked) next.add(entry.id);
        else next.delete(entry.id);
      }
      return next;
    });
  }

  return (
    <div className="flex h-full flex-col gap-3">
      <span className="text-sm font-semibold">Download queue</span>
      <div className="flex shrink-0 items-center justify-between gap-2">
        <div className="flex items-center gap-2">
          <Button
            size="xs"
            variant="outline"
            disabled={entries.length === 0 || batchRunning || clear.isPending}
            onClick={() => {
              if (window.confirm("Remove all entries from the download queue?")) {
                clear.mutate();
              }
            }}
          >
            <Trash2Icon />
            {clear.isPending ? "Clearing..." : "Clear queue"}
          </Button>
          {batchRunning ? (
            <Button
              size="xs"
              variant="outline"
              disabled={batchJobId === null || stopRequested || stopDownloadAll.isPending}
              onClick={() => {
                if (batchJobId !== null) stopDownloadAll.mutate(batchJobId);
              }}
            >
              <SquareStopIcon />
              {stopRequested ? "Stopping..." : "Stop"}
            </Button>
          ) : (
            <Button
              size="xs"
              variant="outline"
              disabled={!hasQueued}
              onClick={() => downloadAll.mutate()}
            >
              <DownloadIcon />
              Download all
            </Button>
          )}
        </div>
      </div>

      {selectable.length > 0 && (
        <div className="flex shrink-0 flex-col gap-2 rounded-xl border bg-card p-2 text-xs">
          <label className="flex cursor-pointer items-center gap-2">
            <Checkbox
              checked={allQueuedSelected}
              disabled={!hasQueued || batchRunning}
              onCheckedChange={(checked) => toggleAllQueued(checked === true)}
            />
            <span>Select all queued ({queued.length})</span>
            {selectedIds.length > 0 && (
              <button
                type="button"
                className="ml-auto text-muted-foreground underline-offset-2 hover:underline"
                onClick={() => setSelected(new Set())}
              >
                Clear
              </button>
            )}
          </label>

          <div className="flex items-center gap-2">
            <select
              className="h-7 min-w-0 flex-1 rounded-md border bg-background px-2 text-xs"
              value={bulkQuality}
              disabled={batchRunning}
              onChange={(e) => setBulkQuality(e.target.value as FormatPreference)}
            >
              {(Object.keys(FORMAT_PREFERENCE_LABELS) as FormatPreference[]).map((preference) => (
                <option key={preference} value={preference}>
                  {FORMAT_PREFERENCE_LABELS[preference]}
                </option>
              ))}
            </select>
            <Button
              size="xs"
              variant="outline"
              className="shrink-0"
              disabled={selectedIds.length === 0 || setQuality.isPending || batchRunning}
              onClick={() =>
                setQuality.mutate(
                  { queueIds: selectedIds, preference: bulkQuality },
                  { onSuccess: () => setSelected(new Set()) }
                )
              }
            >
              {setQuality.isPending ? "Updating..." : `Update ${selectedIds.length || ""}`.trim()}
            </Button>
          </div>

          {setQuality.error && (
            <p className="text-destructive">
              {setQuality.error instanceof Error
                ? setQuality.error.message
                : String(setQuality.error)}
            </p>
          )}

          {setQuality.data && setQuality.data.skipped.length > 0 && (
            <ul className="max-h-20 overflow-y-auto text-muted-foreground">
              {setQuality.data.skipped.map((skip) => (
                <li key={skip.video_id}>
                  {skip.video_id}: {skip.reason}
                </li>
              ))}
            </ul>
          )}
        </div>
      )}

      {openFolderError && (
        <p className="shrink-0 text-sm text-destructive">{openFolderError}</p>
      )}

      {downloadAll.error && (
        <p className="shrink-0 text-sm text-destructive">
          {downloadAll.error instanceof Error ? downloadAll.error.message : String(downloadAll.error)}
        </p>
      )}

      {stopDownloadAll.error && (
        <p className="shrink-0 text-sm text-destructive">
          {stopDownloadAll.error instanceof Error
            ? stopDownloadAll.error.message
            : String(stopDownloadAll.error)}
        </p>
      )}

      {clear.error && (
        <p className="shrink-0 text-sm text-destructive">
          {clear.error instanceof Error ? clear.error.message : String(clear.error)}
        </p>
      )}

      {batchOutcome && !batchRunning && (
        <p className="shrink-0 text-sm text-muted-foreground">
          {batchOutcome.error_message ? (
            `Batch failed: ${batchOutcome.error_message}`
          ) : (
            <>
              {batchOutcome.stopped ? "Batch stopped: " : "Batch finished: "}
              {batchOutcome.completed} completed
              {batchOutcome.failed > 0 ? `, ${batchOutcome.failed} failed` : ""}
              {batchOutcome.stopped ? ", remaining queued entries left untouched" : ""}.
            </>
          )}
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
        <p className="shrink-0 text-sm text-muted-foreground">Add some video or audio to queue first.</p>
      )}

      <ul className="flex min-h-0 flex-1 flex-col gap-2 overflow-y-auto">
        {entries.map((entry) => {
          const progress = progressByQueueId[entry.id];
          const isTranscoding = progress?.status === "transcoding";
          const isDownloading =
            entry.status === "downloading" ||
            progress?.status === "downloading" ||
            isTranscoding;
          const percent =
            progress && progress.status === "downloading" && progress.total_bytes > 0
              ? Math.min(100, Math.round((progress.bytes_written / progress.total_bytes) * 100))
              : null;
          const canStart = entry.status === "queued" || entry.status === "failed" || entry.status === "cancelled";
          const startLabel = entry.status === "failed" || entry.status === "cancelled" ? "Retry" : "Start";

          return (
            <li
              key={entry.id}
              className="flex gap-2 rounded-xl border bg-card p-3 text-xs shadow-xs"
            >
              <Checkbox
                className="mt-0.5 shrink-0"
                checked={selected.has(entry.id)}
                disabled={isDownloading || batchRunning}
                onCheckedChange={(checked) => toggle(entry.id, checked === true)}
              />
              <div className="flex min-w-0 flex-1 flex-col gap-1">
                <span className="truncate text-sm font-medium" title={entry.title}>
                  {entry.title}
                </span>
                <div className="flex items-center justify-between gap-2">
                  <span className={`font-medium capitalize ${statusClassName(entry.status)}`}>
                    {entry.status}
                  </span>
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
                    ) : entry.status === "completed" ? (
                      <Button
                        size="xs"
                        variant="outline"
                        onClick={() =>
                          openFolder(entry.output_path)
                            .then(() => setOpenFolderError(null))
                            .catch((err) => setOpenFolderError(String(err)))
                        }
                      >
                        Open in files
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
                <button
                  type="button"
                  className="w-fit text-left text-muted-foreground underline-offset-2 hover:text-foreground hover:underline disabled:no-underline disabled:hover:text-muted-foreground"
                  disabled={isDownloading || batchRunning}
                  title="Change this entry's format"
                  onClick={() => setFormatEntry(entry)}
                >
                  {formatDescription(entry)}
                </button>
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
                      {isTranscoding
                        ? "converting to mp3..."
                        : progress
                          ? `${formatBytes(progress.bytes_written)}${
                              progress.total_bytes ? ` / ${formatBytes(progress.total_bytes)}` : ""
                            }`
                          : "starting..."}
                    </span>
                  </div>
                )}
                {entry.error_message && <span className="text-destructive">{entry.error_message}</span>}
              </div>
            </li>
          );
        })}
      </ul>

      <ChangeFormatDialog entry={formatEntry} onClose={() => setFormatEntry(null)} />
    </div>
  );
}
