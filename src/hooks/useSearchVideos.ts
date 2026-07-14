import { useMutation } from "@tanstack/react-query";
import { searchVideos } from "@/lib/youtube";

export function useSearchVideos() {
  return useMutation({
    mutationFn: searchVideos,
  });
}
