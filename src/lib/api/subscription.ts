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
