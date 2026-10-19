import { openExternal } from "./external-links";
import { AUTH_REDIRECT_URL, getSurrealAuth } from "./surrealdb";

/**
 * The desktop half of the OAuth round-trip.
 *
 * ```
 * signInWithOAuth(provider)      → authorize URL (PKCE verifier stored in keychain/memory)
 *   → openExternal(url)          → consent in the system browser
 *   → colimaui://auth-callback?state=…&code=…
 *   → deep-link event            → state checked → exchangeCodeForSession(code)
 * ```
 */

export type OAuthProvider = "google" | "github";

const STATE_KEY = "colimaui.oauth.state";

let pendingState: string | null = null;
let lastCallbackError: string | null = null;

/** Read and clear the most recent callback error message, if any. */
export function consumeAuthCallbackError(): string | null {
  const err = lastCallbackError;
  lastCallbackError = null;
  return err;
}

async function keychain(command: string, args: Record<string, unknown>): Promise<string | null> {
  const { invoke } = await import("@tauri-apps/api/core");
  return (await invoke<string | null>(command, args)) ?? null;
}

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

function newState(): string {
  const bytes = new Uint8Array(16);
  crypto.getRandomValues(bytes);
  return Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join("");
}

/**
 * Begin OAuth sign-in: mint a `state`, ask the auth client for the authorize URL,
 * and hand it to the system browser.
 */
export async function startOAuth(provider: OAuthProvider): Promise<void> {
  const client = getSurrealAuth();
  if (!client) throw new Error("Account sign-in is not available in this build");

  lastCallbackError = null;
  const state = newState();
  pendingState = state;
  await keychainOptional("account_session_set", { key: STATE_KEY, value: state });

  const { data, error } = await client.auth.signInWithOAuth({
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

  const opened = await openExternal(data.url);
  if (!opened) {
    pendingState = null;
    await keychainOptional("account_session_delete", { key: STATE_KEY });
    throw new Error("Could not open system browser for sign-in");
  }
}

/**
 * Handle one `colimaui://auth-callback` URL.
 *
 * Resolves true when a session was established. Every rejection path clears the
 * pending `state` and records a user-facing reason in `lastCallbackError`.
 */
export async function handleAuthCallback(rawUrl: string): Promise<boolean> {
  const client = getSurrealAuth();
  if (!client) return false;

  let url: URL;
  try {
    url = new URL(rawUrl);
  } catch {
    return false;
  }
  if (url.protocol !== "colimaui:" || url.host !== "auth-callback") return false;

  const expected =
    pendingState ?? (await keychainOptional("account_session_get", { key: STATE_KEY }));
  const received = url.searchParams.get("state");
  pendingState = null;
  await keychainOptional("account_session_delete", { key: STATE_KEY });

  if (!expected || received !== expected) {
    lastCallbackError = "Authentication callback rejected: state mismatch.";
    console.warn("[account] auth callback rejected: state mismatch");
    return false;
  }

  const providerError =
    url.searchParams.get("error_description") ?? url.searchParams.get("error");
  if (providerError) {
    lastCallbackError = `Sign-in declined by provider: ${providerError}`;
    console.warn("[account] auth callback returned an error from the provider");
    return false;
  }

  const code = url.searchParams.get("code");
  if (!code) {
    lastCallbackError = "Authentication callback missing authorization code.";
    return false;
  }

  const { error } = await client.auth.exchangeCodeForSession(code);
  if (error) {
    lastCallbackError = error.message || "Authentication code exchange failed.";
    console.warn("[account] code exchange failed");
    return false;
  }

  lastCallbackError = null;
  return true;
}

/**
 * Subscribe to deep-link callbacks for the life of the app.
 */
export async function initAuthCallbackListener(
  onSettled: (ok: boolean, errorMessage?: string | null) => void,
): Promise<void> {
  if (!getSurrealAuth()) return;
  try {
    const { onOpenUrl, getCurrent } = await import("@tauri-apps/plugin-deep-link");

    const drain = async (urls: string[] | null) => {
      for (const url of urls ?? []) {
        const ok = await handleAuthCallback(url);
        onSettled(ok, consumeAuthCallbackError());
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
    // No deep-link plugin (browser mode, or an older build).
  }
}
