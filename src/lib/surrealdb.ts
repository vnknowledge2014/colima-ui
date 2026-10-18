import { isRunningInTauri } from "./env";

/**
 * SurrealDB Auth & Entitlement Client.
 *
 * Replaces the legacy Supabase client with a lightweight, zero-dependency
 * SurrealDB HTTP REST/RPC client (`/signup`, `/signin`, `/sql`) backed by the
 * `colima_ui` namespace and `auth` database (`surrealdb/schema.surql`).
 *
 * # Security & Storage Model
 *
 * - Sessions and PKCE verifiers are stored via `keychainStorage`, which routes
 *   reads/writes to the macOS Keychain (`account_session_*` Tauri commands) and
 *   maintains an in-memory `Map` fallback (`memoryStorage`).
 * - When the OS Keychain is disabled (debug builds, `COLIMAUI_NO_KEYCHAIN=1`,
 *   Linux/Windows, or browser mode), the session lives in process memory only —
 *   never in `localStorage`.
 * - Being offline or unreachable never downgrades a cached Pro entitlement or
 *   clears an already-authenticated session.
 */

interface ImportMetaEnvLike {
  VITE_SURREALDB_URL?: string;
  VITE_SURREALDB_NS?: string;
  VITE_SURREALDB_DB?: string;
  VITE_SURREALDB_ACCESS?: string;
  VITE_OAUTH_ENABLED?: string;
}

function readEnv(key: keyof ImportMetaEnvLike, fallback: string): string {
  try {
    const env = (import.meta as unknown as { env?: ImportMetaEnvLike }).env;
    const val = env?.[key];
    if (typeof val === "string" && val.trim() !== "") {
      return val.trim();
    }
  } catch {
    // Ignore in environments without import.meta.env
  }
  return fallback;
}

/** Public SurrealDB HTTP endpoint (defaults to local Colima SurrealDB container). */
export const SURREALDB_URL = readEnv("VITE_SURREALDB_URL", "http://127.0.0.1:18000").replace(
  /\/+$/,
  "",
);

/** SurrealDB namespace for ColimaUI identity and entitlement records. */
export const SURREALDB_NS = readEnv("VITE_SURREALDB_NS", "colima_ui");

/** SurrealDB database for ColimaUI identity and entitlement records. */
export const SURREALDB_DB = readEnv("VITE_SURREALDB_DB", "auth");

/** SurrealDB RECORD access method name (`DEFINE ACCESS account ON DATABASE TYPE RECORD`). */
export const SURREALDB_ACCESS = readEnv("VITE_SURREALDB_ACCESS", "account");

/** Whether external OAuth deep-link providers are enabled alongside SurrealDB auth. */
export const OAUTH_ENABLED = readEnv("VITE_OAUTH_ENABLED", "") === "true";

/** The OAuth return address for deep-link flows (`colimaui://auth-callback`). */
export const AUTH_REDIRECT_URL = "colimaui://auth-callback";

/** Storage key for the serialized SurrealDB session. */
export const SESSION_STORAGE_KEY = "colimaui.surreal.session";

/** Storage key for the PKCE code verifier during deep-link OAuth flows. */
export const PKCE_VERIFIER_KEY = "colimaui.oauth.code_verifier";

export interface StorageAdapter {
  getItem(key: string): Promise<string | null>;
  setItem(key: string, value: string): Promise<void>;
  removeItem(key: string): Promise<void>;
}

/**
 * In-memory session and PKCE verifier store.
 *
 * Ensures that debug builds (`cfg!(debug_assertions)`), `COLIMAUI_NO_KEYCHAIN=1`,
 * non-macOS systems, and browser mode retain their session and PKCE verifier
 * across the lifetime of the running process without touching `localStorage`.
 */
const memoryStorage = new Map<string, string>();

/** Clear the in-memory storage fallback (used by tests and sign-out). */
export function clearMemoryStorage(): void {
  memoryStorage.clear();
}

/**
 * Storage adapter backing sessions onto the OS keychain with an in-memory fallback.
 */
export const keychainStorage: StorageAdapter = {
  async getItem(key: string): Promise<string | null> {
    if (isRunningInTauri()) {
      try {
        const { invoke } = await import("@tauri-apps/api/core");
        const stored = await invoke<string | null>("account_session_get", { key });
        if (typeof stored === "string") {
          memoryStorage.set(key, stored);
          return stored;
        }
      } catch {
        // Fall through to in-memory storage when Keychain is disabled or denied.
      }
    }
    return memoryStorage.get(key) ?? null;
  },

  async setItem(key: string, value: string): Promise<void> {
    memoryStorage.set(key, value);
    if (isRunningInTauri()) {
      try {
        const { invoke } = await import("@tauri-apps/api/core");
        await invoke("account_session_set", { key, value });
      } catch {
        // Session will not survive a full process restart, but remains in
        // memoryStorage so the active session and PKCE exchange succeed.
      }
    }
  },

  async removeItem(key: string): Promise<void> {
    memoryStorage.delete(key);
    if (isRunningInTauri()) {
      try {
        const { invoke } = await import("@tauri-apps/api/core");
        await invoke("account_session_delete", { key });
      } catch {
        // Already removed from memoryStorage.
      }
    }
  },
};

/**
 * Whether sign-in is available in this build and configuration.
 *
 * With SurrealDB HTTP record authentication (`/signin` and `/signup`), sign-in
 * works in both the native Tauri desktop app and browser mode (using
 * `memoryStorage` when the OS Keychain is unavailable).
 */
export function accountAvailability(): "ready" | "unconfigured" | "browser-mode" {
  if (!SURREALDB_URL) return "unconfigured";
  return "ready";
}

export interface AuthUser {
  id: string;
  email?: string;
  user_metadata?: {
    full_name?: string;
    name?: string;
    user_name?: string;
    avatar_url?: string;
    picture?: string;
    [key: string]: unknown;
  };
  app_metadata?: {
    provider?: string;
    [key: string]: unknown;
  };
}

export interface AuthSession {
  access_token: string;
  user: AuthUser;
}

export type AuthChangeEvent = "SIGNED_IN" | "SIGNED_OUT" | "TOKEN_REFRESHED";
export type AuthStateChangeCallback = (
  event: AuthChangeEvent,
  session: AuthSession | null,
) => void;

interface SurrealSqlResponseItem<T = unknown> {
  status: "OK" | "ERR";
  time?: string;
  result?: T;
  detail?: string;
}

interface SurrealAuthRecord {
  id: string;
  email?: string;
  name?: string;
  avatar?: string;
  provider?: string;
}

interface SurrealTokenResponse {
  code?: number;
  details?: string;
  description?: string;
  information?: string;
  token?: string;
}

function recordToAuthUser(rec: SurrealAuthRecord, fallbackEmail = ""): AuthUser {
  const email = rec.email || fallbackEmail;
  const name = rec.name || (email ? email.split("@")[0] : "Account") || "Account";
  const provider = rec.provider || "surrealdb";
  return {
    id: String(rec.id),
    email,
    user_metadata: {
      full_name: name,
      name,
      avatar_url: rec.avatar || "",
    },
    app_metadata: {
      provider,
    },
  };
}

/** Decode the `ID` or `sub` claim from a SurrealDB JWT when `/sql` `$auth` is empty. */
function decodeJwtRecordId(token: string): string | null {
  try {
    const parts = token.split(".");
    if (parts.length !== 3) return null;
    const base64 = parts[1].replace(/-/g, "+").replace(/_/g, "/");
    const padded = base64.padEnd(base64.length + ((4 - (base64.length % 4)) % 4), "=");
    const payload = JSON.parse(atob(padded)) as { ID?: string; id?: string; sub?: string };
    return payload.ID ?? payload.id ?? payload.sub ?? null;
  } catch {
    return null;
  }
}

/**
 * Execute one or more SurrealQL statements against `SURREALDB_URL/sql`.
 */
export async function surrealQuery<T = unknown>(
  surql: string,
  token?: string,
): Promise<SurrealSqlResponseItem<T>[]> {
  const headers: Record<string, string> = {
    Accept: "application/json",
    "Content-Type": "text/plain",
    "surreal-ns": SURREALDB_NS,
    "surreal-db": SURREALDB_DB,
  };
  if (token) {
    headers.Authorization = `Bearer ${token}`;
  }

  const res = await fetch(`${SURREALDB_URL}/sql`, {
    method: "POST",
    headers,
    body: surql,
  });

  if (!res.ok) {
    const text = await res.text().catch(() => "");
    throw new Error(text || `SurrealDB HTTP ${res.status}`);
  }

  const data = (await res.json()) as SurrealSqlResponseItem<T>[];
  return Array.isArray(data) ? data : [];
}

/**
 * Fetch the authenticated user's profile from SurrealDB using their JWT.
 */
async function fetchCurrentUser(
  token: string,
  fallbackEmail = "",
  fallbackName = "",
): Promise<AuthUser> {
  try {
    const rows = await surrealQuery<SurrealAuthRecord[]>(
      "SELECT id, email, name, avatar, provider FROM $auth;",
      token,
    );
    const first = rows[0]?.result?.[0];
    if (first && first.id) {
      return recordToAuthUser(first, fallbackEmail);
    }
  } catch {
    // Fall back to JWT claims if $auth query fails
  }

  const jwtId = decodeJwtRecordId(token) ?? `user:${fallbackEmail || "local"}`;
  return recordToAuthUser(
    {
      id: jwtId,
      email: fallbackEmail,
      name: fallbackName || (fallbackEmail ? fallbackEmail.split("@")[0] : "Account"),
      provider: "surrealdb",
    },
    fallbackEmail,
  );
}

const authListeners = new Set<AuthStateChangeCallback>();

function notifyAuthListeners(event: AuthChangeEvent, session: AuthSession | null): void {
  for (const listener of [...authListeners]) {
    try {
      listener(event, session);
    } catch {
      // Isolate listener errors
    }
  }
}

async function persistSession(session: AuthSession): Promise<void> {
  await keychainStorage.setItem(SESSION_STORAGE_KEY, JSON.stringify(session));
}

async function readPersistedSession(): Promise<AuthSession | null> {
  const raw = await keychainStorage.getItem(SESSION_STORAGE_KEY);
  if (!raw) return null;
  try {
    const parsed = JSON.parse(raw) as AuthSession;
    if (parsed && typeof parsed.access_token === "string" && parsed.user?.id) {
      return parsed;
    }
  } catch {
    // Malformed session entry
  }
  return null;
}

export interface SurrealAuthClient {
  auth: {
    getSession(): Promise<{ data: { session: AuthSession | null }; error: Error | null }>;
    signInWithPassword(credentials: {
      email: string;
      password: string;
    }): Promise<{ data: { session: AuthSession | null; user: AuthUser | null }; error: Error | null }>;
    signUp(credentials: {
      email: string;
      password: string;
      name?: string;
    }): Promise<{ data: { session: AuthSession | null; user: AuthUser | null }; error: Error | null }>;
    signInWithOAuth(params: {
      provider: string;
      options?: { redirectTo?: string; skipBrowserRedirect?: boolean };
    }): Promise<{ data: { url: string | null }; error: Error | null }>;
    exchangeCodeForSession(
      code: string,
    ): Promise<{ data: { session: AuthSession | null }; error: Error | null }>;
    signOut(): Promise<{ error: Error | null }>;
    onAuthStateChange(callback: AuthStateChangeCallback): {
      data: { subscription: { unsubscribe: () => void } };
    };
  };
}

function parseSurrealAuthError(status: number, body: SurrealTokenResponse | null): string {
  const detail = body?.information || body?.description || body?.details || "";
  if (status === 401 || status === 403 || detail.toLowerCase().includes("invalid")) {
    return "Invalid email or password, or account already exists.";
  }
  if (detail) return detail;
  return `SurrealDB authentication failed (HTTP ${status})`;
}

const surrealClient: SurrealAuthClient = {
  auth: {
    async getSession() {
      const saved = await readPersistedSession();
      if (!saved) {
        return { data: { session: null }, error: null };
      }

      // Opportunistically refresh user profile from SurrealDB when reachable,
      // but never drop a stored session on a network or offline error.
      try {
        const rows = await surrealQuery<SurrealAuthRecord[]>(
          "SELECT id, email, name, avatar, provider FROM $auth;",
          saved.access_token,
        );
        const record = rows[0]?.result?.[0];
        if (record && record.id) {
          const updated: AuthSession = {
            access_token: saved.access_token,
            user: recordToAuthUser(record, saved.user.email),
          };
          await persistSession(updated);
          return { data: { session: updated }, error: null };
        }
      } catch {
        // Offline or unreachable: honour cached identity.
      }

      return { data: { session: saved }, error: null };
    },

    async signInWithPassword({ email, password }) {
      const trimmedEmail = email.trim().toLowerCase();
      if (!trimmedEmail || !password) {
        return {
          data: { session: null, user: null },
          error: new Error("Email and password are required."),
        };
      }

      try {
        const res = await fetch(`${SURREALDB_URL}/signin`, {
          method: "POST",
          headers: {
            Accept: "application/json",
            "Content-Type": "application/json",
          },
          body: JSON.stringify({
            ns: SURREALDB_NS,
            db: SURREALDB_DB,
            ac: SURREALDB_ACCESS,
            email: trimmedEmail,
            password,
          }),
        });

        const body = (await res.json().catch(() => null)) as SurrealTokenResponse | null;
        if (!res.ok || !body?.token) {
          return {
            data: { session: null, user: null },
            error: new Error(parseSurrealAuthError(res.status, body)),
          };
        }

        const user = await fetchCurrentUser(body.token, trimmedEmail);
        const session: AuthSession = { access_token: body.token, user };
        await persistSession(session);
        notifyAuthListeners("SIGNED_IN", session);
        return { data: { session, user }, error: null };
      } catch (e) {
        const msg =
          e instanceof Error
            ? `Cannot connect to SurrealDB at ${SURREALDB_URL}: ${e.message}`
            : `Cannot connect to SurrealDB at ${SURREALDB_URL}`;
        return { data: { session: null, user: null }, error: new Error(msg) };
      }
    },

    async signUp({ email, password, name }) {
      const trimmedEmail = email.trim().toLowerCase();
      const trimmedName = (name ?? "").trim();
      if (!trimmedEmail || !password) {
        return {
          data: { session: null, user: null },
          error: new Error("Email and password are required."),
        };
      }

      try {
        const res = await fetch(`${SURREALDB_URL}/signup`, {
          method: "POST",
          headers: {
            Accept: "application/json",
            "Content-Type": "application/json",
          },
          body: JSON.stringify({
            ns: SURREALDB_NS,
            db: SURREALDB_DB,
            ac: SURREALDB_ACCESS,
            email: trimmedEmail,
            password,
            name: trimmedName || trimmedEmail.split("@")[0] || "Account",
          }),
        });

        const body = (await res.json().catch(() => null)) as SurrealTokenResponse | null;
        if (!res.ok || !body?.token) {
          return {
            data: { session: null, user: null },
            error: new Error(parseSurrealAuthError(res.status, body)),
          };
        }

        const user = await fetchCurrentUser(body.token, trimmedEmail, trimmedName);
        const session: AuthSession = { access_token: body.token, user };
        await persistSession(session);
        notifyAuthListeners("SIGNED_IN", session);
        return { data: { session, user }, error: null };
      } catch (e) {
        const msg =
          e instanceof Error
            ? `Cannot connect to SurrealDB at ${SURREALDB_URL}: ${e.message}`
            : `Cannot connect to SurrealDB at ${SURREALDB_URL}`;
        return { data: { session: null, user: null }, error: new Error(msg) };
      }
    },

    async signInWithOAuth({ provider, options }) {
      // Generate PKCE verifier and persist in keychainStorage (with memory fallback)
      const bytes = new Uint8Array(32);
      crypto.getRandomValues(bytes);
      const verifier = Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join("");
      await keychainStorage.setItem(PKCE_VERIFIER_KEY, verifier);

      const redirectTo = options?.redirectTo ?? AUTH_REDIRECT_URL;
      const params = new URLSearchParams({
        provider,
        redirect_to: redirectTo,
        ns: SURREALDB_NS,
        db: SURREALDB_DB,
        ac: SURREALDB_ACCESS,
      });
      const url = `${SURREALDB_URL}/oauth/authorize?${params.toString()}`;
      return { data: { url }, error: null };
    },

    async exchangeCodeForSession(code: string) {
      const verifier = await keychainStorage.getItem(PKCE_VERIFIER_KEY);
      await keychainStorage.removeItem(PKCE_VERIFIER_KEY);
      if (!verifier) {
        return {
          data: { session: null },
          error: new Error("PKCE code verifier not found in storage"),
        };
      }

      try {
        const res = await fetch(`${SURREALDB_URL}/oauth/token`, {
          method: "POST",
          headers: {
            Accept: "application/json",
            "Content-Type": "application/json",
          },
          body: JSON.stringify({
            ns: SURREALDB_NS,
            db: SURREALDB_DB,
            ac: SURREALDB_ACCESS,
            code,
            code_verifier: verifier,
          }),
        });

        const body = (await res.json().catch(() => null)) as SurrealTokenResponse | null;
        if (!res.ok || !body?.token) {
          return {
            data: { session: null },
            error: new Error(parseSurrealAuthError(res.status, body)),
          };
        }

        const user = await fetchCurrentUser(body.token);
        const session: AuthSession = { access_token: body.token, user };
        await persistSession(session);
        notifyAuthListeners("SIGNED_IN", session);
        return { data: { session }, error: null };
      } catch (e) {
        return {
          data: { session: null },
          error: e instanceof Error ? e : new Error(String(e)),
        };
      }
    },

    async signOut() {
      await keychainStorage.removeItem(SESSION_STORAGE_KEY);
      await keychainStorage.removeItem(PKCE_VERIFIER_KEY);
      notifyAuthListeners("SIGNED_OUT", null);
      return { error: null };
    },

    onAuthStateChange(callback: AuthStateChangeCallback) {
      authListeners.add(callback);
      return {
        data: {
          subscription: {
            unsubscribe: () => {
              authListeners.delete(callback);
            },
          },
        },
      };
    },
  },
};

export function getSurrealAuth(): SurrealAuthClient | null {
  if (accountAvailability() !== "ready") return null;
  return surrealClient;
}
