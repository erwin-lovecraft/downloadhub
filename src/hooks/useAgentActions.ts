import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  approveAgentAction,
  listPendingAgentActions,
  rejectAgentAction,
} from "@/lib/agentActions";
import { queueQueryKey } from "@/hooks/useQueue";

export const agentActionsQueryKey = ["agent-actions", "unresolved"] as const;

/** How often to look for new agent requests. Polling, not events: the
 * writer is the mcp-server *process*, which the app gets no in-process
 * signal from — the shared SQLite database is the only channel. */
const POLL_INTERVAL_MS = 2000;

export function useAgentActions() {
  const queryClient = useQueryClient();

  const actions = useQuery({
    queryKey: agentActionsQueryKey,
    queryFn: listPendingAgentActions,
    refetchInterval: POLL_INTERVAL_MS,
  });

  const invalidate = () => {
    queryClient.invalidateQueries({ queryKey: agentActionsQueryKey });
    // Approving add_to_queue/start_download/download_all all change queue
    // rows; refetching unconditionally is simpler than special-casing.
    queryClient.invalidateQueries({ queryKey: queueQueryKey });
  };

  const approve = useMutation({
    mutationFn: approveAgentAction,
    onSettled: invalidate,
  });

  const reject = useMutation({
    mutationFn: rejectAgentAction,
    onSettled: invalidate,
  });

  return { actions, approve, reject };
}
