import { create } from "zustand";
import type { FormatKey, NavKey, Video } from "@/types";

type View = "login" | "app";

interface SessionState {
  view: View;
  activeNav: NavKey;
  detailOpen: boolean;
  selectedVideo: Video | null;
  selectedFormat: FormatKey;
  login: () => void;
  signOut: () => void;
  navigate: (key: NavKey) => void;
  openDetail: (video: Video) => void;
  closeDetail: () => void;
  selectFormat: (key: FormatKey) => void;
}

export const useSessionStore = create<SessionState>((set) => ({
  view: "login",
  activeNav: "search",
  detailOpen: false,
  selectedVideo: null,
  selectedFormat: "p1080",
  login: () => set({ view: "app" }),
  signOut: () => set({ view: "login", activeNav: "search", detailOpen: false }),
  navigate: (key) => set({ activeNav: key }),
  openDetail: (video) => set({ selectedVideo: video, detailOpen: true, selectedFormat: "p1080" }),
  closeDetail: () => set({ detailOpen: false }),
  selectFormat: (key) => set({ selectedFormat: key }),
}));
