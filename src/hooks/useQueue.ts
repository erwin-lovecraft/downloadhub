import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { addToQueue, listQueue } from "@/lib/queue";

const queueQueryKey = ["queue", "list"] as const;

export function useQueue() {
  const queryClient = useQueryClient();

  const list = useQuery({
    queryKey: queueQueryKey,
    queryFn: listQueue,
  });

  const add = useMutation({
    mutationFn: addToQueue,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: queueQueryKey });
    },
  });

  return { list, add };
}
