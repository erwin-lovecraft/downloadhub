import { useState } from "react";
import { useVideoFormats } from "@/hooks/useVideoFormats";
import { useQueue } from "@/hooks/useQueue";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { formatBytes, formatDuration } from "@/lib/format";
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

export function VideoDetailPanel({ videoId, onClose }: { videoId: string; onClose: () => void }) {
  const { data, isLoading, error } = useVideoFormats(videoId);
  const { add } = useQueue();
  const [outputPath, setOutputPath] = useState("");

  return (
    <div className="flex w-full max-w-2xl flex-col gap-3 rounded-md border p-3">
      <div className="flex items-center justify-between gap-2">
        <span className="text-sm font-medium">
          {data ? `${data.title} — ${data.author}` : "Video details"}
        </span>
        <Button variant="ghost" size="sm" onClick={onClose}>
          Close
        </Button>
      </div>

      {isLoading && <p className="text-sm text-muted-foreground">Loading formats...</p>}

      {error && (
        <p className="text-sm text-destructive">
          {error instanceof Error ? error.message : String(error)}
        </p>
      )}

      {data && (
        <>
          <span className="text-xs text-muted-foreground">
            Duration: {formatDuration(data.duration_seconds)}
          </span>

          <Input
            value={outputPath}
            onChange={(e) => setOutputPath(e.target.value)}
            placeholder="Output file path (e.g. C:\Downloads\video.mp4)"
          />
          {add.error && (
            <p className="text-sm text-destructive">
              {add.error instanceof Error ? add.error.message : String(add.error)}
            </p>
          )}

          <ul className="flex flex-col gap-1">
            {data.formats.map((format) => (
              <li
                key={format.itag}
                className="flex items-center justify-between gap-2 rounded border px-2 py-1 text-xs"
              >
                <span className="font-medium">{formatLabel(format)}</span>
                <span className="text-muted-foreground">{trackLabel(format)}</span>
                <span className="text-muted-foreground">
                  {format.width && format.height ? `${format.width}x${format.height}` : "--"}
                  {format.fps ? ` @${format.fps}fps` : ""}
                </span>
                <span className="text-muted-foreground">{formatBytes(format.content_length_bytes)}</span>
                <span className="text-muted-foreground">itag {format.itag}</span>
                <Button
                  size="xs"
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
    </div>
  );
}
