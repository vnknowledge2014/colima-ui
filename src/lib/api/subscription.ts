import { SURREALDB_URL, surrealQuery } from "../surrealdb";
import { call } from "./client";

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

  clear: () =>
    call<SubscriptionState>("subscription_clear", undefined, "POST", "/api/subscription/clear"),
};

/**
 * Ask the backend for a checkout URL bound to the signed-in account.
 */
export async function createCheckout(
  accessToken: string,
  tier: "pro" | "pro_teams",
  seats?: number,
): Promise<string | null> {
  try {
    const res = await fetch(`${SURREALDB_URL}/functions/v1/checkout`, {
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

/**
 * Query SurrealDB (`subscription` table in `colima_ui`/`auth`) for the signed-in
 * user's entitlement record, with fallback to an HTTP `/functions/v1/entitlement`
 * oracle endpoint when present.
 *
 * Returns `null` when unreachable or when no subscription record exists yet, so
 * the caller leaves the existing offline cache untouched.
 */
export async function fetchEntitlement(accessToken: string): Promise<EntitlementPayload | null> {
  // 1. Primary path: query SurrealDB `subscription` table scoped by $auth.id
  try {
    const rows = await surrealQuery<EntitlementPayload[]>(
      "SELECT user_id, entitled, tier, seats_total, seats_claimed, expires_at FROM subscription WHERE user_id = <string>$auth.id LIMIT 1;",
      accessToken,
    );
    const record = rows[0]?.result?.[0];
    if (record && typeof record.entitled === "boolean" && typeof record.user_id === "string") {
      return {
        user_id: record.user_id,
        entitled: record.entitled,
        tier: record.tier || "pro",
        seats_total: record.seats_total ?? null,
        seats_claimed: record.seats_claimed ?? null,
        expires_at: record.expires_at ?? null,
      };
    }
  } catch {
    // Fall through to HTTP oracle fallback or offline no-op
  }

  // 2. Optional HTTP oracle fallback
  try {
    const res = await fetch(`${SURREALDB_URL}/functions/v1/entitlement`, {
      method: "POST",
      headers: {
        Authorization: `Bearer ${accessToken}`,
        "Content-Type": "application/json",
      },
    });
    if (!res.ok) return null;
    const body = (await res.json()) as EntitlementPayload;
    return typeof body?.entitled === "boolean" && typeof body?.user_id === "string" ? body : null;
  } catch {
    return null;
  }
}
