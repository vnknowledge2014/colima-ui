---
phase: 6
title: "Subscription & Teams UI"
status: completed
priority: P1
dependencies: [4, 5]
effort: "1-2 days"
---

# Phase 6: Subscription & Teams UI

> Rewritten after phase 2's Step 0. Team management is **Polar's hosted portal**,
> not a roster UI we build. Effort drops accordingly.

## Overview
The surfaces a customer actually touches: choosing a plan, buying it, seeing what
they have, and reaching Polar's portal to manage the rest.

## Requirements
- Functional: plan picker (Free / Pro / Pro Teams), checkout entry, read-only
  subscription status, and outbound links into Polar's portal.
- Non-functional: honest degradation — a failed entitlement refresh must read as
  "couldn't refresh", never as "you lost Pro". Free stays fully usable and unnagged.

## Architecture

- **Settings → Subscription** replaces Settings → License. Read-only: tier, seats
  claimed/total, renewal date — plus two outbound buttons, "Manage team" and
  "Billing & invoices", both into Polar's Customer Portal.
- **No Settings → Team page.** Polar's portal owns invite, revoke, resend and seat
  count. Rebuilding it would mean maintaining a second UI over the same data, and
  every mutation would need its own authorized server path.
- `UpgradeDialog` becomes a **plan picker** rather than a single "Get Pro" link.
- **No invite deep-link handling.** Polar emails the claim link and hosts the claim
  page; the member then signs into ColimaUI and the oracle sees a claimed seat.

Sending customers to Polar's portal also inherits things worth having:
failed-payment recovery, and self-service cancellation — the latter is legally
required in some jurisdictions (California's Automatic Renewal Law).

The existing `ProGate` / `UpgradeDialog` pattern stays: gates offer, never block.
Free remains fully usable, and nothing here should turn into a nag screen.

## Related Code Files
- Create: `src/pages/settings/Subscription.svelte` — replaces `License.svelte`.
- Create: `src/lib/api/subscription.ts` — calls the entitlement oracle.
- Modify: `src/components/UpgradeDialog.svelte` — plan picker.
- Modify: `src/pages/Settings.svelte` — swap License for Subscription.
- Modify: `src/lib/external-links.ts` — portal + checkout URLs carrying tier and
  the Supabase user id as Polar's external customer id.
- Modify: `src/locales/{en,vi,zh,ja}.json` — per Phase 1's string inventory. The
  `account.signin.no_unlock` line ("an account unlocks nothing") is now **false**
  and must go.

## Implementation Steps
1. Build `Subscription.svelte` against phase 5's entitlement state.
2. Build the plan picker; wire checkout with tier + external customer id.
3. **Do not set a custom `success_url` on checkout** unless the buyer's seat is
   also assigned via the API — see the risk below. This is the sharpest edge in
   the whole plan.
4. Make the oracle-unreachable path explicit: the page says "couldn't refresh",
   while entitlement keeps rendering from cache. Never imply Pro was lost.
5. Rewrite the affected i18n strings in all four locales, per phase 1's inventory
   (16 delete, 5 rewrite, 7 keep, 2 new namespaces — `subscription.*`, `plans.*`).
6. Remove `License.svelte` and every reference to license keys.

## Success Criteria
- [ ] Plan picker → checkout → **Pro actually active**, with no key typed. Verify
      the buyer holds a *claimed* seat afterwards, not merely a purchase.
- [ ] Subscription page shows tier and seats claimed/total accurately.
- [ ] "Manage team" and "Billing" reach the Polar portal, pre-authenticated.
- [ ] Oracle unreachable → cached entitlement still renders; no false downgrade.
- [ ] Free unchanged and unnagged; no first-launch wall added.
- [ ] All four locales updated; no string still claims an account unlocks nothing.
- [ ] No license-key UI remains.

## Risk Assessment
- **Stale copy is now a correctness bug**, not a typo: the app currently tells
  users in four languages that an account unlocks nothing.
- **Team management failure looking like entitlement failure.** A user who sees
  "team unavailable" must not conclude they have lost Pro — keep the two
  surfaces visually and textually separate.
- **`success_url` silently voids the purchase.** Polar auto-claims the buyer's
  seat only on its default confirmation page. Set a custom `success_url` and the
  buyer pays, returns to our page, and holds **no seat** — Free, with a receipt.
  For a 1-seat Pro purchase that is a total failure of the transaction, and it
  fails quietly. Either keep Polar's confirmation page, or assign the seat via API
  before redirecting. Explicit test in phase 7.
- **Billing manager ≠ seat holder.** The person who pays gets no benefits unless
  they also hold a seat. A Teams buyer may reasonably expect Pro immediately.
- **Nag creep.** With two paid tiers there is pressure to push upgrades harder.
  The published promise is that Free is fully usable; keep gates as offers.

## Phase 6 execution log — 2026-08-11

`pnpm build` OK · `pnpm check` **143 errors / 36 warnings** (baseline, no new).
No Rust change, so `cargo` gates are unaffected.

### Built

| File | Role |
|---|---|
| `src/pages/settings/Subscription.svelte` | Read-only tier / seats / renewal, with portal + refresh buttons. Replaces `License.svelte` |
| `src/components/UpgradeDialog.svelte` | Rewritten as a plan picker (Pro / Pro Teams) |
| `src/lib/external-links.ts` | Per-tier checkout carrying the external customer id; Polar portal URL |
| `src/store/upgrade.svelte.ts` | `goToCheckout(tier, seats)`, sign-in gated |
| `src/locales/*.json` | +19 keys × 4 locales (`subscription.*`, `plans.*`); `settings.pro.*` retired |

### Three decisions worth recording

**Checkout requires sign-in, and says so before the click.** The Supabase user id
travels as Polar's external customer id and is the only thing that later connects
the purchase to the person using the app. A signed-out click therefore opens
sign-in rather than checkout — taking money for a subscription with nothing to
attach it to would be the worst possible version of this feature. The Subscription
page states it in advance rather than surprising the user mid-flow.

**No custom `success_url`, enforced by absence.** Polar auto-claims the buyer's own
seat only on its default confirmation page, so returning the user to the app after
payment would leave them Free with a receipt. `checkoutUrl()` takes no success-URL
parameter at all, and the reasoning is in a comment at the point where someone
would otherwise add one. Verified: no `success_url` anywhere in `src/`.

**The old "ColimaUI Pro — COMING SOON" card was deleted, not left beside the new
page.** It described a state that no longer exists and would have contradicted the
Subscription section two cards below it. Its `settings.pro.*` keys went with it, in
all four locales.

### Honest degradation

- **Not configured** (no product URLs yet — today's state): plan picker and
  Subscription both say paid plans are not available, instead of offering buttons
  that lead to the pricing page pretending to be checkout.
- **Lapsed vs never-subscribed** are different screens. A lapsed subscriber sees
  when it ended and a billing link, not the new-customer pitch.
- **Offline note sits next to the renewal date**, so the date does not read as a
  deadline requiring connectivity.

### Success criteria

- [x] Plan picker → checkout, with no key typed anywhere. *(Live purchase
      unverifiable until the owner creates the Polar products — see below.)*
- [x] Subscription page shows tier and seats claimed/total.
- [x] "Manage team" / billing reach the Polar portal.
- [x] Oracle unreachable → cached entitlement still renders; no false downgrade.
- [x] Free unchanged and unnagged; no first-launch wall added.
- [x] All four locales updated; 19/19 new keys resolve in each.
- [x] No license-key UI remains (0 references).

### Blocked on the owner

`POLAR_ORG_SLUG` and `CHECKOUT_URLS` in `src/lib/external-links.ts` are empty.
Until the seat-based products exist, `isBillingConfigured()` is false and both
surfaces explain themselves. Filling them is a three-line change.

Unverifiable until then: the end-to-end purchase, and specifically **whether the
buyer ends up holding a *claimed* seat** — the phase 7 test that catches the
`success_url` trap.

### Note on the portal link

`portalUrl()` returns the public `polar.sh/<org>/portal`, where the customer signs
in with an email one-time code. A pre-authenticated link needs
`customerSessions.create()` and therefore the org secret, so it can only come from
the entitlement oracle. Worth adding in phase 4 as a second endpoint; not required
for the portal to work.

## Correction — 2026-08-11 22:00: checkout links cannot carry identity

Found when the owner supplied a real sandbox checkout link. **Phase 6 shipped a
defect that would have taken money without ever granting Pro.**

`checkoutUrl()` appended `customer_external_id` to a hosted Checkout Link. That
parameter does not exist: the documented set is `customer_email`,
`customer_name`, `discount_code`, `amount`, `custom_field_data.{slug}`,
`reference_id`. Unknown parameters are **ignored silently**, so nothing would
have failed visibly. Polar would have created a customer keyed to the typed
email; the oracle would have asked for
`/customers/external/<supabase-user-id>/state`, received 404, and correctly
reported Free — permanently, to a paying customer, with no error logged anywhere.

The URL format was wrong too: `https://<env>-api.polar.sh/v1/checkout-links/<id>/redirect`,
not the `buy.polar.sh/...` guessed when writing the setup doc.

### Fix

`POST /v1/checkouts/` accepts `external_customer_id` — *"if a matching customer
exists on Polar, the resulting order will be linked to this customer. Otherwise,
a new customer will be created with this external ID set."* It needs
`checkouts:write`, so the session is created server-side.

- **New:** `supabase/functions/checkout/index.ts` — verifies the JWT, creates the
  session with `external_customer_id = user.id`, returns the URL. Only the
  *tier* is read from the body; identity never is.
- `src/lib/api/subscription.ts` — `createCheckout()`.
- `src/store/upgrade.svelte.ts` — server session first; falls back to a plain
  link **with a visible warning** that the purchase will not be linked.
- `src/lib/external-links.ts` — `checkoutUrl()`/`CHECKOUT_URLS` removed;
  `CHECKOUT_LINK_URL` is an environment-neutral fallback (the limitation is a
  property of checkout links, not of sandbox).
- Token scope is now `customers:read` + `checkouts:write`; product ids move to
  function env (`POLAR_PRODUCT_PRO`, `POLAR_PRODUCT_PRO_TEAMS`).
- `docs/polar-billing-setup.md` corrected: URL format, "why not a Checkout Link",
  scopes, and a second required verification — *the Polar customer must carry the
  Supabase user id as its external ID*.

Gates: `build` OK, `check` 143/36 (baseline). The functions remain untypechecked
— no Deno or Supabase CLI on this machine.

### What this says about the plan

Phase 2 verified that Polar *supports seats*. It did not verify **how a purchase
binds to an account**, and phase 6 filled that gap with an assumption that looked
reasonable and was wrong. Phase 7's "buy and confirm a claimed seat" test would
have caught it — but only after a real payment.
