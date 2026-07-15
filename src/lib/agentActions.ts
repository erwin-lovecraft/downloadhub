import { invoke } from "@tauri-apps/api/core";

/** Mirrors core::agent::AgentActionRequest (serde-tagged on `kind`). */
export type AgentActionRequest =
  | {
      kind: "add_to_queue";
      entry: {
        video_id: string;
        title: string;
        itag: number;
        quality_label: string | null;
        output_path: string;
      };
    }
  | { kind: "start_download"; queue_id: number; title: string }
  | { kind: "download_all" };

export type AgentActionStatus = "pending" | "approved" | "rejected" | "completed" | "failed";

export interface PendingAgentAction {
  id: number;
  request: AgentActionRequest;
  status: AgentActionStatus;
  requested_by: string | null;
  error_message: string | null;
  created_at: number;
  resolved_at: number | null;
}

export const listPendingAgentActions = () =>
  invoke<PendingAgentAction[]>("list_pending_agent_actions");

export const approveAgentAction = (actionId: number) =>
  invoke<void>("approve_agent_action", { actionId });

export const rejectAgentAction = (actionId: number) =>
  invoke<void>("reject_agent_action", { actionId });

/** One-line human description of what approving the action will do. */
export function describeAgentAction(request: AgentActionRequest): string {
  switch (request.kind) {
    case "add_to_queue":
      return `Add to queue: "${request.entry.title}" (${
        request.entry.quality_label ?? `itag ${request.entry.itag}`
      }) into ${request.entry.output_path}`;
    case "start_download":
      return `Start downloading: "${request.title}" (queue entry #${request.queue_id})`;
    case "download_all":
      return "Download every queued entry, one at a time";
  }
}
