import { useVideoFormats } from "@/hooks/useVideoFormats";
import { useQueue } from "@/hooks/useQueue";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
} from "@/components/ui/dialog";
import { formatBytes } from "@/lib/format";
import type { FormatSummary } from "@/lib/video";
import { formatDescription, type QueueEntry } from "@/lib/queue";

function formatLabel(format: FormatSummary): string {
  return format.quality_label ?? "audio";
}

function trackLabel(format: FormatSummary): string {
  if (format.has_video && format.has_audio) return "video+audio";
  if (format.has_video) return "video only";
  if (format.has_audio) return "audio only";
  return "unknown";
}

/**
 * Repoints a single queue entry at a different format, picked from that
 * video's own format list. The precise counterpart to the queue's bulk
 * quality update, which can only offer preferences because a multi-select
 * spans videos with different itags.
 *
 * Also offers MP3 for any audio-only format, since "convert after
 * download" is a property of the queue row rather than of the stream.
 */
export function ChangeFormatDialog({
  entry,
  onClose,
}: {
  entry: QueueEntry | null;
  onClose: () => void;
}) {
  const { data, isLoading, error } = useVideoFormats(entry?.video_id ?? null);
  const { setFormat } = useQueue();

  function apply(itag: number, qualityLabel: string | null, convertToMp3: boolean) {
    if (!entry) return;
    setFormat.mutate(
      { queueId: entry.id, itag, qualityLabel, convertToMp3 },
      { onSuccess: onClose }
    );
  }

  return (
    <Dialog open={entry !== null} onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="flex max-h-[80vh] flex-col gap-3 sm:max-w-2xl">
        <DialogHeader>
          <DialogTitle>Change format</DialogTitle>
          <DialogDescription>
            {entry
              ? `"${entry.title}" — currently ${formatDescription(
                  entry,
                )}. Picking a format re-queues the entry.`
              : ""}
          </DialogDescription>
        </DialogHeader>

        {isLoading && <p className="text-sm text-muted-foreground">Loading formats...</p>}

        {error && (
          <p className="text-sm text-destructive">
            {error instanceof Error ? error.message : String(error)}
          </p>
        )}

        {setFormat.error && (
          <p className="text-sm text-destructive">
            {setFormat.error instanceof Error
              ? setFormat.error.message
              : String(setFormat.error)}
          </p>
        )}

        {data && (
          <ul className="flex flex-col gap-1 overflow-y-auto">
            {data.formats.map((format) => {
              const isCurrent = format.itag === entry?.itag && !entry?.convert_to_mp3;
              const audioOnly = format.has_audio && !format.has_video;

              return (
                <li
                  key={format.itag}
                  className="flex items-center justify-between gap-2 rounded-lg border px-2 py-1.5 text-xs transition-colors hover:bg-muted/50"
                >
                  <span className="w-14 shrink-0 font-medium">{formatLabel(format)}</span>
                  <span className="w-24 shrink-0 text-muted-foreground">{trackLabel(format)}</span>
                  <span className="w-24 shrink-0 text-muted-foreground">
                    {format.width && format.height ? `${format.width}x${format.height}` : "--"}
                    {format.fps ? ` @${format.fps}fps` : ""}
                  </span>
                  <span className="w-16 shrink-0 text-muted-foreground">
                    {formatBytes(format.content_length_bytes)}
                  </span>
                  <span className="w-16 shrink-0 text-muted-foreground">itag {format.itag}</span>
                  <div className="ml-auto flex shrink-0 gap-1.5">
                    {audioOnly && (
                      <Button
                        size="xs"
                        variant="outline"
                        disabled={setFormat.isPending}
                        onClick={() => apply(format.itag, format.quality_label, true)}
                      >
                        Use as MP3
                      </Button>
                    )}
                    <Button
                      size="xs"
                      disabled={isCurrent || setFormat.isPending}
                      onClick={() => apply(format.itag, format.quality_label, false)}
                    >
                      {isCurrent ? "Current" : "Use this"}
                    </Button>
                  </div>
                </li>
              );
            })}
          </ul>
        )}
      </DialogContent>
    </Dialog>
  );
}
