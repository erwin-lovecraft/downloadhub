import { create } from "zustand";
import type { QueueItem, Video, FormatOption } from "@/types";
import { QUEUE_SEED } from "@/data/mock";
import { notAvailable } from "@/store/toast";

interface QueueState {
  items: QueueItem[];
  addFromVideo: (video: Video, format: FormatOption) => void;
  quickAdd: (video: Video) => void;
  addFromAgentProposal: (item: Omit<QueueItem, "id" | "status" | "progress">) => void;
  startAll: () => void;
  clearCompleted: () => void;
  pauseItem: (id: string) => void;
  resumeItem: (id: string) => void;
  cancelItem: (id: string) => void;
  retryItem: (id: string) => void;
  openFolder: () => void;
  tick: (concurrentLimit: number) => void;
}

function newId(): string {
  return `${Date.now()}-${Math.random()}`;
}

export const useQueueStore = create<QueueState>((set) => ({
  items: QUEUE_SEED.map((x) => ({ ...x })),

  addFromVideo: (video, format) => {
    const item: QueueItem = {
      id: newId(),
      title: video.title,
      channel: video.channel,
      thumbGradient: video.thumbGradient,
      format: `${format.res} · ${format.key === "audio" ? "M4A" : "MP4"}`,
      status: "queued",
      progress: 0,
    };
    set((s) => ({ items: [item, ...s.items] }));
    notAvailable("Added to queue");
  },

  quickAdd: (video) => {
    const item: QueueItem = {
      id: newId(),
      title: video.title,
      channel: video.channel,
      thumbGradient: video.thumbGradient,
      format: "1080p · MP4",
      status: "queued",
      progress: 0,
    };
    set((s) => ({ items: [item, ...s.items] }));
    notAvailable("Added to queue");
  },

  addFromAgentProposal: (item) => {
    const queueItem: QueueItem = { ...item, id: newId(), status: "queued", progress: 0 };
    set((s) => ({ items: [queueItem, ...s.items] }));
  },

  startAll: () =>
    set((s) => ({
      items: s.items.map((it) =>
        it.status === "queued" || it.status === "paused" ? { ...it, status: "downloading" } : it,
      ),
    })),

  clearCompleted: () => set((s) => ({ items: s.items.filter((it) => it.status !== "completed") })),

  pauseItem: (id) =>
    set((s) => ({ items: s.items.map((it) => (it.id === id ? { ...it, status: "paused" } : it)) })),

  resumeItem: (id) =>
    set((s) => ({
      items: s.items.map((it) => (it.id === id ? { ...it, status: "downloading" } : it)),
    })),

  cancelItem: (id) => set((s) => ({ items: s.items.filter((it) => it.id !== id) })),

  retryItem: (id) =>
    set((s) => ({
      items: s.items.map((it) =>
        it.id === id ? { ...it, status: "queued", progress: 0, error: undefined } : it,
      ),
    })),

  openFolder: () => notAvailable("Preview only — folder not available"),

  tick: (concurrentLimit) =>
    set((s) => {
      let downloadingCount = 0;
      let items = s.items.map((it) => {
        if (it.status !== "downloading") return it;
        downloadingCount++;
        const progress = Math.min(100, it.progress + 3 + Math.floor(Math.random() * 7));
        if (progress >= 100) return { ...it, status: "completed" as const, progress: 100 };
        return {
          ...it,
          progress,
          speed: `${(2 + Math.random() * 4).toFixed(1)} MB/s`,
          eta: `${Math.max(1, Math.round((100 - progress) / 8))} min`,
        };
      });

      let capacity = concurrentLimit - downloadingCount;
      items = items.map((it) => {
        if (capacity > 0 && it.status === "queued") {
          capacity--;
          return { ...it, status: "downloading" as const, speed: "-- MB/s", eta: "--" };
        }
        return it;
      });

      return { items };
    }),
}));
