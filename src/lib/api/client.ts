// ===== Runtime Detection =====

export const isTauri = (): boolean => {
  return !!(window as any).__TAURI_INTERNALS__;
};

export const API_BASE = "http://127.0.0.1:11420";

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
      } catch (err: any) {
        if (err && typeof err === 'object' && err.type && err.message) {
          throw `[${err.type}] ${err.message}`;
        }
        throw err;
      }
    }
  }

  // Browser mode: use HTTP API
  let url = `${API_BASE}${httpPath}`;
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
    throw new Error(json.error || "API call failed");
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
    const res = await fetch(`${API_BASE}/api/auth/token`);
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
