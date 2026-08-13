/**
 * Global Toast Notification System
 *
 * Module-level event emitter that allows any component (even unmounted ones)
 * to show toast notifications. The toast UI is rendered in App.svelte which
 * never unmounts, ensuring notifications are always visible.
 */

import { redact } from "./redact";
import type { AppError } from "./errors";
import { pushNotification } from "../store/notifications.svelte";

export type ToastType = "success" | "error" | "info";

export interface ToastMessage {
  id: number;
  type: ToastType;
  text: string;
  timestamp: number;
  /** Structured payload when the toast came from a failed operation. */
  error?: AppError;
  /** Remediation line shown under the message. */
  hint?: string;
  /** How many identical toasts have collapsed into this one. */
  count: number;
}

export interface ToastOptions {
  error?: AppError;
  hint?: string;
  /**
   * Whether this toast also becomes a notification entry. Default true.
   *
   * `errorReporter` sets it false because it has already called `recordError`;
   * without that the same failure would be listed twice. A flag rather than
   * inferring it from `error` being present: a call site is free to pass a
   * structured error without having recorded it, and guessing would be wrong
   * exactly when it mattered.
   */
  record?: boolean;
}

type ToastListener = (toast: ToastMessage) => void;
type ToastUpdateListener = (toast: ToastMessage) => void;
type ToastRemoveListener = (id: number) => void;

let _nextId = 1;
const _listeners: Set<ToastListener> = new Set();
const _updateListeners: Set<ToastUpdateListener> = new Set();
const _removeListeners: Set<ToastRemoveListener> = new Set();

/** How long each kind stays up. Errors carry something to act on. */
const DISMISS_MS: Record<ToastType, number> = { error: 9000, success: 4000, info: 5000 };

/** Pending expiry timers, so a collapsed repeat can extend its toast's life. */
const _timers = new Map<number, ReturnType<typeof setTimeout>>();

/**
 * Toasts currently on screen, so repeats can be collapsed instead of stacking.
 *
 * A failing background poller used to be able to paper the screen over; now the
 * repeat increments a counter on the existing toast.
 */
const _active = new Map<string, ToastMessage>();

/** Beyond this, the oldest toast is dropped rather than growing the stack. */
const MAX_ACTIVE_TOASTS = 3;

/** Identity used for collapsing: same type and same text. */
function toastKey(type: ToastType, text: string): string {
  return `${type}\u0000${text}`;
}

/**
 * Remove a toast and tell the UI to stop showing it.
 *
 * Lifecycle is owned here, not in the component. When the component owned the
 * expiry timer, a toast emitted before `ToastContainer` mounted was added to
 * `_active` and never released — permanently suppressing that message for the
 * rest of the session, because every repeat collapsed into an invisible entry.
 */
export function dismissToast(id: number): void {
  const timer = _timers.get(id);
  if (timer !== undefined) {
    clearTimeout(timer);
    _timers.delete(id);
  }
  for (const [key, toast] of _active) {
    if (toast.id === id) {
      _active.delete(key);
      break;
    }
  }
  _removeListeners.forEach((fn) => fn(id));
}

function scheduleDismiss(toast: ToastMessage): void {
  const existing = _timers.get(toast.id);
  if (existing !== undefined) clearTimeout(existing);
  _timers.set(
    toast.id,
    setTimeout(() => dismissToast(toast.id), DISMISS_MS[toast.type]),
  );
}

/** Show a global toast notification from anywhere */
export function globalToast(type: ToastType, text: string, options?: ToastOptions): void {
  // Redact here rather than at each of the ~100 call sites. This is the last
  // point every user-visible message passes through, and error text is also
  // forwarded to the AI diagnostics listener below — i.e. off to an LLM — so
  // an unredacted string would leak both to the screen and to a third party.
  const safeText = redact(text);
  const key = toastKey(type, safeText);

  // A toast lasts seconds; the notification list is where it stays readable.
  //
  // Recorded before the collapse branch below, which returns early: that branch
  // means "a toast with this text is still on screen", which says nothing about
  // whether the list already has it. The store applies its own, much longer,
  // collapse window — tying the two together is what made a poller retrying every
  // five seconds add an entry per retry, since its toast had already expired.
  if (options?.record !== false) {
    pushNotification({
      kind: "message",
      status: type === "error" ? "error" : "success",
      title: safeText,
      detail: options?.hint,
      error: options?.error,
      hint: options?.hint,
    });
  }

  // Collapse a repeat into the toast already on screen.
  const existing = _active.get(key);
  if (existing) {
    existing.count += 1;
    existing.timestamp = Date.now();
    // Restart the clock: the condition is still happening.
    scheduleDismiss(existing);
    _updateListeners.forEach((fn) => fn(existing));
    return;
  }

  const toast: ToastMessage = {
    id: _nextId++,
    type,
    text: safeText,
    timestamp: Date.now(),
    error: options?.error,
    hint: options?.hint ? redact(options.hint) : undefined,
    count: 1,
  };

  if (_active.size >= MAX_ACTIVE_TOASTS) {
    // Drop the oldest so a burst cannot fill the screen. This must also remove
    // it from the UI, otherwise the map and the screen disagree and a repeat
    // would render a second copy of a toast that is still visible.
    const oldest = [..._active.values()].sort((a, b) => a.timestamp - b.timestamp)[0];
    if (oldest) dismissToast(oldest.id);
  }
  _active.set(key, toast);
  scheduleDismiss(toast);

  _listeners.forEach((fn) => fn(toast));
  // Notify error listeners (used by AI diagnostics bubble for auto-trigger)
  if (type === "error") {
    _errorListeners.forEach((fn) => fn(safeText));
  }
}

/** Subscribe to toast events (used by App.svelte) */
export function onToast(fn: ToastListener): () => void {
  _listeners.add(fn);
  return () => _listeners.delete(fn);
}

/** Subscribe to updates of a toast already on screen (repeat collapsing). */
export function onToastUpdate(fn: ToastUpdateListener): () => void {
  _updateListeners.add(fn);
  return () => _updateListeners.delete(fn);
}

/** Subscribe to removals, whether by timeout, eviction, or the close button. */
export function onToastRemove(fn: ToastRemoveListener): () => void {
  _removeListeners.add(fn);
  return () => _removeListeners.delete(fn);
}

/** Test seam: forget on-screen toasts so collapsing starts fresh. */
export function _resetToastsForTest(): void {
  for (const timer of _timers.values()) clearTimeout(timer);
  _timers.clear();
  _active.clear();
}

// ===== Error-specific listener for AI Diagnostics =====

type ErrorListener = (error: string) => void;
const _errorListeners: Set<ErrorListener> = new Set();

/** Subscribe to error-only events (used by AiChatBubble for auto-trigger) */
export function onError(fn: ErrorListener): () => void {
  _errorListeners.add(fn);
  return () => _errorListeners.delete(fn);
}
