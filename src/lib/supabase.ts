import type { SupportedStorage } from "@supabase/supabase-js";
import { createClient, type SupabaseClient } from "@supabase/supabase-js";
import { isRunningInTauri } from "./env";

/**
 * Supabase Auth — identity only.
 *
 * This client answers "who is signed in". Since license keys were retired, that
 * answer also determines which Polar seat subscription applies — but the
 * entitlement itself is resolved elsewhere (`lib/api/subscription.ts` →
 * `src-tauri/src/subscription/`) and cached offline.
 *
 * Nothing in this file is consulted to decide whether a feature is unlocked, and
 * no application data is stored in Supabase at all. Supabase being unreachable
 * must not downgrade a paying customer.
 *
 * # What is safe to put in this file
 *
 * Only public client config: the project URL and the **anon** key. Both ship in
 * the bundle by design, exactly like `POLAR_ORG_ID` — the anon key is a public
 * identifier, not a credential, and PKCE means an intercepted auth code is
 * useless without the verifier.
 *
 * The project's Postgres connection string / database password is a *server*
 * secret with no role in this app. It must never appear here, in any config, or
 * in any log.
 */

/** Public project URL. Derived from the project ref; not a secret. */
export const SUPABASE_URL = "https://nakwwkderqjfsxajduwj.supabase.co";

/**
 * Public anon key from Supabase → Project Settings → API.
 *
 * Empty until the owner supplies it. While empty the account feature reports
 * itself as unconfigured and the sign-in buttons are disabled with a visible
 * explanation, rather than failing on click.
 */
export const SUPABASE_ANON_KEY = "sb_publishable_Hr1D3MJZ7lvbr60eIA9IzQ_jDTUqLgH";

/** The OAuth return address. Must be allow-listed in Supabase → Auth → URL Configuration. */
export const AUTH_REDIRECT_URL = "colimaui://auth-callback";

/**
 * Whether sign-in can work at all in this build and runtime.
 *
 * Two independent reasons it may not:
 *
 *  - **Not configured** — no anon key compiled in yet.
 *  - **Browser mode** — the OAuth return path is a registered `colimaui://` URL
 *    scheme handled by the desktop app, and the session is stored in the OS
 *    keychain. A browser tab has neither. Rather than fall back to persisting a
 *    refresh token in `localStorage`, sign-in is desktop-only and says so.
 */
export function accountAvailability(): "ready" | "unconfigured" | "browser-mode" {
  if (!isRunningInTauri()) return "browser-mode";
  if (!SUPABASE_ANON_KEY) return "unconfigured";
  return "ready";
}

/**
 * Storage adapter backing supabase-js onto the OS keychain.
 *
 * supabase-js defaults to `localStorage`, which in a webview means the access
 * token, the long-lived refresh token *and* the PKCE `code_verifier` sit in
 * plaintext on disk. Every key supabase-js writes is routed through the Rust
 * `account_session_*` commands instead — the verifier included, since leaving
 * it behind would defeat the PKCE protection the deep-link flow relies on.
 *
 * Failures are swallowed into "no value". A keychain the user has denied means
 * the session does not survive a restart; it must not break the running app.
 */
const keychainStorage: SupportedStorage = {
  async getItem(key: string): Promise<string | null> {
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      return (await invoke<string | null>("account_session_get", { key })) ?? null;
    } catch {
      return null;
    }
  },
  async setItem(key: string, value: string): Promise<void> {
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("account_session_set", { key, value });
    } catch {
      // Session will not survive a restart. Sign-in itself still succeeded, so
      // the user keeps working; `account.svelte.ts` surfaces the degradation.
    }
  },
  async removeItem(key: string): Promise<void> {
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("account_session_delete", { key });
    } catch {
      // Sign-out clears the in-memory session regardless.
    }
  },
};

let client: SupabaseClient | null = null;

/**
 * The shared client, or `null` when sign-in cannot work here.
 *
 * Callers branch on `null` instead of constructing a client that would fail on
 * first use. Built lazily so an unconfigured build pays nothing.
 */
export function getSupabase(): SupabaseClient | null {
  if (accountAvailability() !== "ready") return null;
  if (client) return client;

  client = createClient(SUPABASE_URL, SUPABASE_ANON_KEY, {
    auth: {
      flowType: "pkce",
      storage: keychainStorage,
      persistSession: true,
      autoRefreshToken: true,
      // There is no URL to inspect: the callback arrives as a deep-link event,
      // and `account-oauth.ts` performs the exchange explicitly.
      detectSessionInUrl: false,
    },
  });
  return client;
}
