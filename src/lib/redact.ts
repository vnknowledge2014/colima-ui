/**
 * Redaction of secrets from strings that reach the UI, the clipboard, or an
 * LLM prompt.
 *
 * Mirrors `src-tauri/src/redact.rs`. Both sides are needed: the frontend calls
 * LLM providers directly (see `llmProviders.ts`), so provider errors are built
 * in the browser and never pass through Rust.
 *
 * Two rules, same as the Rust side:
 *   1. Redact by position — credential-shaped query parameters and auth headers
 *      are redacted whatever the value looks like. This is what protects
 *      providers we have never heard of.
 *   2. Redact by shape — known key formats are caught wherever they appear.
 */

const MASK = "<redacted>";

/** Credential-bearing query parameters: `?key=`, `&api_key=`, `?access_token=` … */
const QUERY_PARAM =
  /([?&](?:api[_-]?key|apikey|access[_-]?token|auth[_-]?token|key|token|auth|secret|password)=)[^&\s)\]}"']+/gi;

/** `Authorization: Bearer <token>` in any casing. */
const BEARER = /(bearer\s+)[A-Za-z0-9._-]{8,}/gi;

/**
 * Header-style credentials: `x-api-key: sk-…`, `"x-goog-api-key": "AIza…"`.
 *
 * The value class allows one optional space-separated second token so that
 * `Authorization: Basic dXNlcjpwdw==` masks the credential rather than just the
 * word `Basic`. BEARER handles the bearer scheme on its own.
 */
const HEADER_KEY =
  /((?:x-api-key|x-goog-api-key|api-key|authorization)"?\s*[:=]\s*"?)[^\s,;"'})]+(?:\s+[^\s,;"'})]+)?/gi;

/**
 * Provider key shapes. Deliberately conservative — a pattern must be
 * distinctive enough that it cannot match legitimate output.
 *
 * Bare hex runs are intentionally absent: a 64-char hex pattern would also
 * match Docker image digests, which must stay readable. Rule 1 covers the
 * providers that use opaque keys.
 */
const KEY_SHAPES: RegExp[] = [
  /sk-[A-Za-z0-9._-]{16,}/g, // OpenAI, Anthropic, OpenRouter, DeepSeek, Together
  /AIza[A-Za-z0-9_-]{20,}/g, // Google AI Studio / Gemini
  /gsk_[A-Za-z0-9]{20,}/g, // Groq
  /hf_[A-Za-z0-9]{16,}/g, // Hugging Face
  /gh[pousr]_[A-Za-z0-9]{16,}/g, // GitHub
  /colima-[0-9a-f]{64}/g, // This app's own HTTP API bearer token
  /AKIA[0-9A-Z]{16}/g, // AWS access key id
  /eyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}/g, // JWT
];

/**
 * Entries under `/Users` that name a directory rather than an account.
 */
const NON_ACCOUNT_HOME_DIRS = new Set(["Shared", "Guest"]);

/**
 * The user's account name in a home path.
 *
 * Not a credential, which is why it is not in {@link KEY_SHAPES} — but a
 * JavaScript stack trace carries absolute paths, and `crashReporter` sends those
 * onward. The Rust side masks these; leaving the frontend out would mean the
 * account name leaks from exactly one of the two crash paths.
 *
 * Applied last, so it cannot cut into a value an earlier rule already masked.
 */
const HOME_PATH = /\/(Users|home)\/([^/\s:,;"')\]]+)/g;

/**
 * Strip credentials from a string before showing, copying, or sending it.
 *
 * Apply at the point the string is built where possible; `globalToast` applies
 * it as a backstop for everything that reaches the user.
 */
export function redact(input: string): string {
  let out = input
    .replace(QUERY_PARAM, `$1${MASK}`)
    .replace(BEARER, `$1${MASK}`)
    .replace(HEADER_KEY, `$1${MASK}`);

  for (const shape of KEY_SHAPES) {
    out = out.replace(shape, MASK);
  }

  return out.replace(HOME_PATH, (match, root: string, name: string) =>
    NON_ACCOUNT_HOME_DIRS.has(name) ? match : `/${root}/<user>`,
  );
}

/** Redact an unknown thrown value on its way to a message. */
export function redactError(e: unknown): string {
  if (e instanceof Error) return redact(e.message);
  if (typeof e === "string") return redact(e);
  try {
    return redact(JSON.stringify(e));
  } catch {
    return redact(String(e));
  }
}
