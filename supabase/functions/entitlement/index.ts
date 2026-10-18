/**
 * The entitlement oracle.
 *
 * Answers one question: **does the signed-in user hold a paid, active seat, and
 * until when?** It is the only server-side component in the product, and it
 * exists for one reason — reading Polar seat state needs `POLAR_ACCESS_TOKEN`,
 * an organization secret that cannot ship in a desktop app.
 *
 * It should not grow past that. Every addition re-opens the "no backend"
 * decision this project has now made twice.
 *
 * ```
 * app --(user JWT)--> here --(POLAR_ACCESS_TOKEN)--> Polar customer state
 *                       |
 *                       └-> { entitled, tier, seats, expires_at }
 *                              app caches it to ~/.colima-ui/subscription.json
 * ```
 *
 * # Two failure directions, and they are not symmetric
 *
 * Wrongly saying "entitled" gives away a $6 feature. Wrongly saying "not
 * entitled" takes Pro from someone who paid — and does it silently, on a
 * machine the customer is trying to work on. This function is written to avoid
 * the second at the cost of occasionally risking the first: anything that is
 * not a *definite* "no" from Polar returns `unknown`, which the client treats
 * as "change nothing".
 */

import { getAuthedUser, json, preflight } from "../_shared/auth.ts";
import { type CustomerStateSubscription, getCustomerStateByExternalId } from "../_shared/polar.ts";

/**
 * How long the client may honour a cached answer offline.
 *
 * This is also the enforcement lag: a cancelled subscriber, a removed team
 * member, or a shared machine whose owner signed out all keep Pro for up to
 * this long. Shortening it tightens enforcement and punishes anyone who works
 * offline; lengthening it does the reverse.
 */
const OFFLINE_GRACE_DAYS = Number(Deno.env.get("OFFLINE_GRACE_DAYS") ?? "30");

/** Statuses Polar reports for a subscription that is genuinely paid up. */
const LIVE_STATUSES = new Set(["active", "trialing"]);

interface EntitlementResponse {
  user_id: string;
  entitled: boolean;
  tier: string;
  seats_total: number | null;
  seats_claimed: number | null;
  expires_at: string | null;
}

Deno.serve(async (req: Request) => {
  // Answered before anything else: the `Authorization` header makes this a
  // preflighted request, so an unanswered OPTIONS means the real call never
  // arrives.
  const pre = preflight(req);
  if (pre) return pre;

  if (req.method !== "POST" && req.method !== "GET") {
    return json({ error: "method_not_allowed" }, 405);
  }

  // Identity comes from the verified token and nowhere else. Note that the
  // request body is never read: there is deliberately nothing in it to trust.
  const user = await getAuthedUser(req);
  if (!user) return json({ error: "unauthorized" }, 401);

  const result = await getCustomerStateByExternalId(user.id);

  if (result.kind === "unreachable") {
    // 503 with a distinct code so the client can tell "we could not ask" from
    // "the answer is no". The client's contract is to leave its cache alone.
    console.warn(`[entitlement] upstream unavailable: ${result.reason}`);
    return json({ error: "upstream_unavailable", entitled: null }, 503);
  }

  if (result.kind === "not_found") {
    // Polar has never seen this user: a definite no, safe to cache.
    return json(deny(user.id));
  }

  const live = result.state.active_subscriptions.filter((s) => LIVE_STATUSES.has(s.status));
  if (live.length === 0) return json(deny(user.id));

  // A member holding a claimed seat receives the benefit grant; the billing
  // manager who never claimed one does not. Prefer the subscription that
  // actually granted this user something, and fall back to the longest-running
  // live subscription so a payer is not denied over a benefit-config detail.
  const grantedSubscriptionIds = new Set(
    result.state.granted_benefits.map((g) => g.subscription_id).filter(Boolean),
  );
  const granted = live.filter((s) => grantedSubscriptionIds.has(s.id));
  const chosen = pickLatest(granted.length > 0 ? granted : live);

  const periodEnd = chosen.current_period_end ?? chosen.ends_at ?? null;
  const seats = chosen.seats ?? chosen.quantity ?? null;

  const response: EntitlementResponse = {
    user_id: user.id,
    entitled: true,
    // Tier is display-only — no gate branches on it — so deriving it from seat
    // count is good enough and needs no product-id configuration.
    tier: seats !== null && seats > 1 ? "pro_teams" : "pro",
    seats_total: seats,
    // Polar's customer state does not expose how many seats are claimed; the
    // billing portal does. Reported as unknown rather than guessed, and the UI
    // hides the counter when it is null.
    seats_claimed: null,
    expires_at: cacheExpiry(periodEnd),
  };

  return json(response);
});

function deny(userId: string): EntitlementResponse {
  return {
    user_id: userId,
    entitled: false,
    tier: "pro",
    seats_total: null,
    seats_claimed: null,
    // A definite "no" still carries an expiry so the client caches it rather
    // than asking on every launch; being wrong here only costs a re-check.
    expires_at: addDays(new Date(), 1).toISOString(),
  };
}

/**
 * When the client must stop trusting this answer.
 *
 * The earlier of the paid period ending and the offline grace window. Never
 * beyond what was actually paid for, and never so far out that a cancellation
 * goes unnoticed for months.
 */
function cacheExpiry(periodEnd: string | null): string {
  const graceLimit = addDays(new Date(), OFFLINE_GRACE_DAYS);
  if (!periodEnd) return graceLimit.toISOString();

  const end = new Date(periodEnd);
  // An unparseable date from upstream must not become an eternal entitlement.
  if (Number.isNaN(end.getTime())) return graceLimit.toISOString();

  return (end < graceLimit ? end : graceLimit).toISOString();
}

function pickLatest(subs: CustomerStateSubscription[]): CustomerStateSubscription {
  return subs.reduce((best, s) => {
    const a = Date.parse(s.current_period_end ?? "") || 0;
    const b = Date.parse(best.current_period_end ?? "") || 0;
    return a > b ? s : best;
  }, subs[0]);
}

function addDays(from: Date, days: number): Date {
  const d = new Date(from);
  d.setDate(d.getDate() + days);
  return d;
}
