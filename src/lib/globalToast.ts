/**
 * Global Toast Notification System
 * 
 * Module-level event emitter that allows any component (even unmounted ones)
 * to show toast notifications. The toast UI is rendered in App.svelte which
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
  // Notify error listeners (used by AI diagnostics bubble for auto-trigger)
  if (type === "error") {
    _errorListeners.forEach((fn) => fn(text));
  }
}

/** Subscribe to toast events (used by App.svelte) */
export function onToast(fn: ToastListener): () => void {
  _listeners.add(fn);
  return () => _listeners.delete(fn);
}

// ===== Error-specific listener for AI Diagnostics =====

type ErrorListener = (error: string) => void;
const _errorListeners: Set<ErrorListener> = new Set();

/** Subscribe to error-only events (used by AiChatBubble for auto-trigger) */
export function onError(fn: ErrorListener): () => void {
  _errorListeners.add(fn);
  return () => _errorListeners.delete(fn);
}
