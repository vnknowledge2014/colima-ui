import { isRunningInTauri } from "./env";

/**
 * Outbound links shown in the app.
 *
 * The pricing page does not exist yet, so PRICING_URL points at GitHub
 * Discussions. It is declared here, and only here, so swapping in the real
 * domain later is a one-line change.
 */
export const PRICING_URL = "https://github.com/vnknowledge2014/colima-ui/discussions";

/** Repository the bug reporter files against. */
export const REPO_URL = "https://github.com/vnknowledge2014/colima-ui";

/**
 * A prefilled "new issue" URL.
 *
 * The body carries the signature and a placeholder, not the bundle itself:
 * GitHub rejects a query string past roughly 8 KB, and a diagnostic bundle is
 * routinely larger than that. Losing the body silently would be worse than
 * asking the user to paste — which they have to consent to anyway, and which is
 * the point of showing them the contents first.
 */
export function newIssueUrl(title: string, body: string): string {
  const params = new URLSearchParams({ title, body });
  return `${REPO_URL}/issues/new?${params}`;
}

/**
 * Polar configuration. All public — a checkout link and an org slug are not
 * secrets. `POLAR_ACCESS_TOKEN` is a secret and lives only in the entitlement
 * oracle, never here.
 *
 * Empty until the owner creates the seat-based products. While empty,
 * `isBillingConfigured()` is false and the UI says so rather than offering
 * buttons that lead nowhere.
 */
export const POLAR_ORG_SLUG = "";

export type PaidTier = "pro" | "pro_teams";

/**
 * A hosted Polar checkout link. Swap sandbox for production by replacing this
 * one line — the host differs (`sandbox-api` vs `api`), nothing else does.
 *
 * # A checkout link cannot make anyone Pro — in either environment
 *
 * This is a property of Checkout Links, not of the sandbox. Their documented
 * query parameters are `customer_email`, `customer_name`, `discount_code`,
 * `amount`, `custom_field_data.{slug}` and `reference_id`. There is **no**
 * `customer_external_id`, and unknown parameters are ignored silently rather
 * than rejected.
 *
 * So a purchase made through any such link creates a Polar customer keyed to
 * whatever email was typed. The entitlement oracle asks for
 * `/customers/external/<supabase-user-id>/state`, gets a 404, and correctly
 * reports Free. The money is real; the entitlement never arrives.
 *
 * Attributable purchases go through `POST /v1/checkouts/` with
 * `external_customer_id`, created by the `checkout` Edge Function — see
 * `src/lib/api/subscription.ts`. This constant is the fallback used only while
 * that function is undeployed, and the app warns before opening it.
 */
export const CHECKOUT_LINK_URL =
  "https://sandbox-api.polar.sh/v1/checkout-links/polar_cl_w6P5PxPjiyLRQIRE23nY90BXhCZmPWU4dhuG33gwawP/redirect";

/**
 * Whether the app can offer a purchase at all.
 *
 * Whether *attributable* checkout works depends on the `checkout` function being
 * deployed, which cannot be known from here — so the app tries, and reports
 * honestly when it cannot.
 */
export function isBillingConfigured(): boolean {
  return !!CHECKOUT_LINK_URL || !!POLAR_ORG_SLUG;
}

/**
 * The unattributed fallback: a checkout the buyer can complete but that no
 * account will ever be credited for.
 *
 * Returns empty when there is nothing to fall back to, so callers can say
 * "checkout unavailable" rather than opening the pricing page dressed up as a
 * purchase.
 */
export function fallbackCheckoutUrl(): string {
  return CHECKOUT_LINK_URL;
}

/**
 * The Polar Customer Portal — where seats, invoices, payment method and
 * cancellation all live.
 *
 * This is the unauthenticated entry point: the customer signs in with the email
 * they purchased under and Polar sends a one-time code. A pre-authenticated link
 * would need `customerSessions.create()`, which requires the org secret, so it
 * could only come from the entitlement oracle — worth adding later, not needed
 * for the portal to work.
 */
export function portalUrl(): string {
  return POLAR_ORG_SLUG ? `https://polar.sh/${POLAR_ORG_SLUG}/portal` : PRICING_URL;
}

/**
 * Whether a string is a URL this app is willing to hand to the operating system.
 *
 * `https:` only. In the desktop app an external link leaves the webview entirely
 * and is resolved by the OS handler, which is outside the CSP and outside
 * anything the app can undo — `javascript:`, `file:` and a long tail of
 * registered schemes all mean something there.
 *
 * Parsed with `new URL()` rather than matched with a regex: the parser is the
 * same one that decides what the scheme actually is, so it cannot disagree with
 * whatever opens the link.
 */
export function isSafeExternalUrl(url: string): boolean {
  try {
    return new URL(url).protocol === "https:";
  } catch {
    return false;
  }
}

/**
 * Hosts an announcement may link to.
 *
 * Kept here, next to the other outbound URLs, so extending it is one edit in one
 * file. Deliberately short: announcement content is fetched from the network, so
 * this is the only list standing between a compromised feed and a link the user
 * is invited to click.
 */
const ANNOUNCEMENT_LINK_HOSTS = ["github.com", "www.github.com", "polar.sh"];

/**
 * Whether an announcement's `linkUrl` may be offered as a link.
 *
 * A rejected link is simply not rendered — the announcement still shows its text,
 * which is the part that carries the warning.
 */
export function isAllowedAnnouncementLink(url: string | undefined): boolean {
  if (!url || !isSafeExternalUrl(url)) return false;
  try {
    const parsed = new URL(url);
    // Port and credentials are refused rather than ignored. Neither can point at
    // another host, but `https://user:pw@github.com/` hands credentials to the
    // browser and a port says the link is not the ordinary web page it looks
    // like — a real vendor announcement needs neither.
    if (parsed.port || parsed.username || parsed.password) return false;
    return ANNOUNCEMENT_LINK_HOSTS.includes(parsed.hostname.toLowerCase());
  } catch {
    return false;
  }
}

/**
 * Open a URL in the user's default browser.
 *
 * In the desktop app the webview must hand the URL to the OS via the opener
 * plugin; in browser mode there is no plugin, so fall back to window.open.
 *
 * Anything that is not `https:` is refused here rather than at each call site.
 * Every caller today passes a compile-time constant, so the guard changes
 * nothing for them — it is here for the callers that come later, and for the one
 * that already passes a value from the network (announcement links, which are
 * filtered again by host before a link is even drawn).
 */
export async function openExternal(url: string): Promise<void> {
  if (!isSafeExternalUrl(url)) {
    console.warn("Refused to open a non-https URL");
    return;
  }
  if (isRunningInTauri()) {
    try {
      const { openUrl } = await import("@tauri-apps/plugin-opener");
      await openUrl(url);
      return;
    } catch {
      // Plugin unavailable — fall through to the browser path rather than
      // leaving the click with no visible effect.
    }
  }
  window.open(url, "_blank", "noopener,noreferrer");
}
