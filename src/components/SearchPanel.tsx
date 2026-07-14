import { useState } from "react";
import { useSearchVideos } from "@/hooks/useSearchVideos";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { formatDuration } from "@/lib/format";

export function SearchPanel() {
  const [query, setQuery] = useState("");
  const search = useSearchVideos();

  function onSubmit(e: React.FormEvent) {
    e.preventDefault();
    const trimmed = query.trim();
    if (trimmed) search.mutate(trimmed);
  }

  return (
    <div className="flex w-full max-w-2xl flex-col gap-4">
      <form onSubmit={onSubmit} className="flex gap-2">
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
        <p className="text-sm text-destructive">
          {search.error instanceof Error ? search.error.message : String(search.error)}
        </p>
      )}

      {search.data && search.data.length === 0 && (
        <p className="text-sm text-muted-foreground">No results.</p>
      )}

      <ul className="flex flex-col gap-3">
        {search.data?.map((video) => (
          <li key={video.video_id} className="flex gap-3 rounded-md border p-2">
            {video.thumbnail_url && (
              <img
                src={video.thumbnail_url}
                alt={video.title}
                className="h-20 w-32 shrink-0 rounded object-cover"
              />
            )}
            <div className="flex min-w-0 flex-col justify-center gap-1">
              <span className="truncate text-sm font-medium">{video.title}</span>
              <span className="text-xs text-muted-foreground">{video.channel_title}</span>
              <span className="text-xs text-muted-foreground">
                {formatDuration(video.duration_seconds)}
              </span>
            </div>
          </li>
        ))}
      </ul>
    </div>
  );
}
