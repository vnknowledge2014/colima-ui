import type { User } from "@supabase/supabase-js";
import { initAuthCallbackListener, startOAuth, type OAuthProvider } from "./account-oauth";
import { accountAvailability, getSupabase } from "./supabase";
import { loadEntitlement, refreshEntitlement } from "./pro.svelte";

/**
 * Who is signed in — and nothing else.
 *
 * This store deliberately contains no license, Pro or entitlement state, and
 * imports nothing that does. `pro.svelte.ts` is the only thing that answers "is
 * this unlocked", from the cached Polar license. Keeping the two apart is what
 * guarantees that being signed out, offline, or hitting a Supabase outage cannot
 * take Pro away from someone who paid for it.
 */

/** The provider-supplied identity, flattened to what the UI actually renders. */
export interface AccountUser {
  /**
   * The Supabase user id. Not rendered anywhere, but load-bearing twice: it
   * keys the entitlement cache so one user cannot inherit another's Pro, and it
   * is the external customer id Polar assigns seats by.
   */
  id: string;
  name: string;
  email: string;
  /** Provider avatar URL, or empty — surfaces fall back to initials. */
  avatar: string;
  /** "google" | "github", for the "signed in with" line. */
  provider: string;
}

export const accountState = $state({
  user: null as AccountUser | null,
  /** True once the first session lookup has finished, so the badge can avoid flashing. */
  loaded: false,
  /** A sign-in round-trip is in flight; the panel disables its buttons. */
  busy: false,
  /**
   * Signed in, but the session could not reach the keychain — it will be gone
   * after a restart. Shown as a note on the profile, never as a blocking error.
   */
  ephemeral: false,
});

/** Why sign-in may be unavailable; the UI explains rather than showing dead buttons. */
export function accountStatus(): "ready" | "unconfigured" | "browser-mode" {
  return accountAvailability();
}

export function isSignedIn(): boolean {
  return accountState.user !== null;
}

/**
 * Flatten a Supabase user.
 *
 * `user_metadata` shape varies by provider — Google sends `full_name`, GitHub
 * `name` plus `user_name` — so each field falls back until something renderable
 * is found, ending at the email local-part. A signed-in user always has a name.
 */
function normalize(user: User): AccountUser {
  const meta = (user.user_metadata ?? {}) as Record<string, unknown>;
  const str = (key: string): string => (typeof meta[key] === "string" ? (meta[key] as string) : "");
  const email = user.email ?? "";
  return {
    id: user.id,
    name: str("full_name") || str("name") || str("user_name") || email.split("@")[0] || "Account",
    email,
    avatar: str("avatar_url") || str("picture"),
    provider: user.app_metadata?.provider ?? "",
  };
}

/**
 * Restore an existing session and start listening for OAuth callbacks.
 *
 * Called once at startup. Any failure resolves to "signed out" — the account is
 * optional, so a broken restore must never hold up the app.
 */
export async function loadSession(): Promise<void> {
  const supabase = getSupabase();
  if (!supabase) {
    accountState.loaded = true;
    return;
  }

  try {
    const { data } = await supabase.auth.getSession();
    accountState.user = data.session?.user ? normalize(data.session.user) : null;
  } catch {
    accountState.user = null;
  }
  accountState.loaded = true;

  // Keeps the badge honest across token refresh, sign-out in another window, and
  // the callback exchange. A failed refresh is deliberately not handled here:
  // supabase-js retries, and dropping the identity on a flaky network would make
  // the badge flicker for no reason.
  supabase.auth.onAuthStateChange((event, session) => {
    const previousId = accountState.user?.id ?? null;

    if (session?.user) {
      accountState.user = normalize(session.user);
    } else if (event === "SIGNED_OUT") {
      accountState.user = null;
    }

    // Who is signed in decides which cached entitlement applies, so a change of
    // user has to re-resolve it. Signing *out* is included on purpose: the
    // cached Pro keeps standing (it is not revoked), but the answer must be
    // recomputed rather than left as whatever the previous user's was.
    const currentId = accountState.user?.id ?? null;
    if (currentId !== previousId) {
      void loadEntitlement(currentId ?? undefined);
      // A newly signed-in user has no cache of their own yet; ask the oracle.
      if (currentId) void refreshEntitlement();
    }
  });

  // Fires once a callback has been dealt with, however it turned out. The
  // identity itself arrives through `onAuthStateChange` above, so this only ends
  // the "waiting on your browser" state.
  await initAuthCallbackListener(() => {
    accountState.busy = false;
  });
}

/**
 * Open the provider's consent screen.
 *
 * Resolves as soon as the browser is handed the URL — the session arrives later
 * via the deep-link callback, which clears `busy`.
 */
export async function signIn(provider: OAuthProvider): Promise<void> {
  if (accountState.busy) return;
  accountState.busy = true;
  try {
    await startOAuth(provider);
  } catch (e) {
    accountState.busy = false;
    throw e;
  }
}

/** Cancel a pending sign-in locally (the browser tab is the user's to close). */
export function cancelPendingSignIn(): void {
  accountState.busy = false;
}

/**
 * Sign out.
 *
 * Clears memory first so the UI updates even if the keychain delete fails; a
 * leftover keychain entry is re-validated (and rejected) on next launch anyway.
 */
export async function signOut(): Promise<void> {
  accountState.user = null;
  accountState.ephemeral = false;
  try {
    await getSupabase()?.auth.signOut();
  } catch {
    // Already signed out as far as this app is concerned.
  }
}

/**
 * Whether this build can persist a session at all.
 *
 * Used once after sign-in to set `ephemeral`, so the profile can say "you will
 * need to sign in again next launch" instead of silently forgetting.
 */
export async function checkSessionPersistence(): Promise<void> {
  if (accountStatus() !== "ready") return;
  try {
    const { invoke } = await import("@tauri-apps/api/core");
    accountState.ephemeral = !(await invoke<boolean>("account_session_available"));
  } catch {
    accountState.ephemeral = true;
  }
}
