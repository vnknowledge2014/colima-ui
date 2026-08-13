import { isRunningInTauri } from "../env";
import { toAppException } from "../errors";

export const isTauri = (): boolean => {
  return isRunningInTauri();
};

/** Default API origin. The server prefers this port but falls back — see `resolveApiBase`. */
export const API_BASE = "http://127.0.0.1:11420";

/**
 * The server binds the first free port in 11420-11429 (`api_server.rs`), but
 * the client assumed 11420. When the default port was taken by anything else,
 * browser mode simply failed to reach the API.
 *
 * Probe the same range against the unauthenticated `/api/health` endpoint and
 * remember the winner. The default port is tried first, so the common case
 * costs one request.
 */
const API_PORT_RANGE = Array.from({ length: 10 }, (_, i) => 11420 + i);

/**
 * Resolve with the first promise that fulfils; reject only when all reject.
 * Hand-rolled because the project targets ES2020 and `Promise.any` is ES2021.
 */
function firstFulfilled<T>(promises: Promise<T>[]): Promise<T> {
  return new Promise<T>((resolve, reject) => {
    let pending = promises.length;
    if (pending === 0) {
      reject(new Error("no candidates"));
      return;
    }
    for (const p of promises) {
      p.then(resolve, () => {
        pending -= 1;
        if (pending === 0) reject(new Error("all candidates failed"));
      });
    }
  });
}

let _resolvedBase = "";
let _resolving: Promise<string> | null = null;

export async function resolveApiBase(): Promise<string> {
  if (_resolvedBase) return _resolvedBase;
  // Collapse concurrent callers onto a single probe.
  if (_resolving) return _resolving;

  _resolving = (async () => {
    const probe = async (port: number): Promise<string> => {
      const base = `http://127.0.0.1:${port}`;
      const res = await fetch(`${base}/api/health`, { signal: AbortSignal.timeout(1500) });
      if (!res.ok) throw new Error(`port ${port} not healthy`);
      return base;
    };

    // Fast path: the server prefers 11420, so try it alone first.
    try {
      _resolvedBase = await probe(API_PORT_RANGE[0]);
      return _resolvedBase;
    } catch {
      // Fall through to scanning the rest.
    }

    try {
      // Concurrently, so a busy range costs one timeout rather than nine.
      _resolvedBase = await firstFulfilled(API_PORT_RANGE.slice(1).map(probe));
      return _resolvedBase;
    } catch {
      // Nothing answered. Return the default so callers surface a normal
      // connection error, but do NOT cache it — the server is often still
      // starting up, and caching would pin this page to the wrong port until
      // a reload.
      return API_BASE;
    }
  })();

  try {
    return await _resolving;
  } finally {
    _resolving = null;
  }
}

// Lazy-loaded Tauri invoke to avoid import errors in browser
let _invoke: ((cmd: string, args?: Record<string, unknown>) => Promise<unknown>) | null = null;

export async function getInvoke() {
  if (_invoke) return _invoke;
  try {
    const mod = await import("@tauri-apps/api/core");
    _invoke = mod.invoke;
    return _invoke;
  } catch {
    return null;
  }
}


// ===== Unified call function =====

export async function call<T>(
  tauriCmd: string,
  tauriArgs: Record<string, unknown> | undefined,
  httpMethod: "GET" | "POST",
  httpPath: string,
  httpParams?: Record<string, string>,
  httpBody?: unknown
): Promise<T> {
  if (isTauri()) {
    const invoke = await getInvoke();
    if (invoke) {
      try {
        return await (invoke(tauriCmd, tauriArgs) as Promise<T>);
      } catch (err) {
        // Single choke point: everything downstream sees an AppErrorException,
        // whichever transport produced the failure.
        throw toAppException(err);
      }
    }
  }

  // Browser mode: use HTTP API
  const base = await resolveApiBase();
  let url = `${base}${httpPath}`;
  if (httpParams) {
    const params = new URLSearchParams(httpParams);
    url += `?${params.toString()}`;
  }

  const token = await getApiToken();
  const headers: Record<string, string> = { "Content-Type": "application/json" };
  if (token) {
    headers["Authorization"] = `Bearer ${token}`;
  }

  const opts: RequestInit = {
    method: httpMethod,
    headers,
  };
  if (httpBody && httpMethod === "POST") {
    opts.body = JSON.stringify(httpBody);
  }

  const res = await fetch(url, opts);

  // The auth middleware rejects with a bare status and no body, so parsing it
  // as JSON throws "Unexpected end of JSON input" — a parse error where the
  // real problem is a missing credential. Browser mode reaches this whenever a
  // tab was opened without the token fragment, so it needs to say so.
  if (res.status === 401) {
    throw toAppException(
      token
        ? "Not authorized: the API rejected this token."
        : "Not authorized: no API token. Open browser mode through the desktop app so it can hand the token over.",
    );
  }

  const json = await res.json();

  if (!json.success) {
    // `json.error` is the same structured payload the Tauri path rejects with.
    throw toAppException(json.error ?? "API call failed");
  }
  return json.data as T;
}


// ===== API Token (for SSE/browser mode auth) =====

/**
 * Where the browser stashes the token handed to it by the desktop app.
 *
 * `sessionStorage`, not `localStorage`: the credential should die with the tab,
 * and it is regenerated on every app start anyway, so persisting it across
 * sessions would only keep a stale value around.
 */
const TOKEN_STORAGE_KEY = "colima-ui.api-token";

let _cachedToken = "";

/**
 * Read a token out of the URL fragment and immediately remove it.
 *
 * The desktop app opens browser mode as `…/#token=<token>`. A fragment is never
 * sent to a server, so unlike a query parameter it stays out of access logs and
 * out of `Referer` headers on any onward navigation.
 *
 * Stripping it from the address bar afterwards keeps it from being copied into
 * a bug report or shared as a link. `replaceState` leaves no history entry, so
 * Back does not resurrect it.
 */
function takeTokenFromFragment(): string {
  if (typeof window === "undefined") return "";
  const hash = window.location.hash;
  if (!hash) return "";

  // Parse before deciding, rather than substring-matching "token=". A fragment
  // carrying `access_token=` (an OAuth implicit flow, say) contains that
  // substring without being ours, and must be left alone for its real owner.
  const params = new URLSearchParams(hash.slice(1));
  const token = params.get("token") ?? "";
  if (!token) return "";

  // Persist before stripping. If storage throws after the fragment is already
  // gone, the credential exists only in memory and the next reload lands on a
  // tab that can never authenticate again.
  try {
    sessionStorage.setItem(TOKEN_STORAGE_KEY, token);
  } catch {
    // Private mode or storage disabled. In-memory carries this page load; a
    // reload will need a fresh URL from the app.
  }

  params.delete("token");
  const rest = params.toString();
  window.history.replaceState(null, "", window.location.pathname + window.location.search + (rest ? `#${rest}` : ""));

  return token;
}

/**
 * The HTTP API token.
 *
 * Two paths, because there are two kinds of client and only one of them is
 * inside the trust boundary:
 *
 * - **Desktop:** ask the backend over IPC. Unrelated local processes cannot.
 * - **Browser:** use what the app handed over in the URL fragment.
 *
 * There is deliberately no HTTP fallback. The endpoint that used to serve this
 * (`GET /api/auth/token`, unauthenticated) gave the whole API to any process on
 * the machine, and has been removed — see `src-tauri/src/auth.rs`.
 *
 * Returns `""` when no token is available, e.g. a browser tab opened by hand
 * rather than through the app. Callers surface the resulting 401 as a normal
 * connection failure.
 */
export async function getApiToken(): Promise<string> {
  if (_cachedToken) return _cachedToken;

  if (isTauri()) {
    const invoke = await getInvoke();
    if (invoke) {
      try {
        _cachedToken = (await invoke("api_token")) as string;
        return _cachedToken;
      } catch {
        // Backend older than this frontend, or IPC unavailable. There is no
        // second way in — fall through and report "no token".
      }
    }
    return "";
  }

  const fromFragment = takeTokenFromFragment();
  if (fromFragment) {
    _cachedToken = fromFragment;
    return _cachedToken;
  }

  try {
    _cachedToken = sessionStorage.getItem(TOKEN_STORAGE_KEY) ?? "";
  } catch {
    _cachedToken = "";
  }
  return _cachedToken;
}
