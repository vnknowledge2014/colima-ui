import { call } from "./client";
import { SUPABASE_URL } from "../supabase";

/** Mirrors `SubscriptionState` in `src-tauri/src/subscription/mod.rs`. */
export interface SubscriptionState {
  /** The only field a gate may read. */
  entitled: boolean;
  /** Whether any record exists here for the current user. */
  known: boolean;
  /** "pro" | "pro_teams" | null. Display only — never branch a gate on it. */
  tier: string | null;
  seats_total: number | null;
  seats_claimed: number | null;
  expires_at: string | null;
  last_checked_at: string | null;
}

/** What the entitlement oracle returns, and what gets cached verbatim. */
export interface EntitlementPayload {
  user_id: string;
  entitled: boolean;
  tier: string;
  seats_total: number | null;
  seats_claimed: number | null;
  expires_at: string | null;
}

export const subscriptionApi = {
  /**
   * Read the cached entitlement. `userId` is the signed-in user, omitted when
   * signed out — the backend needs it to refuse handing one user's entitlement
   * to another.
   */
  state: (userId?: string) =>
    call<SubscriptionState>(
      "subscription_state",
      { userId },
      "GET",
      `/api/subscription${userId ? `?user_id=${encodeURIComponent(userId)}` : ""}`,
    ),

  store: (payload: EntitlementPayload) =>
    call<SubscriptionState>(
      "subscription_store",
      { payload },
      "POST",
      "/api/subscription/store",
      undefined,
      payload,
    ),

  clear: () => call<SubscriptionState>("subscription_clear", undefined, "POST", "/api/subscription/clear"),
};

/**
 * Ask the entitlement oracle whether the signed-in user holds a claimed seat.
 *
 * The lookup needs `POLAR_ACCESS_TOKEN`, an organization secret, so it cannot
 * happen in this app — a Supabase Edge Function holds it and this call carries
 * the user's JWT as proof of who is asking.
 *
 * Returns `null` when the answer could not be obtained. **That is not "not
 * entitled"** — the caller must leave the cache alone on `null`, or a flat tyre
 * on the network becomes a downgrade for a paying customer.
 */
/**
 * Ask the backend for a checkout URL bound to the signed-in account.
 *
 * A static Polar checkout link cannot carry the buyer's identity — its query
 * parameters do not include `customer_external_id`, and unknown parameters are
 * ignored silently. Only `POST /v1/checkouts/` accepts `external_customer_id`,
 * and that needs the organization token, so the session is created server-side.
 *
 * Without that binding a purchase is unattachable: Polar would know an email,
 * the app would ask about a Supabase user id, and the two would never meet.
 *
 * Returns `null` if a URL could not be obtained — the caller must say so rather
 * than opening a fallback link that would take money it cannot honour.
 */
export async function createCheckout(
  accessToken: string,
  tier: "pro" | "pro_teams",
  seats?: number,
): Promise<string | null> {
  try {
    const res = await fetch(`${SUPABASE_URL}/functions/v1/checkout`, {
      method: "POST",
      headers: {
        Authorization: `Bearer ${accessToken}`,
        "Content-Type": "application/json",
      },
      body: JSON.stringify({ tier, seats }),
    });
    if (!res.ok) return null;
    const body = (await res.json()) as { url?: string };
    return typeof body?.url === "string" ? body.url : null;
  } catch {
    return null;
  }
}

export async function fetchEntitlement(accessToken: string): Promise<EntitlementPayload | null> {
  try {
    const res = await fetch(`${SUPABASE_URL}/functions/v1/entitlement`, {
      method: "POST",
      headers: {
        // Identity comes from this token alone. The function must never read a
        // user id out of the body, and this call deliberately sends none.
        Authorization: `Bearer ${accessToken}`,
        "Content-Type": "application/json",
      },
    });
    if (!res.ok) return null;
    const body = (await res.json()) as EntitlementPayload;
    return typeof body?.entitled === "boolean" && typeof body?.user_id === "string" ? body : null;
  } catch {
    // Offline, DNS failure, function not deployed yet. Indistinguishable here,
    // and all mean the same thing: we learned nothing, so change nothing.
    return null;
  }
}
