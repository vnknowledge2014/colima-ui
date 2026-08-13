import { openExternal } from "./external-links";
import { AUTH_REDIRECT_URL, getSupabase } from "./supabase";

/**
 * The desktop half of the OAuth round-trip.
 *
 * ```
 * signInWithOAuth(provider)      → authorize URL (PKCE verifier stored in keychain)
 *   → openExternal(url)          → consent in the system browser
 *   → colimaui://auth-callback?state=…&code=…
 *   → deep-link event            → state checked → exchangeCodeForSession(code)
 * ```
 *
 * # The deep-link is untrusted transport
 *
 * Any macOS application may register the `colimaui://` scheme, so a callback can
 * be delivered by something that is not the browser we opened. Two things make
 * that harmless:
 *
 *  1. **PKCE** — the authorization code is worthless without the `code_verifier`,
 *     which never leaves this app's keychain entry.
 *  2. **`state`** — a nonce generated here, carried on the redirect URL and
 *     required to match on return. A callback with a missing or wrong `state` is
 *     dropped without ever reaching `exchangeCodeForSession`.
 *
 * Neither the code nor the verifier is ever logged.
 */

export type OAuthProvider = "google" | "github";

/**
 * Where the one-shot `state` nonce lives between the authorize request and the
 * callback. Same keychain service as the session itself, so a stolen callback
 * cannot be matched by reading a file.
 */
const STATE_KEY = "colimaui.oauth.state";

/**
 * In-memory copy of the pending `state`.
 *
 * The nonce only has to survive from "open the browser" to "the deep link comes
 * back", which happens inside one process — the app stays running while the
 * system browser is in front. Process memory is therefore the natural scope for
 * it, and the keychain write below is a bonus that lets the flow survive an app
 * restart mid-sign-in, not the mechanism it depends on.
 *
 * Treating the keychain as mandatory is what broke here: `account_session_set`
 * reports failure whenever the keychain is unavailable — every debug build now,
 * plus `COLIMAUI_NO_KEYCHAIN=1` and a denied prompt — and an unhandled throw at
 * that line made sign-in impossible rather than merely non-persistent.
 *
 * The security property is unchanged: `handleAuthCallback` still requires a
 * non-empty expected value that matches, so a callback nobody initiated is
 * rejected exactly as before.
 */
let pendingState: string | null = null;

async function keychain(command: string, args: Record<string, unknown>): Promise<string | null> {
  const { invoke } = await import("@tauri-apps/api/core");
  return (await invoke<string | null>(command, args)) ?? null;
}

/**
 * Keychain call whose failure is not fatal. Used for the `state` nonce, where
 * memory already holds the authoritative copy.
 */
async function keychainOptional(
  command: string,
  args: Record<string, unknown>,
): Promise<string | null> {
  try {
    return await keychain(command, args);
  } catch {
    return null;
  }
}

/** Cryptographically random nonce; `crypto` is available in the webview. */
function newState(): string {
  const bytes = new Uint8Array(16);
  crypto.getRandomValues(bytes);
  return Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join("");
}

/**
 * Begin sign-in: mint a `state`, ask Supabase for the authorize URL, and hand it
 * to the system browser.
 *
 * `skipBrowserRedirect` keeps the webview on the current page — navigating it to
 * the provider would trap the user in an unauthenticated app window and, for
 * Google, be refused outright as an embedded user-agent.
 */
export async function startOAuth(provider: OAuthProvider): Promise<void> {
  const supabase = getSupabase();
  if (!supabase) throw new Error("Account sign-in is not available in this build");

  const state = newState();
  // Held in memory before the browser opens — this is the copy the callback is
  // verified against, and it cannot fail. Persisting it as well is best-effort:
  // it buys survival across an app restart mid-sign-in, and its absence must not
  // stop the user signing in.
  pendingState = state;
  await keychainOptional("account_session_set", { key: STATE_KEY, value: state });

  const { data, error } = await supabase.auth.signInWithOAuth({
    provider,
    options: {
      redirectTo: `${AUTH_REDIRECT_URL}?state=${state}`,
      skipBrowserRedirect: true,
    },
  });
  if (error || !data?.url) {
    pendingState = null;
    await keychainOptional("account_session_delete", { key: STATE_KEY });
    throw new Error(error?.message ?? "Could not start sign-in");
  }

  await openExternal(data.url);
}

/**
 * Handle one `colimaui://auth-callback` URL.
 *
 * Resolves true when a session was established. Every rejection path clears the
 * pending `state` so a replayed callback cannot be retried against it.
 */
export async function handleAuthCallback(rawUrl: string): Promise<boolean> {
  const supabase = getSupabase();
  if (!supabase) return false;

  let url: URL;
  try {
    url = new URL(rawUrl);
  } catch {
    return false;
  }
  if (url.protocol !== "colimaui:" || url.host !== "auth-callback") return false;

  // Memory first — it is authoritative and always present for a flow this
  // process started. The keychain copy only answers when the app was restarted
  // between opening the browser and the callback arriving.
  const expected = pendingState ?? (await keychainOptional("account_session_get", { key: STATE_KEY }));
  const received = url.searchParams.get("state");
  // Single-use: consumed whether or not it matches, from both places.
  pendingState = null;
  await keychainOptional("account_session_delete", { key: STATE_KEY });

  if (!expected || received !== expected) {
    // A callback we did not initiate — a hijacked scheme, a stale window, or a
    // replay. Say nothing about the code it carried.
    console.warn("[account] auth callback rejected: state mismatch");
    return false;
  }

  // The provider declined (user cancelled, app not approved). Not an error worth
  // a dialog; the user simply stays signed out.
  if (url.searchParams.get("error")) {
    console.warn("[account] auth callback returned an error from the provider");
    return false;
  }

  const code = url.searchParams.get("code");
  if (!code) return false;

  const { error } = await supabase.auth.exchangeCodeForSession(code);
  if (error) {
    console.warn("[account] code exchange failed");
    return false;
  }
  return true;
}

/**
 * Subscribe to deep-link callbacks for the life of the app.
 *
 * Also drains any URL that launched the app: on macOS a cold start triggered by
 * the redirect delivers the URL before any listener exists, and without this the
 * sign-in would appear to hang.
 */
export async function initAuthCallbackListener(onSettled: () => void): Promise<void> {
  if (!getSupabase()) return;
  try {
    const { onOpenUrl, getCurrent } = await import("@tauri-apps/plugin-deep-link");

    const drain = async (urls: string[] | null) => {
      for (const url of urls ?? []) {
        // Fired on rejection too, not only success: a callback that fails its
        // state check or code exchange still ends the attempt, and leaving the
        // panel spinning on "finish in your browser" would be a lie.
        await handleAuthCallback(url);
        onSettled();
        // Consent ran in the system browser; bring the app window back to front.
        const { getCurrentWindow } = await import("@tauri-apps/api/window");
        const win = getCurrentWindow();
        await win.unminimize();
        await win.show();
        await win.setFocus();
      }
    };

    await onOpenUrl((urls) => void drain(urls));
    await drain(await getCurrent());
  } catch {
    // No deep-link plugin (browser mode, or an older build). Sign-in is already
    // reported unavailable there; nothing further to do.
  }
}
