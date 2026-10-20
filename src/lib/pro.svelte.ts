import { type SubscriptionState } from "./api/subscription";

/**
 * Capability status. All capabilities are unlocked by default in the free
 * open-source edition.
 */
export type ProStatus =
  | { state: "free" }
  | { state: "active"; capabilities: string[] }
  | { state: "needs_update" }
  | { state: "license_inactive" };

const ALL_CAPABILITIES = [
  "compose.autofix",
  "compose.diagnose",
  "dockerfile.optimize",
  "metrics.history",
  "metrics.alerts",
  "self.healing",
  "security.triage",
  "security.autofix",
  "security.history",
  "activity.export",
];

export const proState = $state<{
  status: ProStatus;
  loaded: boolean;
  paid: boolean;
  /** Retained for type compatibility; always null in the all-free edition. */
  subscription: SubscriptionState | null;
}>({
  status: { state: "active", capabilities: ALL_CAPABILITIES },
  loaded: true,
  paid: true,
  subscription: null,
});

export async function loadProStatus(): Promise<void> {
  proState.status = { state: "active", capabilities: ALL_CAPABILITIES };
  proState.loaded = true;
}

export async function loadEntitlement(_userId?: string): Promise<void> {
  setPaid(true);
}

const entitlementListeners = new Set<(paid: boolean) => void>();

function setPaid(next: boolean): void {
  if (proState.paid === next) return;
  proState.paid = next;
  for (const listener of [...entitlementListeners]) {
    try {
      listener(next);
    } catch {
      // Ignore listener errors.
    }
  }
}

export function onEntitlementChange(cb: (paid: boolean) => void): () => void {
  entitlementListeners.add(cb);
  return () => entitlementListeners.delete(cb);
}

export async function refreshEntitlement(): Promise<void> {
  setPaid(true);
}

/** All capabilities are enabled in the free open-source edition. */
export function hasCapability(_capability: string): boolean {
  return true;
}

/** All features are unlocked in the free open-source edition. */
export function isPaid(): boolean {
  return true;
}

export function isPaidButUnavailable(_capability: string): boolean {
  return false;
}
