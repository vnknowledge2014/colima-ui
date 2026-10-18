/**
 * Creates a Polar checkout session bound to the signed-in account.
 *
 * # Why this is a function and not a static link
 *
 * A plain Checkout Link cannot carry the buyer's identity. Its documented query
 * parameters are `customer_email`, `customer_name`, `discount_code`, `amount`,
 * `custom_field_data.{slug}` and `reference_id` — there is **no**
 * `customer_external_id` among them, and an unknown parameter is silently
 * ignored rather than rejected.
 *
 * The consequence would have been invisible until someone paid: Polar would
 * create a customer keyed only by the email they typed at checkout, the
 * entitlement oracle would ask for `/customers/external/<supabase-user-id>/state`,
 * get a 404, and report Free. Forever, to a paying customer.
 *
 * `POST /v1/checkouts/` does accept `external_customer_id` — "if a matching
 * customer exists on Polar, the resulting order will be linked to this customer.
 * Otherwise, a new customer will be created with this external ID set." That is
 * exactly the binding the oracle later reads, so checkout has to happen here,
 * where the organization token lives.
 */

import { getAuthedUser, json, preflight } from "../_shared/auth.ts";
import { apiBase } from "../_shared/polar.ts";

/** Product ids come from the environment: they differ per Polar environment. */
function productIdFor(tier: string): string | null {
  const id = tier === "pro_teams"
    ? Deno.env.get("POLAR_PRODUCT_PRO_TEAMS")
    : Deno.env.get("POLAR_PRODUCT_PRO");
  return id && id.trim() !== "" ? id : null;
}

Deno.serve(async (req: Request) => {
  const pre = preflight(req);
  if (pre) return pre;
  if (req.method !== "POST") return json({ error: "method_not_allowed" }, 405);

  const user = await getAuthedUser(req);
  if (!user) return json({ error: "unauthorized" }, 401);

  let tier = "pro";
  let seats: number | undefined;
  try {
    const body = await req.json();
    // Only the *tier* is taken from the body — never identity. The worst a
    // caller can do by lying here is buy the other plan.
    if (body?.tier === "pro_teams" || body?.tier === "pro") tier = body.tier;
    if (Number.isInteger(body?.seats) && body.seats > 1) seats = body.seats;
  } catch {
    // No body is fine; defaults apply.
  }

  const productId = productIdFor(tier);
  if (!productId) return json({ error: "not_configured" }, 503);

  const token = Deno.env.get("POLAR_ACCESS_TOKEN");
  if (!token) return json({ error: "not_configured" }, 503);

  const payload: Record<string, unknown> = {
    products: [productId],
    // The whole point of this function.
    external_customer_id: user.id,
    // Prefills the email field; the binding above is what actually matters.
    ...(user.email ? { customer_email: user.email } : {}),
    ...(seats ? { seats } : {}),
    // `success_url` is deliberately omitted. Polar auto-claims the buyer's own
    // seat only on its default confirmation page; redirecting away from it
    // leaves the buyer paid-up and seatless, which reads to them as "I bought
    // Pro and nothing happened".
  };

  let res: Response;
  try {
    res = await fetch(`${apiBase()}/v1/checkouts/`, {
      method: "POST",
      headers: {
        Authorization: `Bearer ${token}`,
        "Content-Type": "application/json",
      },
      body: JSON.stringify(payload),
    });
  } catch (e) {
    console.error("[checkout] unreachable:", e instanceof Error ? e.message : "fetch failed");
    return json({ error: "upstream_unavailable" }, 503);
  }

  if (!res.ok) {
    // Body may name the invalid field (e.g. a stale product id); log it for
    // operators. It contains no customer data and no secret.
    console.error(`[checkout] Polar returned ${res.status}: ${await res.text()}`);
    return json({ error: "checkout_failed" }, 502);
  }

  const checkout = await res.json();
  if (!checkout?.url) return json({ error: "checkout_failed" }, 502);

  return json({ url: checkout.url });
});
