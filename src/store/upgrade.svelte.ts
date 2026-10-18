import { PRICING_URL, fallbackCheckoutUrl, openExternal, type PaidTier } from "../lib/external-links";
import { accountState } from "../lib/account.svelte";
import { showSignIn } from "./account-dialog.svelte";
import { createCheckout } from "../lib/api/subscription";
import { getSupabase } from "../lib/supabase";
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

/**
 * Open the Polar checkout for a tier.
 *
 * **Sign-in is required first.** The Supabase user id travels with the checkout
 * as Polar's external customer id, and it is the only thing that later lets the
 * entitlement oracle connect the purchase to the person using the app. Buying
 * while signed out would produce a paid subscription with nothing to attach it
 * to — so an unauthenticated click opens sign-in instead, rather than taking
 * money we cannot honour.
 *
 * Falls back to the pricing page while no product URL is configured, so the
 * click is never dead.
 */
export async function goToCheckout(tier: PaidTier = "pro", seats?: number): Promise<void> {
  const userId = accountState.user?.id;
  if (!userId) {
    closeUpgrade();
    showSignIn();
    return;
  }

  // Funnel: the last step before purchase. Consent-gated server-side.
  telemetryApi.recordCheckoutOpened();

  // The real path: a session created server-side carrying `external_customer_id`,
  // which is the only thing that later connects the purchase to this account.
  const supabase = getSupabase();
  let url: string | null = null;
  if (supabase) {
    try {
      const { data } = await supabase.auth.getSession();
      const token = data.session?.access_token;
      if (token) url = await createCheckout(token, tier, seats);
    } catch {
      // Fall through to the fallback below.
    }
  }

  if (url) {
    openExternal(url);
    closeUpgrade();
    return;
  }

  // The `checkout` function is not deployed. A hosted checkout link can still
  // take a payment, but no such link can carry `external_customer_id`, so Polar
  // has no way to attach the purchase to this account and the buyer would pay
  // and stay Free. That is true in production as well as sandbox, so warn rather
  // than let it happen quietly.
  const fallback = fallbackCheckoutUrl();
  if (!fallback) {
    globalToast("error", t('subscription.checkout_unavailable', {
      default: 'Checkout is unavailable right now. Nothing was charged.',
    }));
    return;
  }

  globalToast("info", t('subscription.checkout_unlinked', {
    default: 'This purchase will not be linked to your account and will not unlock Pro.',
  }));
  openExternal(fallback);
  closeUpgrade();
}
