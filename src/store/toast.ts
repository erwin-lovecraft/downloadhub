import { create } from "zustand";

interface Toast {
  id: string;
  text: string;
}

interface ToastState {
  toasts: Toast[];
  addToast: (text: string) => void;
  dismissToast: (id: string) => void;
}

export const useToastStore = create<ToastState>((set) => ({
  toasts: [],
  addToast: (text) => {
    const id = `${Date.now()}-${Math.random()}`;
    set((s) => ({ toasts: [...s.toasts, { id, text }] }));
    setTimeout(() => {
      set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) }));
    }, 3000);
  },
  dismissToast: (id) => set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) })),
}));

/** Shorthand for actions that aren't implemented yet in this UI-only pass. */
export function notAvailable(msg: string): void {
  useToastStore.getState().addToast(msg);
}
