import { useState } from "react";
import { useVideoFormats } from "@/hooks/useVideoFormats";
import { useQueue } from "@/hooks/useQueue";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
} from "@/components/ui/dialog";
import { formatBytes, formatDuration } from "@/lib/format";
import { pickOutputFolder } from "@/lib/dialog";
import type { FormatSummary } from "@/lib/video";

function formatLabel(format: FormatSummary): string {
  if (format.quality_label) return format.quality_label;
  if (format.quality) return format.quality;
  return "audio";
}

function trackLabel(format: FormatSummary): string {
  if (format.has_video && format.has_audio) return "video+audio";
  if (format.has_video) return "video only";
  if (format.has_audio) return "audio only";
  return "unknown";
}

export function VideoDetailPanel({
  videoId,
  onClose,
}: {
  videoId: string | null;
  onClose: () => void;
}) {
  const { data, isLoading, error } = useVideoFormats(videoId);
  const { add } = useQueue();
  const [outputPath, setOutputPath] = useState("");

  return (
    <Dialog open={videoId !== null} onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="flex max-h-[80vh] flex-col gap-3 sm:max-w-2xl">
        <DialogHeader>
          <DialogTitle>{data ? data.title : "Video details"}</DialogTitle>
          <DialogDescription>
            {data
              ? `${data.author} — ${formatDuration(data.duration_seconds)}`
              : "Choose a format and destination folder to add this video to the queue."}
          </DialogDescription>
        </DialogHeader>

        {isLoading && <p className="text-sm text-muted-foreground">Loading formats...</p>}

        {error && (
          <p className="text-sm text-destructive">
            {error instanceof Error ? error.message : String(error)}
          </p>
        )}

        {data && (
          <>
            <div className="flex gap-2">
              <Input
                value={outputPath}
                onChange={(e) => setOutputPath(e.target.value)}
                placeholder="Output folder (e.g. C:\Downloads)"
              />
              <Button
                type="button"
                variant="outline"
                size="sm"
                className="shrink-0"
                onClick={async () => {
                  const folder = await pickOutputFolder();
                  if (folder) setOutputPath(folder);
                }}
              >
                Browse...
              </Button>
            </div>
            {add.error && (
              <p className="text-sm text-destructive">
                {add.error instanceof Error ? add.error.message : String(add.error)}
              </p>
            )}

            <ul className="flex flex-col gap-1 overflow-y-auto">
              {data.formats.map((format) => (
                <li
                  key={format.itag}
                  className="flex items-center justify-between gap-2 rounded border px-2 py-1 text-xs"
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
                  <Button
                    size="xs"
                    className="ml-auto shrink-0"
                    disabled={!outputPath.trim() || add.isPending}
                    onClick={() =>
                      add.mutate({
                        videoId: data.video_id,
                        title: data.title,
                        itag: format.itag,
                        qualityLabel: format.quality_label,
                        outputPath: outputPath.trim(),
                      })
                    }
                  >
                    Add to queue
                  </Button>
                </li>
              ))}
            </ul>
          </>
        )}
      </DialogContent>
    </Dialog>
  );
}
