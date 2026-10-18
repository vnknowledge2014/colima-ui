import { call } from "./api/client";
import { fetchEntitlement, subscriptionApi, type SubscriptionState } from "./api/subscription";
import { getSupabase } from "./supabase";

/**
 * Pro status as reported by the backend. Mirrors `ProStatus` in
 * `src-tauri/src/pro/mod.rs` — the `state` tag drives everything here.
 *
 * `free` is the default and the common case today: no sidecar is bundled yet,
 * so the whole app runs free with no gating.
 */
export type ProStatus =
  | { state: "free" }
  | { state: "active"; capabilities: string[] }
  | { state: "needs_update" }
  | { state: "license_inactive" };

export const proState = $state<{
  status: ProStatus;
  loaded: boolean;
  paid: boolean;
  /** Tier, seats and renewal date for the Subscription page. Display only. */
  subscription: SubscriptionState | null;
}>({
  status: { state: "free" },
  loaded: false,
  // Entitlement from the Polar seat subscription, independent of the sidecar.
  // A subscriber is a paying customer even before any sidecar exists — so the
  // UI must never show them the "buy Pro" upsell.
  paid: false,
  subscription: null,
});

/**
 * Ask the backend for Pro status once. Infallible on the backend side; if the
 * call itself fails (e.g. browser mode with no server), we stay on `free`
 * rather than surfacing an error, because free is the correct fallback.
 */
export async function loadProStatus(): Promise<void> {
  try {
    const status = await call<ProStatus>("pro_status", undefined, "GET", "/api/pro/status");
    if (status && typeof status.state === "string") {
      proState.status = status;
    }
  } catch {
    proState.status = { state: "free" };
  } finally {
    proState.loaded = true;
  }
}

/**
 * Load entitlement from the local cache. Independent of the sidecar: this
 * answers "has this user paid?", not "can this feature run?".
 *
 * Cache-only and synchronous with the app's startup — no network. Refreshing
 * against the oracle is [`refreshEntitlement`], deliberately separate so a slow
 * or unreachable network never delays the answer at launch.
 */
export async function loadEntitlement(userId?: string): Promise<void> {
  try {
    const s = await subscriptionApi.state(userId);
    proState.subscription = s ?? null;
    setPaid(!!s?.entitled);
  } catch {
    // A failed *read* is not a statement about entitlement. This used to set
    // `paid = false`, which is the same downgrade-on-unreachable that
    // `refreshEntitlement` below refuses by design — and it costs more now that
    // gates and background work both key off `paid`: one transient IPC failure
    // would revoke Pro and tell running work to stop.
    //
    // `paid` keeps whatever it had. The display-only subscription is cleared,
    // because showing a renewal date read from nothing would be a claim we
    // cannot support.
    proState.subscription = null;
  }
}

/**
 * Listeners notified when entitlement flips.
 *
 * Reactive UI does not need this — `proState` is a rune and a gate re-renders on
 * its own. What needs it is anything that keeps *running* rather than rendering:
 * a background executor holds no component and would otherwise keep working
 * after the entitlement it depends on expired.
 */
const entitlementListeners = new Set<(paid: boolean) => void>();

/**
 * The single place `paid` changes, so there is one place to notify from.
 *
 * Notifying only on an actual flip: entitlement is re-read on a timer, and a
 * listener that tears down a subscription must not do so every time the answer
 * comes back the same.
 */
function setPaid(next: boolean): void {
  if (proState.paid === next) return;
  proState.paid = next;

  // Snapshot before notifying. Iterating the live `Set` would deliver `next` to
  // a listener registered *during* this notification — an entry added mid-pass
  // is visited by the same pass — and a listener that itself calls `setPaid`
  // would leave the rest of the loop delivering a value that is already stale.
  for (const listener of [...entitlementListeners]) {
    try {
      listener(next);
    } catch {
      // One listener's failure must not stop the others from learning that
      // entitlement ended — that is precisely when they need to know.
    }
  }
}

/**
 * Run `cb` whenever entitlement changes. Returns an unsubscribe function.
 *
 * Use this to stop background work when Pro ends. Checking at start-up instead
 * is the bug it exists to prevent: `pro_status` is re-detected per call and the
 * cached entitlement expires on a wall clock, so a process that decided once at
 * launch keeps acting on an answer that has since changed — an upgrade needs a
 * restart to take effect, and a downgrade never takes effect at all.
 */
export function onEntitlementChange(cb: (paid: boolean) => void): () => void {
  entitlementListeners.add(cb);
  return () => entitlementListeners.delete(cb);
}

/**
 * Re-check entitlement against the oracle and persist the answer.
 *
 * Only a definite reply updates the cache. If the oracle cannot be reached —
 * offline, DNS, function not deployed — this returns having changed nothing,
 * and the cached entitlement keeps standing until its own expiry. Treating an
 * unreachable server as "not entitled" is precisely the failure that would
 * downgrade a paying customer, so it is refused here rather than guarded
 * against downstream.
 *
 * Signed out, there is nothing to ask about: the existing cache stays as it is,
 * because signing out does not revoke Pro.
 */
export async function refreshEntitlement(): Promise<void> {
  const supabase = getSupabase();
  if (!supabase) return;

  let token: string | undefined;
  try {
    const { data } = await supabase.auth.getSession();
    token = data.session?.access_token;
  } catch {
    return;
  }
  if (!token) return;

  const payload = await fetchEntitlement(token);
  if (!payload) return;

  try {
    const s = await subscriptionApi.store(payload);
    proState.subscription = s ?? null;
    setPaid(!!s?.entitled);
  } catch {
    // Persisting failed; the in-memory answer would disagree with what a
    // restart reads, so leave both alone rather than create that split.
  }
}

/** Whether a named capability may be used right now — a sidecar question. */
export function hasCapability(capability: string): boolean {
  const s = proState.status;
  return s.state === "active" && s.capabilities.includes(capability);
}

/** Whether the user is a paying customer (license entitled), regardless of sidecar. */
export function isPaid(): boolean {
  return proState.paid;
}

/**
 * A paying customer whose Pro feature cannot run right now — either the sidecar
 * is version-skewed, or they are entitled but the sidecar/capability is absent.
 * The UI uses this to say "restore your Pro component" instead of "buy Pro", so
 * a customer is never told they are not one.
 */
export function isPaidButUnavailable(capability: string): boolean {
  if (hasCapability(capability)) return false;
  return proState.paid
    || proState.status.state === "needs_update"
    || proState.status.state === "license_inactive";
}
