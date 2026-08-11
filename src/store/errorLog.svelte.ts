/**
 * Session error log.
 *
 * A toast disappears after five seconds, which is not long enough to read a
 * command and an exit code, let alone copy them into a bug report. Every
 * reported error is kept here for the lifetime of the session so the details
 * view can show it afterwards.
 *
 * Nothing is persisted: this is a debugging aid, not a record, and the entries
 * contain command output we have no reason to keep on disk.
 */

import type { AppError } from "../lib/errors";

export interface ErrorLogEntry {
  id: number;
  error: AppError;
  /** What the user was doing, when the call site said so. */
  action?: string;
  timestamp: number;
}

/** Older entries are dropped; the recent ones are the ones being debugged. */
const MAX_ENTRIES = 50;

let _nextId = 1;

export const errorLogState = $state({
  entries: [] as ErrorLogEntry[],
  isOpen: false,
});

export function recordError(error: AppError, action?: string): void {
  errorLogState.entries.unshift({
    id: _nextId++,
    error,
    action,
    timestamp: Date.now(),
  });
  if (errorLogState.entries.length > MAX_ENTRIES) {
    errorLogState.entries.length = MAX_ENTRIES;
  }
}

export function clearErrorLog(): void {
  errorLogState.entries = [];
}

export function openErrorLog(): void {
  errorLogState.isOpen = true;
}

export function closeErrorLog(): void {
  errorLogState.isOpen = false;
}

/**
 * Plain-text rendering of an entry for the clipboard.
 *
 * Fields are already redacted upstream (`errors.ts` / `redact.rs`), so this is
 * safe to paste into an issue.
 */
export function formatEntryForClipboard(entry: ErrorLogEntry): string {
  const lines = [
    `code: ${entry.error.code}`,
    `detail: ${entry.error.detail}`,
  ];
  if (entry.action) lines.unshift(`action: ${entry.action}`);
  if (entry.error.command) lines.push(`command: ${entry.error.command}`);
  if (entry.error.exitCode !== undefined) lines.push(`exit code: ${entry.error.exitCode}`);
  lines.push(`time: ${new Date(entry.timestamp).toISOString()}`);
  return lines.join("\n");
}
