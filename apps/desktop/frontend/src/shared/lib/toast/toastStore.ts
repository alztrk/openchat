import type { ReactNode } from 'react';
import { create } from 'zustand';
import type { ToastVariant } from '@shared/components/Toast';

export interface ToastAction {
  label: string;
  onClick(): void;
}

export interface ToastOptions {
  id?: string;
  variant?: ToastVariant;
  action?: ToastAction;
  durationMs?: number;
}

export interface ToastData {
  id: string;
  message: ReactNode;
  variant: ToastVariant;
  action?: ToastAction;
  durationMs: number;
  createdAt: number;
}

interface ToastStore {
  toasts: ToastData[];
  show(message: ReactNode, options?: ToastOptions): string;
  dismiss(id: string): void;
  clear(): void;
}

const DEFAULT_DURATION_MS = 5000;
let toastCounter = 0;
const activeTimers = new Map<string, ReturnType<typeof setTimeout>>();

export const useToastStore = create<ToastStore>((set, get) => ({
  toasts: [],
  show(message, options = {}) {
    const id = options.id ?? `toast-${Date.now()}-${++toastCounter}`;
    const variant = options.variant ?? 'destructive';
    const durationMs = options.durationMs !== undefined ? options.durationMs : DEFAULT_DURATION_MS;

    const existingTimer = activeTimers.get(id);
    if (existingTimer) {
      clearTimeout(existingTimer);
      activeTimers.delete(id);
    }

    const item: ToastData = {
      id,
      message,
      variant,
      action: options.action,
      durationMs,
      createdAt: Date.now(),
    };

    set((state) => ({
      toasts: [...state.toasts.filter((t) => t.id !== id), item],
    }));

    if (durationMs > 0) {
      const timer = setTimeout(() => {
        activeTimers.delete(id);
        get().dismiss(id);
      }, durationMs);
      activeTimers.set(id, timer);
    }

    return id;
  },
  dismiss(id) {
    const timer = activeTimers.get(id);
    if (timer) {
      clearTimeout(timer);
      activeTimers.delete(id);
    }
    set((state) => ({ toasts: state.toasts.filter((t) => t.id !== id) }));
  },
  clear() {
    for (const timer of activeTimers.values()) {
      clearTimeout(timer);
    }
    activeTimers.clear();
    set({ toasts: [] });
  },
}));

/** Imperative toast helpers callable anywhere without React hooks. */
export const toast = {
  show(message: ReactNode, options?: ToastOptions): string {
    return useToastStore.getState().show(message, options);
  },
  error(message: ReactNode, options?: Omit<ToastOptions, 'variant'>): string {
    return useToastStore.getState().show(message, { ...options, variant: 'destructive' });
  },
  success(message: ReactNode, options?: Omit<ToastOptions, 'variant'>): string {
    return useToastStore.getState().show(message, { ...options, variant: 'success' });
  },
  warning(message: ReactNode, options?: Omit<ToastOptions, 'variant'>): string {
    return useToastStore.getState().show(message, { ...options, variant: 'warning' });
  },
  info(message: ReactNode, options?: Omit<ToastOptions, 'variant'>): string {
    return useToastStore.getState().show(message, { ...options, variant: 'info' });
  },
  dismiss(id: string): void {
    useToastStore.getState().dismiss(id);
  },
  clear(): void {
    useToastStore.getState().clear();
  },
};
