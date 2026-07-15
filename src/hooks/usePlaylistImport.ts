import { useMutation, useQueryClient } from "@tanstack/react-query";
import { importPlaylistToQueue, listPlaylistItems } from "@/lib/playlist";
import { queueQueryKey } from "@/hooks/useQueue";

export function usePlaylistImport() {
  const queryClient = useQueryClient();

  const load = useMutation({ mutationFn: listPlaylistItems });
  const importVideos = useMutation({
    mutationFn: importPlaylistToQueue,
    onSuccess: () => queryClient.invalidateQueries({ queryKey: queueQueryKey }),
  });

  return { load, importVideos };
}
