/**
 * Unique client-side identifiers.
 *
 * These ids were previously `Date.now().toString()`. Two items created in the
 * same millisecond — a user message followed immediately by a system message,
 * or a burst of agent status lines — got the same id, which caused two failures:
 *
 *   1. `{#each aiState.messages as msg (msg.id)}` threw `each_key_duplicate`.
 *   2. Worse and silent: `ai_chat_save_message` upserts with
 *      `ON CONFLICT(id) DO UPDATE SET content`, so the second message
 *      overwrote the first in the persisted history.
 *
 * Nothing sorts on these ids (the backend orders chat history by `created_at`),
 * so they only need to be unique, not monotonic.
 */

/** Counter backing the fallback path, so ids stay unique within a session. */
let _counter = 0;

/**
 * Generate a unique id, optionally namespaced.
 *
 * @param prefix optional label kept for readability in logs, e.g. `newId("cron")`
 */
export function newId(prefix?: string): string {
  const id = randomId();
  return prefix ? `${prefix}-${id}` : id;
}

function randomId(): string {
  // Available in the Tauri webview and in browsers on localhost (a secure
  // context), which covers both modes this app runs in.
  const c = globalThis.crypto;
  if (c && typeof c.randomUUID === "function") {
    return c.randomUUID();
  }
  // Fallback: timestamp for readability, counter for in-session uniqueness,
  // random suffix so two sessions cannot collide on the same millisecond.
  _counter += 1;
  const random = Math.random().toString(36).slice(2, 10);
  return `${Date.now().toString(36)}-${_counter.toString(36)}-${random}`;
}
