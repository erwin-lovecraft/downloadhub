import { useEffect, useState } from "react";
import { usePlaylistImport } from "@/hooks/usePlaylistImport";
import { useSettings } from "@/hooks/useSettings";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
} from "@/components/ui/dialog";
import { formatDuration } from "@/lib/format";
import { pickOutputFolder } from "@/lib/dialog";
import type { FormatPreference } from "@/lib/playlist";

export function PlaylistImportDialog({
  open,
  onOpenChange,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const [playlistInput, setPlaylistInput] = useState("");
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [preference, setPreference] = useState<FormatPreference>("best_progressive");
  const [outputPath, setOutputPath] = useState("");
  const { load, importVideos } = usePlaylistImport();
  const { settings } = useSettings();

  const videos = load.data ?? [];

  // Re-seed from settings defaults each time the dialog opens, rather than
  // on every render, so it doesn't clobber what the user's already picked
  // while the dialog stays open.
  useEffect(() => {
    if (open) {
      setOutputPath(settings.data?.default_output_path ?? "");
      setPreference(settings.data?.default_quality ?? "best_progressive");
    }
  }, [open, settings.data]);

  function handleLoad(e: React.FormEvent) {
    e.preventDefault();
    const trimmed = playlistInput.trim();
    if (!trimmed) return;
    importVideos.reset();
    load.mutate(trimmed, {
      onSuccess: (loaded) => setSelected(new Set(loaded.map((v) => v.video_id))),
    });
  }

  function toggle(videoId: string, checked: boolean) {
    setSelected((prev) => {
      const next = new Set(prev);
      if (checked) next.add(videoId);
      else next.delete(videoId);
      return next;
    });
  }

  function handleOpenChange(next: boolean) {
    if (!next) {
      setPlaylistInput("");
      setSelected(new Set());
      setOutputPath("");
      load.reset();
      importVideos.reset();
    }
    onOpenChange(next);
  }

  return (
    <Dialog open={open} onOpenChange={handleOpenChange}>
      <DialogContent className="flex max-h-[80vh] flex-col gap-3 sm:max-w-2xl">
        <DialogHeader>
          <DialogTitle>Import playlist</DialogTitle>
          <DialogDescription>
            Paste a playlist URL or ID, pick which videos to include, a quality, and a
            destination folder.
          </DialogDescription>
        </DialogHeader>

        <form onSubmit={handleLoad} className="flex shrink-0 gap-2">
          <Input
            value={playlistInput}
            onChange={(e) => setPlaylistInput(e.target.value)}
            placeholder="Playlist URL or ID"
          />
          <Button type="submit" disabled={load.isPending || !playlistInput.trim()}>
            {load.isPending ? "Loading..." : "Load"}
          </Button>
        </form>

        {load.error && (
          <p className="shrink-0 text-sm text-destructive">
            {load.error instanceof Error ? load.error.message : String(load.error)}
          </p>
        )}

        {videos.length > 0 && (
          <>
            <div className="flex shrink-0 items-center justify-between gap-2 text-sm">
              <span className="text-muted-foreground">
                {selected.size} of {videos.length} selected
              </span>
              <div className="flex gap-2">
                <Button
                  type="button"
                  variant="ghost"
                  size="xs"
                  onClick={() => setSelected(new Set(videos.map((v) => v.video_id)))}
                >
                  Select all
                </Button>
                <Button type="button" variant="ghost" size="xs" onClick={() => setSelected(new Set())}>
                  Select none
                </Button>
              </div>
            </div>

            <ul className="flex min-h-0 flex-1 flex-col gap-1 overflow-y-auto">
              {videos.map((video) => (
                <li
                  key={video.video_id}
                  className="flex items-center gap-2 rounded-lg border px-2 py-1.5 text-xs transition-colors hover:bg-muted/50"
                >
                  <Checkbox
                    checked={selected.has(video.video_id)}
                    onCheckedChange={(checked) => toggle(video.video_id, checked === true)}
                  />
                  <span className="min-w-0 flex-1 truncate" title={video.title}>
                    {video.title}
                  </span>
                  <span className="shrink-0 text-muted-foreground">
                    {formatDuration(video.duration_seconds)}
                  </span>
                </li>
              ))}
            </ul>

            <div className="flex shrink-0 items-center gap-2">
              <Button
                type="button"
                size="sm"
                variant={preference === "best_progressive" ? "default" : "outline"}
                onClick={() => setPreference("best_progressive")}
              >
                Best quality (video + audio)
              </Button>
              <Button
                type="button"
                size="sm"
                variant={preference === "best_audio_only" ? "default" : "outline"}
                onClick={() => setPreference("best_audio_only")}
              >
                Audio only
              </Button>
            </div>

            <div className="flex shrink-0 gap-2">
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

            <Button
              type="button"
              className="shrink-0"
              disabled={selected.size === 0 || !outputPath.trim() || importVideos.isPending}
              onClick={() =>
                importVideos.mutate({
                  videoIds: Array.from(selected),
                  preference,
                  outputPath: outputPath.trim(),
                })
              }
            >
              {importVideos.isPending ? "Adding..." : `Add ${selected.size} to queue`}
            </Button>

            {importVideos.error && (
              <p className="shrink-0 text-sm text-destructive">
                {importVideos.error instanceof Error
                  ? importVideos.error.message
                  : String(importVideos.error)}
              </p>
            )}

            {importVideos.data && (
              <div className="shrink-0 text-sm">
                <p>
                  {importVideos.data.added.length} added to queue
                  {importVideos.data.skipped.length > 0
                    ? `, ${importVideos.data.skipped.length} skipped`
                    : ""}
                  .
                </p>
                {importVideos.data.skipped.length > 0 && (
                  <ul className="mt-1 max-h-24 overflow-y-auto text-xs text-muted-foreground">
                    {importVideos.data.skipped.map((s) => (
                      <li key={s.video_id}>
                        {s.video_id}: {s.reason}
                      </li>
                    ))}
                  </ul>
                )}
              </div>
            )}
          </>
        )}
      </DialogContent>
    </Dialog>
  );
}
