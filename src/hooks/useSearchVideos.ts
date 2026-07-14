import { useQuery } from "@tanstack/react-query";
import { ALL_VIDEOS } from "@/data/mock";
import type { Video } from "@/types";

/**
 * Stands in for the real `search_videos` Tauri command (Phase 1 step 3,
 * YouTube Data API v3 `search.list`). Only this function's body should
 * need to change when that lands — callers just get a `Video[]`.
 */
async function searchVideos(query: string): Promise<Video[]> {
  const trimmed = query.trim().toLowerCase();
  if (!trimmed) return ALL_VIDEOS;
  return ALL_VIDEOS.filter((v) => v.title.toLowerCase().includes(trimmed));
}

export function useSearchVideos(query: string) {
  return useQuery({
    queryKey: ["search-videos", query],
    queryFn: () => searchVideos(query),
    placeholderData: (previous) => previous,
  });
}
