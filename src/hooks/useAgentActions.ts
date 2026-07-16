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

  // Approve several pending actions in one click. Sequential, not parallel:
  // approving a start_download/download_all awaits execution and is guarded
  // by the backend's batch/registry locks, so firing them concurrently would
  // race or be rejected. A single failing action doesn't abort the rest —
  // failures are collected and surfaced together at the end.
  const approveAll = useMutation({
    mutationFn: async (actionIds: number[]) => {
      const failures: string[] = [];
      for (const id of actionIds) {
        try {
          await approveAgentAction(id);
        } catch (e) {
          failures.push(e instanceof Error ? e.message : String(e));
        }
      }
      if (failures.length > 0) {
        throw new Error(
          `${failures.length} request${failures.length > 1 ? "s" : ""} failed: ${failures.join("; ")}`
        );
      }
    },
    onSettled: invalidate,
  });

  return { actions, approve, reject, approveAll };
}
