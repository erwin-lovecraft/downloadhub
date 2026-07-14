import { create } from "zustand";
import type { Agent } from "@/types";
import { ACTIVITY_LOG, AGENTS_SEED, AGENT_PENDING_SEED } from "@/data/mock";
import { useQueueStore } from "@/store/queue";
import { notAvailable } from "@/store/toast";

interface AgentsState {
  agents: Agent[];
  pending: typeof AGENT_PENDING_SEED;
  activityLog: typeof ACTIVITY_LOG;
  approve: (id: string) => void;
  deny: (id: string) => void;
  togglePermission: (key: string) => void;
  disconnect: (key: string) => void;
  connectAgent: () => void;
}

export const useAgentsStore = create<AgentsState>((set, get) => ({
  agents: AGENTS_SEED.map((x) => ({ ...x })),
  pending: AGENT_PENDING_SEED.map((x) => ({ ...x })),
  activityLog: ACTIVITY_LOG,

  approve: (id) => {
    const proposal = get().pending.find((p) => p.id === id);
    if (!proposal) return;
    useQueueStore.getState().addFromAgentProposal({
      title: proposal.videoTitle,
      channel: `${proposal.agentName} proposal`,
      thumbGradient: proposal.thumbGradient,
      format: proposal.format,
    });
    set((s) => ({ pending: s.pending.filter((p) => p.id !== id) }));
    notAvailable("Approved — added to queue");
  },

  deny: (id) => {
    set((s) => ({ pending: s.pending.filter((p) => p.id !== id) }));
    notAvailable("Proposal dismissed");
  },

  togglePermission: (key) =>
    set((s) => ({
      agents: s.agents.map((a) =>
        a.key === key ? { ...a, permission: a.permission === "auto" ? "manual" : "auto" } : a,
      ),
    })),

  disconnect: () => notAvailable("Preview only — disconnect unavailable"),
  connectAgent: () => notAvailable("Preview only — connect flow unavailable"),
}));
