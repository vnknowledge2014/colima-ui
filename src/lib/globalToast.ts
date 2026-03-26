/**
 * Global Toast Notification System
 * 
 * Module-level event emitter that allows any component (even unmounted ones)
 * to show toast notifications. The toast UI is rendered in App.tsx which
 * never unmounts, ensuring notifications are always visible.
 */

export type ToastType = "success" | "error" | "info";

export interface ToastMessage {
  id: number;
  type: ToastType;
  text: string;
  timestamp: number;
}

type ToastListener = (toast: ToastMessage) => void;

let _nextId = 1;
const _listeners: Set<ToastListener> = new Set();

/** Show a global toast notification from anywhere */
export function globalToast(type: ToastType, text: string): void {
  const toast: ToastMessage = {
    id: _nextId++,
    type,
    text,
    timestamp: Date.now(),
  };
  _listeners.forEach((fn) => fn(toast));
}

/** Subscribe to toast events (used by App.tsx) */
export function onToast(fn: ToastListener): () => void {
  _listeners.add(fn);
  return () => _listeners.delete(fn);
}
