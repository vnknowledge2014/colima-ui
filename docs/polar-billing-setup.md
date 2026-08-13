# Polar Billing Setup — Pro & Pro Teams seats

Step-by-step console guide for selling **Pro** and **Pro Teams** through Polar,
the merchant of record. Entitlement only — see
[account-supabase-oauth.md](./account-supabase-oauth.md) for how a purchase turns
into Pro in the app, and [pricing-rationale.md](./pricing-rationale.md) for why
the tiers are priced this way.

There are **no license keys.** Pro is a seat subscription resolved for the
signed-in account.

**Time:** ~45 minutes, including a sandbox test purchase. **Cost:** free until
you sell something (Polar takes a cut per transaction).

---

## What you end up with

| Piece | Where it lives | Secret? |
|---|---|---|
| `POLAR_ORG_SLUG` | `src/lib/external-links.ts` | No — public |
| `POLAR_ACCESS_TOKEN` | Supabase function secret **only** | **YES — organization-wide** |
| `POLAR_PRODUCT_PRO` / `_PRO_TEAMS` | Supabase function secret | No, but server-side |
| `POLAR_ENV`, `OFFLINE_GRACE_DAYS` | Supabase function secret | No |

The access token can act on your whole organization. It never goes in the app,
the repo, or a log line. The app itself holds **no Polar credential at all** —
only the org slug, used to build a portal link.

> Polar participates in the **GitHub Secret Scanning Program**: if a token is
> pushed to a public repo, it is revoked automatically and you get an email. That
> is a safety net, not a plan — a revoked token means every customer's
> entitlement check starts failing.

---

## 1. Start in the sandbox

Polar's sandbox is a **completely separate environment**, not a test mode inside
your live account: separate URL, separate login, separate organization, separate
tokens. A production token does not work in the sandbox and vice versa.

1. Go to [sandbox.polar.sh](https://sandbox.polar.sh/start).
2. Create a user account and an organization (e.g. `colimaui`).

The app defaults to sandbox unless `POLAR_ENV=production`, so nothing you do here
can charge real money — the same fail-safe default the retired license client had.

> **Sandbox email limitation.** Customer-facing emails — including **seat
> invitations** — are delivered only to members of your organization. To test a
> team invite, either add the recipient under **Settings → Members**, or use a
> sub-address like `you+teammate@example.com`, which is accepted.

---

## 2. Create the two products

**Products → New Product**, twice. Both are **subscriptions** with **seat-based
pricing** — that is what makes Polar model members and seats for you.

| | Pro | Pro Teams |
|---|---|---|
| Billing cycle | Recurring, monthly | Recurring, monthly |
| Pricing model | **Seat-based** | **Seat-based** |
| Price per seat | $6 | $6 |
| Seats | 1 | minimum 2 |

Both tiers ship the **same features** — Teams is Pro bought for several people
plus seat management. Do not configure different benefits per tier; the app's
entitlement is a boolean and no gate branches on tier.

> Billing cycle and pricing model are **locked at creation**. If you pick the
> wrong one, create a new product rather than trying to edit it.

---

## 3. Attach a benefit to each product

**This step is easy to skip and quietly breaks entitlement for team members.**

Polar grants benefits to **seat members, not to the billing customer**. The
entitlement oracle looks for a benefit grant to decide which subscription applies
to the person asking. A product with no benefits produces no grants.

1. On each product, **Benefits → New Benefit**.
2. A **Custom** benefit is enough — the app does not read its contents, only that
   a grant exists. Name it something honest like `ColimaUI Pro`.
3. Attach it to **both** products.

---

## 4. Copy the product IDs

Open each product and copy its **ID** (a UUID, shown in the dashboard URL and on
the product page). You need both.

These go to the **server**, not the app — a checkout session names a product, and
that call carries the buyer's identity, so it happens in an Edge Function.

> ### Why not a Checkout Link
>
> Checkout Links are the obvious choice and they do not work here. Their
> documented query parameters are `customer_email`, `customer_name`,
> `discount_code`, `amount`, `custom_field_data.{slug}` and `reference_id` —
> there is **no `customer_external_id`**, and an unknown parameter is ignored
> silently rather than rejected.
>
> So a purchase through a link creates a Polar customer keyed to the typed email.
> The app then asks for `/customers/external/<supabase-user-id>/state`, gets a
> 404, and correctly reports Free. **The money is real and the entitlement never
> arrives** — and nothing anywhere logs an error.
>
> `POST /v1/checkouts/` does accept `external_customer_id`, which is why the
> `checkout` function exists. Polar's own guidance agrees: *"If you need to
> create Checkout Sessions programmatically (e.g. with per-customer data computed
> by your backend), use the Checkout API directly instead."*
>
> This is a property of Checkout Links, **not of the sandbox** — a production
> link fails the same way.

> ### Never set a Success URL
>
> Polar claims the buyer's own seat automatically **only** on its default
> confirmation page. Redirect away from it and the buyer pays, gets a receipt,
> and holds **no seat** — still on Free. For a 1-seat Pro purchase that is a
> total failure of the transaction, and it fails silently.
>
> The `checkout` function omits `success_url` deliberately. If you later create a
> Checkout Link by hand, leave its Success URL empty for the same reason.

---

## 5. Fill in the app config

`src/lib/external-links.ts`:

```ts
export const POLAR_ORG_SLUG = "colimaui";   // your org slug
```

The org slug builds the Customer Portal URL (`https://polar.sh/<slug>/portal`;
`sandbox.polar.sh` in sandbox). That is the only Polar config the app holds.

`CHECKOUT_LINK_URL` in the same file is a **fallback used only while the
`checkout` function is undeployed**. It lets you exercise the browser hand-off,
and the app shows a warning before opening it because a purchase made that way
cannot be attributed. Empty it once checkout is deployed.

---

## 6. Create the access token

**Organization Settings → Developers → New Token** (an *Organization Access
Token*).

- **Scopes:** `customers:read` and `checkouts:write`. Nothing more — the
  functions read customer state and create checkout sessions; neither modifies a
  customer or a subscription.
- Copy it once; it is not shown again.

Create it in the **same environment** you are testing against. A production token
does not work in sandbox.

---

## 7. Deploy the two functions

The token lives only here — in Supabase Edge Functions — because both reading
seat state and binding a checkout to an account need a secret the desktop app
cannot hold.

```bash
supabase functions deploy entitlement
supabase functions deploy checkout

supabase secrets set \
  POLAR_ACCESS_TOKEN=polar_oat_… \
  POLAR_ENV=sandbox \
  POLAR_PRODUCT_PRO=<product-uuid> \
  POLAR_PRODUCT_PRO_TEAMS=<product-uuid> \
  OFFLINE_GRACE_DAYS=30
```

Switch `POLAR_ENV` to `production` only when you move off the sandbox — and
remember the token must be swapped at the same time.

`OFFLINE_GRACE_DAYS` is how long the app honours a cached entitlement without
being able to re-check. It is also the enforcement lag: a cancelled subscriber, a
removed team member, or a shared machine whose owner signed out all keep Pro for
up to this long.

Until the function is deployed, the app runs purely on its local cache and
`refreshEntitlement()` is a silent no-op — correct degradation, not a fallback
hack.

---

## 8. Verify with a real test purchase

1. Build the debug bundle (sign-in needs a registered `.app`, so `tauri dev` is
   not enough):
   ```bash
   npx tauri build --debug
   open src-tauri/target/debug/bundle/macos/ColimaUI.app
   ```
2. **Settings → Account → Sign in** first. Checkout carries your Supabase user id
   as Polar's *external customer id*; buying while signed out would leave the
   purchase with nothing to attach to, so the app asks you to sign in.
3. **Settings → Subscription → See plans →** pick a tier.
4. Pay with Stripe's test card `4242 4242 4242 4242`, any future expiry, any CVC.
5. **Stay on Polar's confirmation page** until it finishes — that is where your
   seat is claimed.
6. Back in the app: **Settings → Subscription → Refresh**.

**Two checks, both required:**

1. You are **Pro** in the app, and Polar shows your seat as **claimed** — not
   merely a completed order. A purchase without a claimed seat is the
   `success_url` failure above.
2. In Polar, the customer created by that purchase has your **Supabase user id as
   its external ID**. If it does not, the purchase went through a plain checkout
   link and will never grant entitlement, however long you wait.

### Failure modes

| Symptom | Cause | Fix |
|---|---|---|
| Paid, but still Free, **and no Polar customer has your user id as external ID** | Bought through a plain Checkout Link, which cannot carry `external_customer_id` | Deploy the `checkout` function (step 7) and buy again. The first purchase must be refunded or linked by hand |
| Paid, but still Free | Seat never claimed — a Success URL was set | Clear it; claim the seat from the Customer Portal |
| Paid, still Free, seat *is* claimed | Oracle not deployed, or `POLAR_ENV` / token environment mismatch | Steps 6–7. Check function logs |
| Team member signed in but not Pro | Their seat is **pending**, not claimed — an invitation is not entitlement | Have them open the invite email and claim it. Links expire in **24 hours**; resend from the portal |
| Buyer of Pro Teams is not Pro | Billing managers receive no benefits unless they also hold a seat | Assign yourself one of the seats in the Customer Portal |
| Seat invite email never arrives (sandbox) | Sandbox delivers only to organization members | Add them under Settings → Members, or use `you+alias@example.com` |
| "This purchase will not be linked to your account" warning | The `checkout` function is not deployed; the app is falling back to a plain link | Step 7. The warning is correct — do not pay through it expecting Pro |
| Plan buttons say "not available yet" | `CHECKOUT_LINK_URL` and `POLAR_ORG_SLUG` both empty | Step 5 |
| Pro disappears while offline | Cache expired past `OFFLINE_GRACE_DAYS` | Expected. Reconnect and it returns; raise the window if 30 days is too tight |
| Everyone loses Pro at once | Token revoked, expired, or wrong environment | Check function logs for `upstream_unavailable`. The app keeps cached Pro until expiry, so you have that long to fix it |

---

## 9. Going to production

1. Create the organization, products, benefits and checkout links again at
   [polar.sh](https://polar.sh) — sandbox data does not migrate.
2. Connect a payout account (**Finance → Payout Accounts**, Stripe Connect).
3. Create a **production** access token (`customers:read`, `checkouts:write`).
4. `supabase secrets set POLAR_ACCESS_TOKEN=… POLAR_ENV=production
   POLAR_PRODUCT_PRO=… POLAR_PRODUCT_PRO_TEAMS=…` — product ids differ per
   environment, and a stale one fails checkout with `checkout_failed`.
5. Update `POLAR_ORG_SLUG`, and empty `CHECKOUT_LINK_URL`.
6. Repeat step 8 with a real card, then refund yourself.

---

## Security notes

- **`POLAR_ACCESS_TOKEN` is the highest-value secret in the product.** It is
  organization-wide. It belongs in the Supabase function environment and nowhere
  else — not `.env` in the repo, not the app, not CI logs. `supabase/.env` is
  git-ignored; `supabase/.env.example` documents the names only.
- **The app holds no Polar credential at all.** The org slug is a public URL
  fragment; everything privileged happens in the two functions.
- **Scope the token to `customers:read` + `checkouts:write`.** Neither function
  modifies a customer or a subscription, so a wider token only adds blast radius.
- **Sandbox by default.** `POLAR_ENV` must be set to `production` explicitly, so
  a forgotten variable means test mode rather than real charges.
- **An unreachable Polar is not "not entitled."** The oracle returns
  `503 upstream_unavailable` and the app keeps its cached entitlement. Losing
  Polar, or losing the network, never downgrades a paying customer.
