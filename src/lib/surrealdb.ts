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

interface JwtClaims {
  ID?: string;
  id?: string;
  sub?: string;
  exp?: number;
}

function decodeJwtClaims(token: string): JwtClaims | null {
  try {
    const parts = token.split(".");
    if (parts.length !== 3) return null;
    const base64 = parts[1].replace(/-/g, "+").replace(/_/g, "/");
    const padded = base64.padEnd(base64.length + ((4 - (base64.length % 4)) % 4), "=");
    return JSON.parse(atob(padded)) as JwtClaims;
  } catch {
    return null;
  }
}

/** Decode the `ID` or `sub` claim from a SurrealDB JWT when `/sql` `$auth` is empty. */
function decodeJwtRecordId(token: string): string | null {
  const payload = decodeJwtClaims(token);
  return payload?.ID ?? payload?.id ?? payload?.sub ?? null;
}

/** True when a JWT carries a numeric `exp` claim that is at or before `nowMs`. */
export function isJwtExpired(token: string, nowMs = Date.now()): boolean {
  const payload = decodeJwtClaims(token);
  if (!payload || typeof payload.exp !== "number") return false;
  return payload.exp * 1000 <= nowMs;
}

export class SurrealHttpError extends Error {
  readonly status: number;

  constructor(status: number, message: string) {
    super(message);
    this.name = "SurrealHttpError";
    this.status = status;
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
    throw new SurrealHttpError(res.status, text || `SurrealDB HTTP ${res.status}`);
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

const SHA256_K = new Uint32Array([
  0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
  0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
  0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
  0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
  0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
  0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
  0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
  0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
]);

function rotr(x: number, n: number): number {
  return (x >>> n) | (x << (32 - n));
}

function sha256Fallback(data: Uint8Array): Uint8Array {
  const h = new Uint32Array([
    0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a,
    0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
  ]);
  const bitLen = data.length * 8;
  const totalLen = (((data.length + 9 + 63) >> 6) << 6);
  const padded = new Uint8Array(totalLen);
  padded.set(data);
  padded[data.length] = 0x80;
  const view = new DataView(padded.buffer);
  view.setUint32(totalLen - 4, bitLen >>> 0, false);

  const w = new Uint32Array(64);
  for (let offset = 0; offset < totalLen; offset += 64) {
    for (let i = 0; i < 16; i++) {
      w[i] = view.getUint32(offset + i * 4, false);
    }
    for (let i = 16; i < 64; i++) {
      const s0 = rotr(w[i - 15], 7) ^ rotr(w[i - 15], 18) ^ (w[i - 15] >>> 3);
      const s1 = rotr(w[i - 2], 17) ^ rotr(w[i - 2], 19) ^ (w[i - 2] >>> 10);
      w[i] = (w[i - 16] + s0 + w[i - 7] + s1) >>> 0;
    }
    let [a, b, c, d, e, f, g, hh] = h;
    for (let i = 0; i < 64; i++) {
      const S1 = rotr(e, 6) ^ rotr(e, 11) ^ rotr(e, 25);
      const ch = (e & f) ^ (~e & g);
      const temp1 = (hh + S1 + ch + SHA256_K[i] + w[i]) >>> 0;
      const S0 = rotr(a, 2) ^ rotr(a, 13) ^ rotr(a, 22);
      const maj = (a & b) ^ (a & c) ^ (b & c);
      const temp2 = (S0 + maj) >>> 0;
      hh = g;
      g = f;
      f = e;
      e = (d + temp1) >>> 0;
      d = c;
      c = b;
      b = a;
      a = (temp1 + temp2) >>> 0;
    }
    h[0] = (h[0] + a) >>> 0;
    h[1] = (h[1] + b) >>> 0;
    h[2] = (h[2] + c) >>> 0;
    h[3] = (h[3] + d) >>> 0;
    h[4] = (h[4] + e) >>> 0;
    h[5] = (h[5] + f) >>> 0;
    h[6] = (h[6] + g) >>> 0;
    h[7] = (h[7] + hh) >>> 0;
  }

  const out = new Uint8Array(32);
  const outView = new DataView(out.buffer);
  for (let i = 0; i < 8; i++) {
    outView.setUint32(i * 4, h[i], false);
  }
  return out;
}

async function computePkceChallenge(verifier: string): Promise<string> {
  const input = new TextEncoder().encode(verifier);
  let digestBytes: Uint8Array;
  if (typeof globalThis.crypto !== "undefined" && globalThis.crypto.subtle?.digest) {
    const buf = await globalThis.crypto.subtle.digest("SHA-256", input);
    digestBytes = new Uint8Array(buf);
  } else {
    digestBytes = sha256Fallback(input);
  }
  return btoa(String.fromCharCode(...digestBytes))
    .replace(/\+/g, "-")
    .replace(/\//g, "_")
    .replace(/=+$/, "");
}

const surrealClient: SurrealAuthClient = {
  auth: {
    async getSession() {
      const saved = await readPersistedSession();
      if (!saved) {
        return { data: { session: null }, error: null };
      }

      if (isJwtExpired(saved.access_token)) {
        await keychainStorage.removeItem(SESSION_STORAGE_KEY);
        notifyAuthListeners("SIGNED_OUT", null);
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
      } catch (e) {
        if (e instanceof SurrealHttpError && (e.status === 401 || e.status === 403)) {
          await keychainStorage.removeItem(SESSION_STORAGE_KEY);
          notifyAuthListeners("SIGNED_OUT", null);
          return { data: { session: null }, error: null };
        }
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

      const challenge = await computePkceChallenge(verifier);

      const redirectTo = options?.redirectTo ?? AUTH_REDIRECT_URL;
      const params = new URLSearchParams({
        provider,
        redirect_to: redirectTo,
        ns: SURREALDB_NS,
        db: SURREALDB_DB,
        ac: SURREALDB_ACCESS,
        code_challenge: challenge,
        code_challenge_method: "S256",
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
