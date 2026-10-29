import { PRICING_URL, fallbackCheckoutUrl, openExternal, type PaidTier } from "../lib/external-links";
import { globalToast } from "../lib/globalToast";
import { t } from "../lib/i18n.svelte";
import { telemetryApi } from "../lib/api";

/**
 * Drives the single global UpgradeDialog. A ProGate (or any code) calls
 * `showUpgrade(...)` to explain what a Pro feature does and link out to
 * pricing — it never blocks a free action, only offers.
 */
export interface UpgradeOptions {
  /** Human name of the feature, e.g. "Compose auto-fix". */
  feature: string;
  /** One line on what it does. */
  description?: string;
}

export const upgradeState = $state({
  open: false,
  options: { feature: "" } as UpgradeOptions,
});

export function showUpgrade(options: UpgradeOptions | string): void {
  upgradeState.options = typeof options === "string" ? { feature: options } : options;
  upgradeState.open = true;
}

export function closeUpgrade(): void {
  upgradeState.open = false;
}

export function goToPricing(): void {
  openExternal(PRICING_URL);
  closeUpgrade();
}

export async function goToCheckout(_tier: PaidTier = "pro", _seats?: number): Promise<void> {
  telemetryApi.recordCheckoutOpened();

  const fallback = fallbackCheckoutUrl();
  if (!fallback) {
    globalToast("error", t('subscription.checkout_unavailable', {
      default: 'Checkout is unavailable right now. Nothing was charged.',
    }));
    return;
  }

  openExternal(fallback);
  closeUpgrade();
}
