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
  const json = await res.json();
  
  if (!json.success) {
    // `json.error` is the same structured payload the Tauri path rejects with.
    throw toAppException(json.error ?? "API call failed");
  }
  return json.data as T;
}


// ===== API Token (for SSE/browser mode auth) =====

let _cachedToken = "";

export async function getApiToken(): Promise<string> {
  if (_cachedToken) return _cachedToken;
  if (isTauri()) {
    const invoke = await getInvoke();
    if (invoke) {
      try {
        _cachedToken = (await invoke("get_platform")) as string;
        // Platform returns object, not token — try fetching from HTTP
      } catch { /* fall through */ }
    }
  }
  // Fetch token from the public auth endpoint
  try {
    const res = await fetch(`${await resolveApiBase()}/api/auth/token`);
    const json = await res.json();
    // API returns { success: true, data: "token..." }
    const token = json.token || json.data;
    if (token) {
      _cachedToken = token;
      return _cachedToken;
    }
  } catch { /* ignore */ }
  return _cachedToken;
}
