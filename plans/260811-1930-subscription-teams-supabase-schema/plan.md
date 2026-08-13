---
title: "Subscriptions & Teams — account-based entitlement on Polar seats + Supabase roster"
description: "Retire license keys. Pro becomes a Polar seat-based subscription resolved through the signed-in account, cached offline. Supabase holds the team roster only, written exclusively by Edge Functions."
status: in-progress
priority: P1
branch: "dev"
tags: [billing, subscription, teams, supabase, polar, entitlement, migration]
# native-account-supabase-oauth đã xong và archive
# (plans/archive/260811-1815-native-account-supabase-oauth/).
blockedBy: []
blocks: [260811-2245-pro-tier-features]
created: "2026-08-11T12:30:00.000Z"
createdBy: "ck:plan"
source: skill
---

# Subscriptions & Teams

## Context

Two tiers today: Free ($0) and Pro ($6/mo), entitled by a **Polar license key**
cached locally and judged offline (`src-tauri/src/license/`). The account added in
`260811-1815-native-account-supabase-oauth` is identity-only and deliberately
unrelated to entitlement.

This plan **reverses that separation on purpose**, at the owner's decision:
license keys go away entirely, and Pro becomes a **seat-based Polar subscription
resolved through the signed-in account**. Pro is 1 seat; Pro Teams is N.

That is a real trade, accepted with eyes open:

| Lost | Gained |
|---|---|
| Key-based entitlement with no expiry cliff | One model instead of two; seats fit teams properly |
| "Never downgrades, ever" absolute guarantee | Buying, upgrading and seat changes all self-serve |
| `src-tauri/src/license/` (built + tested) | No key distribution/support burden |

## Locked decisions

| | |
|---|---|
| Entitlement source | **Polar subscription (seats)**, resolved for the signed-in account |
| License keys | **Removed everywhere.** No fallback path, no air-gap mode |
| Team roster | **Polar's own** Customer/Member/CustomerSeat model. **No Supabase tables at all** |
| Server surface | **One Edge Function** (entitlement oracle) holding `POLAR_ACCESS_TOKEN`. Nothing else |
| Team management | **Polar's hosted Customer Portal**, deep-linked from Settings. No roster UI in-app |
| Customer↔user link | **External customer ID = Supabase user id** — supported natively by Polar |
| Offline | Entitlement **cached with a long expiry (proposed 30 days)**; honoured offline until it lapses |
| Tiers | Free / **Pro** ($6/mo, 1 seat) / **Pro Teams** ($6/seat/mo, min 2) |
| Teams value | **Billing + management only.** Entitlement stays a boolean — no tier concept |
| Sign-out | **Keeps Pro until the cache expires.** No click sequence can downgrade a paying user |
| Cache key | **Per user id** — a second user signing in never inherits the first's entitlement |
| Cache storage | **File** (`~/.colima-ui/subscription.json`), like `license.json` today. **Not** the keychain — that is macOS-only and would break offline Pro elsewhere |

## The invariant that replaces the old one

The previous plan's rule — *entitlement never routes through the account* — is
now dead. Its replacement, which every phase must hold:

> **Entitlement comes from Polar and nowhere else**, resolved through one Edge
> Function, cached locally against the account, and honoured offline until expiry.
> Supabase provides identity only — it stores no application data whatsoever.

Since phase 3's cancellation this is close to self-enforcing: there is no
application table to be tempted into reading.

## Phases

| Phase | Name | Status |
|-------|------|--------|
| 1 | [Tier & Pricing Design](./phase-01-tier-pricing-design.md) | Completed |
| 2 | [Polar Seats & Subscription Model](./phase-02-polar-seats-subscription.md) | Completed |
| 3 | [Supabase Schema, RLS & Migrations](./phase-03-supabase-schema-rls-migrations.md) | **Cancelled** — Polar is the roster |
| 4 | [Entitlement Oracle](./phase-04-edge-functions-webhook-ingest.md) | Completed (unverified at runtime) |
| 5 | [Client Entitlement Rewrite](./phase-05-client-entitlement-rewrite.md) | Completed |
| 6 | [Subscription UI](./phase-06-subscription-teams-ui.md) | Completed |
| 7 | [Verify & Harden](./phase-07-verify-harden.md) | **Partial** — all offline gates pass; live-purchase tests blocked |

Phase 3 is cancelled and phases 4 and 6 shrank substantially — see "Phase 2
findings" below. Phase 4 is now the only security-critical phase, because it is
the only place a secret lives.

## Reuse (don't rebuild)

- `src-tauri/src/license/client.rs` — the Polar HTTP client, error handling and
  `redact()` usage survive the model change; only the endpoints move from
  license-key to subscription.
- The **cache pattern** in `src-tauri/src/license/cache.rs` — offline-first
  judgement with an embedded expiry is exactly what the new entitlement needs.
  Port the shape, change the payload.
- `src/lib/pro.svelte.ts`, `ProGate.svelte`, `UpgradeDialog.svelte` — the gate UI
  is model-agnostic; it consumes "entitled: bool" and does not care why.
- `src/lib/account.svelte.ts` + the keychain adapter — the session that
  entitlement now hangs off.
- `docs/account-supabase-oauth.md` — must be **corrected**, not duplicated: its
  "identity is not entitlement" section becomes wrong the day Phase 5 lands.

## Acceptance Criteria

- [ ] Buying Pro or Pro Teams in-app leads to Pro without any key being typed.
- [ ] Buying grants a **claimed** seat, not merely a purchase — the buyer is
      actually Pro afterwards.
- [ ] Team owner can invite, remove and see seat usage **via Polar's portal**,
      reached from Settings.
- [ ] An assigned-but-unclaimed seat does **not** confer Pro.
- [ ] A signed-in subscriber stays Pro offline until the cache expires.
- [ ] Entitlement oracle unreachable → cached Pro unaffected (test).
- [ ] A user cannot read another user's entitlement (explicit red-team test).
- [ ] No `POLAR_ACCESS_TOKEN`, `service_role` key, DB password or connection
      string in the app bundle.
- [ ] No Supabase application tables exist — confirmed, not assumed.
- [ ] License-key code and UI fully removed — no dead path, no stale copy.
- [ ] i18n copy corrected in all four locales (the "an account unlocks nothing"
      line is now false).

## Dependencies

- **blockedBy** `260811-1815-native-account-supabase-oauth` — entitlement now
  hangs off that session; it must exist and be reliable first. **Done.**
- **External (owner):** Polar seat support **verified** (phase 2 step 0), then the
  product configured for seat-based subscriptions;
  Polar webhook secret; Supabase CLI access for migrations; decision on the
  offline expiry window (30 days proposed).

## Risks

| Risk | Severity | Where handled |
|---|---|---|
| **Custom `success_url` voids the purchase** — Polar auto-claims the buyer's seat only on its own confirmation page; otherwise they pay and hold no seat | **High** | Phase 6 — keep Polar's page, or assign via API. Explicit phase 7 test |
| Edge Function trusts a client-supplied user id instead of the verified JWT | **High** | Phase 4 — identity taken only from the verified token |
| `POLAR_ACCESS_TOKEN` leaks into the client, a log, or the repo | **High** | Phase 4 env-only; phase 7 grep gate |
| "Polar unreachable" mistaken for "not entitled" → downgrades a payer | **High** | Phase 4 — the two must be distinct responses |
| ~~RLS misconfigured → anon key reads other teams' rosters~~ | — | **Eliminated:** no tables (phase 3 cancelled) |
| ~~Client forges membership by direct write~~ | — | **Eliminated:** nothing to write to |
| ~~Invite emails are the first PII stored~~ | — | **Eliminated:** they live at Polar, already the merchant of record |
| ~~Migration breaks a live project~~ | — | **Eliminated:** no migrations |
| Removed member keeps Pro until cache expiry | Medium | Accepted; Phase 5 documents the window. Shortening it trades UX for enforcement |
| Shared machine keeps Pro after sign-out | Medium | Accepted with the sign-out decision. Bounded by the expiry window |
| Second user inherits the first's cached Pro | **High** | Phase 5 — per-user cache key; explicit test in Phase 7 |
| ~~Users mid-flight on the old key model~~ | — | **Eliminated (phase 1):** nothing was ever sold, so there is nobody to migrate |

## Phase 2 findings — 2026-08-11 (major scope reduction)

Phase 2's blocking Step 0 confirmed Polar supports seat-based pricing — and found
it does considerably more, which removed the riskiest third of this plan.

**Polar already is the roster.** Customer (`type: "team"`) / Member (with roles) /
CustomerSeat (`pending` | `claimed` | `revoked`, with invitation tokens), plus
invitation emails, claim links, resend, revoke, reassignment, seat-count changes
with prorated credit, and a self-service portal. Seats assign by **external
customer ID** — the Supabase user id — so the customer↔user link is native.

| Was | Now |
|---|---|
| 3 Postgres tables + RLS + migrations | **Nothing.** Phase 3 cancelled |
| 6 Edge Functions on `service_role` | **1** entitlement oracle |
| In-app roster UI (invite/remove/seats) | Deep link to Polar's portal |
| Invite deep-link handling | Polar emails and hosts the claim |

**What Polar cannot do:** seat state needs `POLAR_ACCESS_TOKEN`, an organization
secret, so the desktop app cannot query it directly. That single fact is the
entire justification for keeping one Edge Function — unlike the retired
license-key endpoint, which needed no secret and hence no backend.

**Trap found:** the billing manager receives no benefits, and Polar auto-claims
the buyer's own seat *only* on its default confirmation page. A custom
`success_url` therefore turns a successful purchase into no entitlement, quietly.
Now a High risk with a dedicated phase 7 test.

## Phase validation — 2026-08-11

Checked each phase against the code before execution. Three corrections applied.

| # | Finding | Sev | Fix |
|---|---|---|---|
| 1 | Phase 5 said the entitlement cache belongs in the keychain, while its own "reuse" section said port the file-based `license/cache.rs`. `account_session.rs` is **macOS-only**, so the keychain route silently breaks offline entitlement on Linux/Windows — a regression against today's `~/.colima-ui/license.json`, and a direct contradiction of this plan's "stays Pro offline" criterion | **High** | Phase 5 — cache is a file; keychain is for tokens only. Rationale + a Linux-specific test added |
| 2 | Every phase assumes Polar supports seat-based subscriptions. Never verified | **High** | Phase 2 — blocking Step 0 before any other work |
| 3 | Phase 7 hard-coded the 143/36 baseline, which this plan legitimately changes by adding and deleting files | Low | Phase 7 — re-measure at Phase 1, compare against that |

Verified sound, no change needed:

- Dependency graph is acyclic and correctly ordered (1 → 2 → {3,4} → 5 → 6 → 7).
- `pro.svelte.ts` really is model-agnostic: `proState.paid` comes from a single
  `s?.entitled` read, so the swap is contained.
- **`ProGate` blast radius is 2 files**, not the wide audit Phase 5 implied —
  `compose/DiagnosePanel.svelte` and `pages/Compose.svelte`. Cheaper than feared.
- `license/cache.rs` already does atomic write + `.bak` recovery, so the port
  inherits torn-write safety rather than needing it rebuilt.

## Open Questions

1. **Offline expiry window** — 30 days proposed. Shorter enforces seat removal
   faster; longer is kinder to travelling users.
2. ~~Sign-out behavior.~~ **Decided 2026-08-11: sign-out keeps Pro until the
   cache expires.** A shared machine retaining Pro for the remainder of the window
   is the accepted cost. Per-user cache keying prevents a second user inheriting
   it — see phase 5.
3. ~~Existing key holders.~~ **Closed as moot (phase 1):** nothing has ever been
   sold — `CHECKOUT_URL`/`PORTAL_URL` empty, launch pending, UI reads COMING SOON.
   No migration path needed in phase 2.
4. ~~Teams pricing.~~ **Decided (phase 1): $6/seat, flat, minimum 2 seats.**
5. ~~Does Teams unlock features?~~ **Decided (phase 1): no — billing + management
   only, so entitlement stays a boolean and `ProGate` needs no edits.**

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
