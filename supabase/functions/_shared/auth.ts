/**
 * Caller identity for every Edge Function.
 *
 * There is exactly one rule here, and the whole security model rests on it:
 * **the user id comes from the verified token, never from the request body.**
 * A function that reads `{ user_id }` out of JSON and acts on it lets any
 * signed-in user impersonate any other, and the mistake is invisible in review
 * because the code looks reasonable.
 *
 * Verification is delegated to Supabase rather than hand-rolled: `getUser`
 * checks the signature, expiry and issuer against the project's own keys. A JWT
 * library plus a hardcoded secret would be more code and more ways to be subtly
 * wrong.
 */

import { createClient } from "npm:@supabase/supabase-js@2";

export interface AuthedUser {
  id: string;
  email: string | null;
}

/**
 * Verify the `Authorization: Bearer <jwt>` header.
 *
 * Returns `null` for anything less than a valid, current token — missing
 * header, wrong scheme, expired, forged. Callers treat `null` as 401 and must
 * not fall back to any other notion of who is calling.
 */
export async function getAuthedUser(req: Request): Promise<AuthedUser | null> {
  const header = req.headers.get("Authorization") ?? "";
  const [scheme, token] = header.split(" ");
  if (scheme?.toLowerCase() !== "bearer" || !token) return null;

  const url = Deno.env.get("SUPABASE_URL");
  const anonKey = Deno.env.get("SUPABASE_ANON_KEY");
  if (!url || !anonKey) {
    // Misconfiguration must fail closed. Treating it as "cannot verify, so
    // allow" would turn a deploy mistake into an open endpoint.
    console.error("[auth] SUPABASE_URL or SUPABASE_ANON_KEY missing");
    return null;
  }

  try {
    const supabase = createClient(url, anonKey, {
      auth: { persistSession: false, autoRefreshToken: false },
    });
    const { data, error } = await supabase.auth.getUser(token);
    if (error || !data?.user) return null;
    return { id: data.user.id, email: data.user.email ?? null };
  } catch (e) {
    console.error("[auth] verification failed:", e instanceof Error ? e.message : "unknown");
    return null;
  }
}

/**
 * CORS headers.
 *
 * The caller is a Tauri webview, whose origin is `tauri://localhost` (or
 * `http://localhost` in dev) rather than a normal site. Sending an
 * `Authorization` header makes the request preflighted, so without these — and
 * without answering `OPTIONS` — every call fails before it is ever received.
 */
export const CORS_HEADERS: Record<string, string> = {
  "Access-Control-Allow-Origin": "*",
  "Access-Control-Allow-Headers": "authorization, content-type",
  "Access-Control-Allow-Methods": "POST, GET, OPTIONS",
};

/**
 * `*` is deliberate and safe here: the function returns nothing without a valid
 * bearer token, and tokens are not sent automatically by browsers the way
 * cookies are. There is no ambient authority for another origin to abuse.
 */
export function preflight(req: Request): Response | null {
  return req.method === "OPTIONS"
    ? new Response(null, { status: 204, headers: CORS_HEADERS })
    : null;
}

/** JSON response helper. */
export function json(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { ...CORS_HEADERS, "Content-Type": "application/json" },
  });
}
