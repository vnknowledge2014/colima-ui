/**
 * Reporting layer between a failed operation and the user.
 *
 * Call sites should not decide how an error is presented — they pass what they
 * were doing, and this module produces a localized toast, a hint, and an entry
 * in the session error log for the details view.
 */

import { globalToast } from "./globalToast";
import { normalizeError, errorMessage, errorHint, type AppError } from "./errors";
import { recordError } from "../store/errorLog.svelte";

export interface ErrorContext {
  /** What the user was trying to do, already localized. e.g. "Stop container". */
  action?: string;
}

/**
 * Report a failed operation: toast + session log.
 *
 * Returns the normalized error so callers can branch on `code` if they need to.
 */
export function reportError(e: unknown, ctx: ErrorContext = {}): AppError {
  const err = normalizeError(e);
  // Title *and* detail: the detail is usually the actionable part ("container
  // is running, stop it first"). Showing only the title would be a regression
  // against the plain `String(e)` toasts this replaces — and the same text is
  // forwarded to the AI diagnostics listener, which needs the specifics.
  const message = errorMessage(err);
  const text = ctx.action ? `${ctx.action}: ${message}` : message;

  recordError(err, ctx.action);
  globalToast("error", text, { error: err, hint: errorHint(err) });
  return err;
}

/**
 * Wrap an async action so failures are reported instead of vanishing.
 *
 * Returns `undefined` on failure, so callers can do:
 *   `const result = await withErrorReport(() => api.stop(id), { action: t('...') })`
 */
export async function withErrorReport<T>(
  fn: () => Promise<T>,
  ctx: ErrorContext = {},
): Promise<T | undefined> {
  try {
    return await fn();
  } catch (e) {
    reportError(e, ctx);
    return undefined;
  }
}
