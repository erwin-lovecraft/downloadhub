import { create } from "zustand";
import type { DefaultQuality } from "@/types";
import { notAvailable } from "@/store/toast";

const QUALITY_CYCLE: DefaultQuality[] = ["2160p", "1080p", "720p", "360p"];

interface SettingsState {
  outputFolderPath: string;
  defaultQuality: DefaultQuality;
  concurrentLimit: number;
  cycleQuality: () => void;
  incConcurrency: () => void;
  decConcurrency: () => void;
  changeFolder: () => void;
}

export const useSettingsStore = create<SettingsState>((set) => ({
  outputFolderPath: "C:\\Users\\Guest\\Downloads",
  defaultQuality: "1080p",
  concurrentLimit: 2,
  cycleQuality: () =>
    set((s) => {
      const idx = QUALITY_CYCLE.indexOf(s.defaultQuality);
      return { defaultQuality: QUALITY_CYCLE[(idx + 1) % QUALITY_CYCLE.length] };
    }),
  incConcurrency: () => set((s) => ({ concurrentLimit: Math.min(5, s.concurrentLimit + 1) })),
  decConcurrency: () => set((s) => ({ concurrentLimit: Math.max(1, s.concurrentLimit - 1) })),
  changeFolder: () => notAvailable("Preview only — folder picker unavailable"),
}));
