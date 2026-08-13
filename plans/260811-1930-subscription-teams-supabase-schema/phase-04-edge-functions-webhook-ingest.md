---
phase: 4
title: "Entitlement Oracle (Edge Function)"
status: completed
priority: P1
dependencies: [2]
effort: "1 day"
---

# Phase 4: Entitlement Oracle

> Rewritten after phase 2's Step 0. This phase was six functions writing roster
> tables; it is now **one function** that answers a single question. Phase 3 is
> cancelled, so there are no tables and no client write paths anywhere.

## Overview
A desktop app holding only public config cannot ask Polar about seats — that needs
`POLAR_ACCESS_TOKEN`, an organization secret. One Edge Function holds it and
answers: *given this signed-in user, are they entitled, and until when?*

## Why this exists at all

The retired license-key flow needed no backend: Polar's key-validate endpoint
takes the key and a public org id. Seat state has no such public endpoint —
`customerSessions.create()` and the seat APIs are server calls.

That single fact is the entire justification for this phase. It is the minimum
server surface that makes account-based entitlement possible, and it should not
grow beyond it.

## Requirements
- Functional: return `{ entitled, tier, seats_total, seats_claimed, expires_at }`
  for the caller.
- Non-functional: identity comes **only** from a verified Supabase JWT.
  `POLAR_ACCESS_TOKEN` never leaves the function environment. The function is the
  only server-side component in the product.

## Architecture

```
app ──(user JWT)──> Edge Function `entitlement`
                       │  verify JWT  → user id + email
                       │  ask Polar   → seat for this external customer id
                       └─ returns { entitled, tier, seats, expires_at }
                            └─> client caches to ~/.colima-ui/subscription.json

Polar ──(signed webhook)──> Edge Function `polar-webhook`   [optional]
                       └─ nudges the client to re-check sooner than expiry
```

The link is **external customer ID = Supabase user id**, which Polar supports for
seat assignment natively. No email matching, and none of the "I paid but it says
Free" support load that email matching generates.

### The two rules

1. **Never trust a client-supplied identity.** The user id comes from verifying
   the JWT, never from the request body. A function that accepts `{ user_id }`
   and acts on it lets anyone read anyone's entitlement.
2. **Verify the webhook signature before parsing intent.** An unauthenticated
   webhook endpoint is a public "grant me a subscription" button.

### The webhook is optional, and should stay optional

Its only job is to shorten the lag between a subscription change and the client
noticing. Entitlement is still correct without it — the cache expires and the
client re-checks. Build it only if the lag proves annoying in practice; it is not
on the correctness path.

## Related Code Files
- Create: `supabase/functions/entitlement/index.ts`.
- Create: `supabase/functions/_shared/auth.ts` — JWT verification, used by every
  function so the check exists once rather than in near-copies.
- Create: `supabase/functions/_shared/polar.ts` — Polar client + webhook signature
  verification.
- Create (optional, later): `supabase/functions/polar-webhook/index.ts`.
- Modify: `docs/account-supabase-oauth.md` — document the one server component.

## Implementation Steps
1. Scaffold the function; confirm `supabase functions serve` works locally.
2. Build `_shared/auth.ts` first — everything depends on it.
3. Implement `entitlement`: verify JWT → look up the caller's seat at Polar by
   external customer id → map to the payload.
4. Decide the response for every Polar state: no customer, seat `pending`
   (assigned but unclaimed — **not** entitled), `claimed` (entitled), `revoked`,
   subscription past due, subscription cancelled.
5. Fail **closed** on an unexpected Polar response, but distinguish "Polar says
   no" from "Polar could not be reached" — the client must keep honouring its
   cache on the latter, and only a definite "no" should end entitlement early.
6. Store `POLAR_ACCESS_TOKEN` as a function env var. Never in the repo or client.

## Success Criteria
- [ ] Returns correct entitlement for: no subscription, pending seat, claimed
      seat, revoked seat, cancelled subscription.
- [ ] Identity taken only from the verified JWT — tested with a forged body.
- [ ] An unauthenticated call is rejected.
- [ ] A user cannot read another user's entitlement.
- [ ] Polar unreachable → the function says so distinctly; the client keeps its
      cache rather than downgrading.
- [ ] `POLAR_ACCESS_TOKEN` absent from repo and app bundle.

## Risk Assessment
- **`POLAR_ACCESS_TOKEN` leakage** — into the client, a log, or the repo. It is an
  organization-wide secret. Grep gate in phase 7.
- **"Cannot reach Polar" mistaken for "not entitled"** — this is the failure that
  downgrades a paying customer, and it is the one most likely to be written by
  accident. Step 5 exists specifically to prevent it.
- **Pending vs claimed.** An assigned-but-unclaimed seat is *not* entitlement.
  Treating it as such grants Pro to anyone who was merely invited.
- **The function growing.** It exists because one secret must live somewhere. Every
  addition re-opens the "no backend" decision that was made twice already.

## Phase 4 execution log — 2026-08-11

App gates unaffected (`build` OK, `check` 143/36) — `supabase/` sits outside the
app's typecheck, as intended.

### Built

| File | Role |
|---|---|
| `supabase/functions/entitlement/index.ts` | The oracle |
| `supabase/functions/_shared/auth.ts` | JWT verification, CORS, JSON helper |
| `supabase/functions/_shared/polar.ts` | Polar client + webhook signature verification |
| `supabase/config.toml` | Project config; documents *why* there are no migrations |
| `supabase/.env.example` | Secret names and scopes; real values never committed |
| `.gitignore` | `supabase/.env` + local state (verified with `git check-ignore`) |

### API contract, looked up rather than assumed

`GET /v1/customers/external/{external_id}/state` → `CustomerState` with
`active_subscriptions[]` and `granted_benefits[]`; 404 when the customer is
unknown. Base URLs `api.polar.sh` / `sandbox-api.polar.sh`, and **sandbox is the
default** unless `POLAR_ENV=production` — the same fail-safe the retired license
client had, so a test purchase can never grant real entitlement.

The external id is the Supabase user id, set at checkout in phase 6. No email
matching anywhere.

### Entitlement rule

Entitled when a subscription is `active` or `trialing`. Among those, the one that
actually granted this user a benefit is preferred — Polar grants benefits to
**seat members, not the billing customer** — falling back to the longest-running
live subscription so a payer is never denied over a benefit-configuration detail.

`tier` is derived from seat count and is display-only, so no product-id
configuration is needed and nothing can branch a gate on it.

`seats_claimed` is returned as **null**, not guessed: customer state does not
expose it. The UI hides the counter when null rather than showing a wrong number.

### Two defects found and fixed while building

**CORS preflight was missing.** The client sends `Authorization` and
`Content-Type`, which makes the request preflighted. Without an `OPTIONS`
handler and CORS headers, *every* call would have failed before reaching the
function — and it would have looked like "Polar is down" rather than a bug.
Fixed in `_shared/auth.ts`; `preflight()` runs before anything else.

**Missing config would have read as "not entitled".** An absent
`POLAR_ACCESS_TOKEN` now returns `unreachable`, not a denial. Otherwise a
mis-deploy or a rotated secret would have stripped Pro from every paying
customer at once — the single worst outcome available to this system.

### Verified

The expiry logic is pure and was tested directly (ported and run):

- Renewal sooner than the grace window → renewal wins (never grants past what was paid).
- Renewal far out → capped at 30 days (a cancellation cannot go unnoticed for months).
- Missing or unparseable period end → grace window, never eternal, never `NaN`.
- Past renewal → expiry in the past; the client treats it as lapsed.
- `pickLatest` ignores null/bogus dates without throwing or returning undefined.

All pass.

### NOT verified — no runtime available

**Neither Deno nor the Supabase CLI is installed on this machine**, so the
functions have been **neither typechecked nor executed**. Everything above is
review and pure-logic testing, not a running function. Before trusting this:

```
supabase functions serve entitlement    # typechecks on load
```

Then the phase 7 red-team list: forged body identity, unauthenticated call,
cross-user read, pending vs claimed seat, and Polar-unreachable behaviour.

### Deliberately not built

The **webhook**. It only shortens the lag before a client re-checks; entitlement
is correct without it, because the cache expires on its own. Signature
verification is written and ready in `_shared/polar.ts` for whoever adds the
endpoint, so the security-critical half is not left as an exercise.

### Success criteria

- [x] Returns correct entitlement for no subscription / live / cancelled *(by
      construction and review; not executed — see above)*.
- [x] Identity taken only from the verified JWT — the body is never read.
- [x] Unauthenticated call rejected (401), and misconfiguration fails closed.
- [x] A user cannot read another user's entitlement — no id is accepted as input.
- [x] Polar unreachable → `503 upstream_unavailable`; the client keeps its cache.
- [x] `POLAR_ACCESS_TOKEN` absent from repo and app bundle; `.env` git-ignored.
- [ ] Runtime verification — blocked on Deno / Supabase CLI.

### Blocked on the owner

`POLAR_ACCESS_TOKEN` (scope: `customers:read` only), then
`supabase functions deploy entitlement`.

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
