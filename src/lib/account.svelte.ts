import { initAuthCallbackListener, startOAuth, type OAuthProvider } from "./account-oauth";
import { loadEntitlement, refreshEntitlement } from "./pro.svelte";
import { type AuthUser, accountAvailability, getSurrealAuth } from "./surrealdb";

/**
 * Who is signed in — backed by SurrealDB (`colima_ui` / `auth`).
 *
 * This store deliberately contains no license, Pro or entitlement state, and
 * imports nothing that gates features. `pro.svelte.ts` is the only thing that
 * answers "is this unlocked", from the cached entitlement. Keeping the two apart
 * guarantees that being signed out, offline, or hitting a database outage cannot
 * take Pro away from someone who paid for it.
 */

/** The identity flattened to what the UI renders. */
export interface AccountUser {
  /**
   * The SurrealDB user record id (e.g. `user:...`). Keys the entitlement cache
   * so one user cannot inherit another's Pro, and acts as the external customer
   * id Polar assigns seats by.
   */
  id: string;
  name: string;
  email: string;
  /** Avatar URL, or empty — surfaces fall back to initials. */
  avatar: string;
  /** "surrealdb" | "google" | "github", for the "signed in with" line. */
  provider: string;
}

export const accountState = $state({
  user: null as AccountUser | null,
  /** True once the first session lookup has finished, so the badge can avoid flashing. */
  loaded: false,
  /** A sign-in round-trip is in flight; the panel disables its buttons. */
  busy: false,
  /** Latest sign-in or callback error message, if any. */
  error: "",
  /**
   * Signed in, but the session could not reach the OS keychain — it will be gone
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
 * Flatten an AuthUser into an AccountUser.
 */
function normalize(user: AuthUser): AccountUser {
  const meta = (user.user_metadata ?? {}) as Record<string, unknown>;
  const str = (key: string): string => (typeof meta[key] === "string" ? (meta[key] as string) : "");
  const email = user.email ?? "";
  return {
    id: user.id,
    name: str("full_name") || str("name") || str("user_name") || email.split("@")[0] || "Account",
    email,
    avatar: str("avatar_url") || str("picture"),
    provider: user.app_metadata?.provider ?? "surrealdb",
  };
}

/**
 * Restore an existing session from the OS keychain / memory store and start
 * listening for OAuth callbacks.
 */
export async function loadSession(): Promise<void> {
  const client = getSurrealAuth();
  if (!client) {
    accountState.loaded = true;
    return;
  }

  try {
    const { data } = await client.auth.getSession();
    accountState.user = data.session?.user ? normalize(data.session.user) : null;
  } catch {
    accountState.user = null;
  }
  accountState.loaded = true;

  client.auth.onAuthStateChange((event, session) => {
    const previousId = accountState.user?.id ?? null;

    if (session?.user) {
      accountState.user = normalize(session.user);
      accountState.error = "";
    } else if (event === "SIGNED_OUT") {
      accountState.user = null;
    }

    const currentId = accountState.user?.id ?? null;
    if (currentId !== previousId) {
      void loadEntitlement(currentId ?? undefined);
      if (currentId) {
        void refreshEntitlement();
        void checkSessionPersistence();
      }
    }
  });

  await initAuthCallbackListener((ok, errorMessage) => {
    accountState.busy = false;
    if (!ok && errorMessage) {
      accountState.error = errorMessage;
    }
  });
}

/**
 * Sign in with Email & Password against SurrealDB (`DEFINE ACCESS account`).
 */
export async function signInWithCredentials(email: string, password: string): Promise<void> {
  if (accountState.busy) return;
  const client = getSurrealAuth();
  if (!client) throw new Error("Account sign-in is not available in this build");

  accountState.busy = true;
  accountState.error = "";
  try {
    const { error } = await client.auth.signInWithPassword({ email, password });
    if (error) {
      accountState.error = error.message;
      throw error;
    }
  } finally {
    accountState.busy = false;
  }
}

/**
 * Create a new account in SurrealDB (`DEFINE ACCESS account`) and sign in immediately.
 */
export async function signUpWithCredentials(
  email: string,
  password: string,
  name?: string,
): Promise<void> {
  if (accountState.busy) return;
  const client = getSurrealAuth();
  if (!client) throw new Error("Account sign-up is not available in this build");

  accountState.busy = true;
  accountState.error = "";
  try {
    const { error } = await client.auth.signUp({ email, password, name });
    if (error) {
      accountState.error = error.message;
      throw error;
    }
  } finally {
    accountState.busy = false;
  }
}

/**
 * Open an external OAuth provider's consent screen.
 */
export async function signIn(provider: OAuthProvider): Promise<void> {
  if (accountState.busy) return;
  accountState.busy = true;
  accountState.error = "";
  try {
    await startOAuth(provider);
  } catch (e) {
    accountState.busy = false;
    accountState.error = e instanceof Error ? e.message : String(e);
    throw e;
  }
}

/** Cancel a pending sign-in locally. */
export function cancelPendingSignIn(): void {
  accountState.busy = false;
  accountState.error = "";
}

/**
 * Sign out.
 */
export async function signOut(): Promise<void> {
  accountState.ephemeral = false;
  accountState.error = "";
  try {
    await getSurrealAuth()?.auth.signOut();
  } catch {
    // Fall through to ensure local state is cleared.
  } finally {
    if (accountState.user !== null) {
      accountState.user = null;
      void loadEntitlement(undefined);
    }
  }
}

/**
 * Whether this build can persist a session in the OS keychain.
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
