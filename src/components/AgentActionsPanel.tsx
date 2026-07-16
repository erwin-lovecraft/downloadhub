import { CheckCheckIcon } from "lucide-react";
import { useAgentActions } from "@/hooks/useAgentActions";
import { describeAgentAction } from "@/lib/agentActions";
import { Button } from "@/components/ui/button";

/**
 * Approval gate for requests made by external AI agents through the MCP
 * server. Renders nothing while there's nothing to decide; otherwise shows
 * each request with Approve/Reject — nothing an agent asks for runs until
 * it's approved here.
 */
export function AgentActionsPanel() {
  const { actions, approve, reject, approveAll } = useAgentActions();

  const unresolved = actions.data ?? [];
  const mutationError = approve.error ?? reject.error ?? approveAll.error;
  const pending = unresolved.filter((action) => action.status === "pending");
  const busy = approve.isPending || reject.isPending || approveAll.isPending;

  if (unresolved.length === 0 && !mutationError) return null;

  return (
    <div className="flex shrink-0 flex-col gap-2 rounded-xl border border-amber-600/40 bg-amber-50 p-3 dark:border-amber-500/40 dark:bg-amber-500/10">
      <div className="flex items-center justify-between gap-2">
        <span className="text-sm font-semibold">AI agent requests</span>
        {pending.length > 1 && (
          <Button
            size="xs"
            disabled={busy}
            onClick={() => approveAll.mutate(pending.map((action) => action.id))}
          >
            <CheckCheckIcon />
            {approveAll.isPending ? "Approving..." : `Approve all (${pending.length})`}
          </Button>
        )}
      </div>

      {mutationError && (
        <p className="text-xs text-destructive">
          {mutationError instanceof Error ? mutationError.message : String(mutationError)}
        </p>
      )}

      <ul className="flex flex-col gap-2">
        {unresolved.map((action) => {
          const executing = action.status === "approved";
          return (
            <li key={action.id} className="flex flex-col gap-1.5 rounded-lg border bg-card p-2 text-xs shadow-xs">
              <span>{describeAgentAction(action.request)}</span>
              <span className="text-muted-foreground">
                Requested by {action.requested_by ?? "an MCP client"}
              </span>
              {executing ? (
                <span className="text-muted-foreground">Approved — running...</span>
              ) : (
                <div className="flex items-center gap-2">
                  <Button
                    size="xs"
                    disabled={busy}
                    onClick={() => approve.mutate(action.id)}
                  >
                    Approve
                  </Button>
                  <Button
                    size="xs"
                    variant="outline"
                    disabled={busy}
                    onClick={() => reject.mutate(action.id)}
                  >
                    Reject
                  </Button>
                </div>
              )}
            </li>
          );
        })}
      </ul>
    </div>
  );
}
