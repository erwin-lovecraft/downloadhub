import { useState } from "react";
import { downloadDir } from "@tauri-apps/api/path";
import { Play, X } from "lucide-react";
import { useSearchVideos } from "@/hooks/useSearchVideos";
import { useQueue } from "@/hooks/useQueue";
import { useSettings } from "@/hooks/useSettings";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { VideoDetailPanel } from "@/components/VideoDetailPanel";
import { PlaylistImportDialog } from "@/components/PlaylistImportDialog";
import { formatDuration } from "@/lib/format";
import type { VideoSummary } from "@/lib/youtube";

// itag 139 is the low-bitrate m4a audio-only stream, present on virtually
// every video, so "Download audio" can skip the format lookup entirely.
const AUDIO_ITAG = 139;

export function SearchPanel() {
  const [query, setQuery] = useState("");
  const [selectedVideoId, setSelectedVideoId] = useState<string | null>(null);
  const [previewVideoId, setPreviewVideoId] = useState<string | null>(null);
  const [playlistDialogOpen, setPlaylistDialogOpen] = useState(false);
  const search = useSearchVideos();
  const { add } = useQueue();
  const { settings } = useSettings();

  function onSubmit(e: React.FormEvent) {
    e.preventDefault();
    const trimmed = query.trim();
    if (trimmed) search.mutate(trimmed);
  }

  async function downloadAudio(video: VideoSummary) {
    const outputPath = settings.data?.default_output_path?.trim() || (await downloadDir());
    add.mutate({
      videoId: video.video_id,
      title: video.title,
      itag: AUDIO_ITAG,
      qualityLabel: null,
      outputPath,
    });
  }

  return (
    <div className="flex h-full flex-col gap-4">
      <form onSubmit={onSubmit} className="flex shrink-0 gap-2">
        <Input
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder="Search YouTube..."
        />
        <Button type="submit" disabled={search.isPending || !query.trim()}>
          {search.isPending ? "Searching..." : "Search"}
        </Button>
      </form>

      {search.error && (
        <p className="shrink-0 text-sm text-destructive">
          {search.error instanceof Error ? search.error.message : String(search.error)}
        </p>
      )}

      {add.error && (
        <p className="shrink-0 text-sm text-destructive">
          {add.error instanceof Error ? add.error.message : String(add.error)}
        </p>
      )}

      {search.data && search.data.length === 0 && (
        <p className="shrink-0 text-sm text-muted-foreground">No results.</p>
      )}

      <ul className="flex min-h-0 flex-1 flex-col gap-3 overflow-y-auto">
        {search.data?.map((video) => {
          const isPreviewing = previewVideoId === video.video_id;
          const isAddingAudio = add.isPending && add.variables?.videoId === video.video_id;

          return (
            <li
              key={video.video_id}
              className="flex gap-3 rounded-xl border bg-card p-3 shadow-xs transition-shadow hover:shadow-sm"
            >
              {isPreviewing ? (
                <div className="relative w-64 shrink-0">
                  <iframe
                    src={`https://www.youtube-nocookie.com/embed/${video.video_id}?autoplay=1`}
                    title={video.title}
                    className="aspect-video w-full rounded"
                    allow="autoplay; encrypted-media"
                    allowFullScreen
                  />
                  <button
                    type="button"
                    aria-label="Stop preview"
                    className="absolute -right-2 -top-2 flex size-5 items-center justify-center rounded-full border bg-background text-foreground shadow-sm hover:bg-muted"
                    onClick={() => setPreviewVideoId(null)}
                  >
                    <X className="size-3" />
                  </button>
                </div>
              ) : (
                video.thumbnail_url && (
                  <button
                    type="button"
                    aria-label="Preview"
                    className="group relative h-20 w-32 shrink-0 cursor-pointer overflow-hidden rounded"
                    onClick={() => setPreviewVideoId(video.video_id)}
                  >
                    <img
                      src={video.thumbnail_url}
                      alt={video.title}
                      className="size-full object-cover"
                    />
                    <span className="absolute inset-0 flex items-center justify-center bg-black/40 opacity-0 transition-opacity group-hover:opacity-100">
                      <Play className="size-6 fill-white text-white" />
                    </span>
                  </button>
                )
              )}
              <div className="flex min-w-0 flex-1 flex-col justify-center gap-1">
                <span className="truncate text-sm font-medium">{video.title}</span>
                <span className="text-xs text-muted-foreground">{video.channel_title}</span>
                <span className="text-xs text-muted-foreground">
                  {formatDuration(video.duration_seconds)}
                </span>
              </div>
              <div className="flex shrink-0 flex-col justify-center gap-1.5">
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() => setSelectedVideoId(video.video_id)}
                >
                  View formats
                </Button>
                <Button
                  variant="outline"
                  size="sm"
                  disabled={add.isPending}
                  onClick={() => void downloadAudio(video)}
                >
                  {isAddingAudio ? "Adding..." : "Download audio"}
                </Button>
              </div>
            </li>
          );
        })}
      </ul>

      <VideoDetailPanel videoId={selectedVideoId} onClose={() => setSelectedVideoId(null)} />
      <PlaylistImportDialog open={playlistDialogOpen} onOpenChange={setPlaylistDialogOpen} />
    </div>
  );
}
