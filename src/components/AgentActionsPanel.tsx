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
  const { actions, approve, reject } = useAgentActions();

  const unresolved = actions.data ?? [];
  const mutationError = approve.error ?? reject.error;

  if (unresolved.length === 0 && !mutationError) return null;

  return (
    <div className="flex shrink-0 flex-col gap-2 rounded-md border border-amber-500/50 bg-amber-500/5 p-3">
      <span className="text-sm font-medium">AI agent requests</span>

      {mutationError && (
        <p className="text-xs text-destructive">
          {mutationError instanceof Error ? mutationError.message : String(mutationError)}
        </p>
      )}

      <ul className="flex flex-col gap-2">
        {unresolved.map((action) => {
          const executing = action.status === "approved";
          const busy = approve.isPending || reject.isPending;
          return (
            <li key={action.id} className="flex flex-col gap-1.5 rounded-md border bg-background p-2 text-xs">
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
