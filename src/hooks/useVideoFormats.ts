import { useQuery } from "@tanstack/react-query";
import { getVideoFormats } from "@/lib/video";

export function useVideoFormats(videoId: string | null) {
  return useQuery({
    queryKey: ["video", "formats", videoId],
    queryFn: () => getVideoFormats(videoId as string),
    enabled: videoId !== null,
  });
}
