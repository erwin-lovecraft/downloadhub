import { useEffect } from "react";
import { useQueueStore } from "@/store/queue";
import { useSettingsStore } from "@/store/settings";

/**
 * Mock-only progress simulation, ported from the design prototype's
 * `tick()`. Phase 1 step 7 replaces this with real Tauri progress events
 * (`useEffect` subscribing to a `download-progress` event instead of a
 * timer) — the queue store's shape doesn't need to change.
 */
export function useQueueSimulation(): void {
  useEffect(() => {
    const interval = setInterval(() => {
      useQueueStore.getState().tick(useSettingsStore.getState().concurrentLimit);
    }, 900);
    return () => clearInterval(interval);
  }, []);
}
