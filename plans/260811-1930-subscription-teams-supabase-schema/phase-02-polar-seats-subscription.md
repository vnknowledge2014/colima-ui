---
phase: 2
title: "Polar Seats & Subscription Model"
status: completed
priority: P1
dependencies: [1]
effort: "1-2 days"
---

# Phase 2: Polar Seats & Subscription Model

## Overview
Move Polar from license-key products to seat-based subscriptions, and define the
contract the rest of the system reads: *given a signed-in user, are they entitled,
and until when?*

## Requirements
- Functional: checkout for Personal (1 seat) and Teams (N seats); seat count
  changeable after purchase; cancellation and expiry reflected.
- Non-functional: Polar remains the **single source of truth for entitlement**.
  Nothing else may grant Pro. The app never trusts a client-supplied claim.

## Architecture
```
Polar subscription (product = tier, quantity = seats)
  │
  ├── customer  ←→  Supabase auth user      (linked by email, or Polar metadata)
  │
  ├── webhook: subscription.created/updated/canceled  →  Edge Function (Phase 4)
  │
  └── entitlement read: "is this user on an active seat, until when?"
```

**The linking question is the crux.** A Polar customer and a Supabase user must be
matched reliably. Email is the obvious key and the fragile one — a user may sign
in with GitHub under one address and pay with another. Decide here:

- **A** — pass the Supabase user id into Polar checkout **metadata** at purchase
  time and match on it. Robust; requires the user to be signed in *before* buying.
- **B** — match on email, with a manual reconciliation path for mismatches.
  Allows buying before signing in, but generates support load.

Recommend **A**, with sign-in required before checkout. It removes an entire class
of "I paid but it says Free" tickets.

## Related Code Files
- Modify: `src-tauri/src/license/client.rs` → becomes the subscription client.
  The HTTP shape, error mapping and `redact()` usage are reusable; the endpoints
  and response types change.
- Modify: `src-tauri/src/license/mod.rs` → entitlement resolution against a
  subscription rather than a key. Consider renaming the module to
  `subscription` in Phase 5, once nothing references the old name.
- Modify: `src/lib/external-links.ts` — `checkoutUrl()` / `portalUrl()` now carry
  the tier and the user-id metadata.
- Read: `docs/pricing-rationale.md` (Phase 1 output) for the exact products.

## Step 0 — verify Polar actually does seats (blocking)

**Every other phase assumes this and none of it was confirmed.** Before any work:
confirm Polar supports seat-based/quantity subscriptions, that seat count is
adjustable mid-term, and that the webhook payload exposes the quantity.

If it does not, the plan forks and the owner must choose: model seats as N
separate subscriptions, switch merchant-of-record, or keep per-member keys after
all. Finding this out in Phase 4, with the schema already built around seats,
is the expensive version of this discovery.

## Implementation Steps
1. Configure the Polar products per Phase 1's tier table (seat-based).
2. Decide and document the customer↔user linking strategy (A or B above).
3. Build checkout URL construction, carrying tier + Supabase user id metadata.
4. Define the webhook event contract Phase 4 will consume: which events, what
   fields, what each implies for entitlement.
5. Define the entitlement payload the client caches: `{ entitled, tier, seats,
   expires_at, subscription_id }` — the minimum needed to judge offline.
6. Decide the migration path for existing key holders (plan Open Question 3).

## Success Criteria
- [ ] **Polar seat support confirmed** before any other work in this phase.
- [ ] Personal and Teams purchasable; seat count adjustable post-purchase.
- [ ] Customer↔user linking decided, documented, and robust to a mismatched email.
- [ ] Webhook event contract written down before Phase 4 starts.
- [ ] Entitlement payload defined — sufficient to judge Pro entirely offline.
- [ ] Existing key holders have a decided, communicated path.

## Risk Assessment
- **Customer/user mismatch** is the top functional risk: a paying user seeing
  Free. Mitigated by strategy A; whichever is chosen needs a reconciliation path
  and a support runbook.
- **Sandbox vs production.** `api_base()` already defaults to sandbox unless
  `POLAR_ENV=production` — preserve that. A test purchase must never be able to
  grant real entitlement.
- **Seat downgrade semantics.** Reducing seats below current members must have
  defined behavior (refuse, or evict by a stated rule). Undefined here means an
  arbitrary member silently loses Pro later.
- **Proration and mid-term upgrades** — Personal → Teams mid-cycle. Polar handles
  billing, but the app must not show a stale tier afterwards.

## Step 0 result — 2026-08-11: verified, and it changes the plan

**Polar supports seat-based pricing natively, and does far more than this plan
assumed.** Source: `polar.sh/docs/features/seat-based-pricing`,
`.../customer-portal/navigate-customers`.

### Polar already *is* the roster

It models exactly the three entities phase 3 was going to create in Postgres:

| Polar entity | What it holds | The table it makes redundant |
|---|---|---|
| **Customer** (`type: "team"`) | Billing entity; upgraded automatically on first seat purchase | `teams` |
| **Member** | Person, own email, role `owner` \| `billing_manager` \| `member`; benefits granted per member | `team_members` |
| **CustomerSeat** | Product↔member link; status `pending` \| `claimed` \| `revoked`; holds the invitation token | `team_invites` |

Built in, with no code from us: invitation emails with claim links, resend,
revoke, seat reassignment, seat-count changes with prorated credit, and a
self-service seat management UI in the Customer Portal.

**Seats can be assigned by external customer ID** — which is the Supabase user id.
The phase's linking strategy A is therefore native, not something to engineer.

### The one thing Polar cannot do for us

Reading seat state requires `POLAR_ACCESS_TOKEN`, an **organization secret**.
`customerSessions.create()` is a server call. A desktop app holding only public
config cannot ask "does this user hold a claimed seat?" directly.

This is unlike the retired license-key flow, whose validate endpoint needed no
secret — which is precisely why keys worked with no backend at all.

So an Edge Function is still required, but for a **different job than phase 4
describes**: not writing roster rows, but acting as the *entitlement oracle* —
verify the caller's Supabase JWT, ask Polar about their seat, return
`{ entitled, expires_at }`. One function, holding one secret.

### Constraints found that the UI must respect

| Constraint | Consequence |
|---|---|
| **Billing manager receives no benefits** | A buyer who is not also assigned a seat gets **nothing**. See below — this is a trap |
| Default Polar confirmation page auto-claims the buyer's seat; a **custom `success_url` does not** | If we set our own success URL, a Pro (1-seat) buyer pays and stays Free. Either keep the default page, or assign the seat ourselves via API |
| Claim links expire in **24 hours** | Invite UX must make resending obvious |
| Max **1,000 seats**; metadata 10 keys / 1KB | Not binding for us |
| Seat reduction triggers a prorated credit | Billing handled by Polar; the UI should say so |

The `success_url` interaction is the sharpest edge in this phase: it silently
converts a successful purchase into no entitlement.

### Consequences for later phases

- **Phase 3 (schema, RLS, migrations): largely eliminated.** No teams,
  team_members or team_invites. That also removes the two High-severity RLS risks,
  the client-forges-membership risk, and the invite-email PII concern — those
  emails now live only at Polar, which is already the payment processor.
- **Phase 4: shrinks to one function** (entitlement oracle) plus an optional
  webhook to invalidate caches early.
- **Phase 6: team management becomes largely a deep link** into Polar's Customer
  Portal, rather than a roster UI we build and maintain.

Pending owner decision before phases 3/4/6 are rewritten.

## Phase 2 execution log — 2026-08-11

Step 0 verified (above). Owner decisions taken: **drop the Supabase roster
tables** (Polar owns the roster) and **deep-link team management into Polar's
portal**. Phases 3/4/6/7 and `plan.md` rewritten accordingly.

### Contracts defined (phase 4/5/6 consume these)

**Customer↔user link — decided: native external customer ID.**
Strategy A from the design, but no engineering needed: Polar assigns seats by
external customer ID, so we pass the Supabase user id at checkout and read seats
by the same value. Email matching (strategy B) is rejected — it was the source of
the "I paid but it says Free" support class.

Requires the user to be **signed in before checkout**. Accepted; that is also what
makes the seat assignable to them at all.

**Entitlement payload** — what the oracle returns and the client caches:

```jsonc
{
  "user_id":       "<supabase uuid>",   // per-user cache key (plan decision)
  "entitled":      true,
  "tier":          "pro" | "pro_teams",  // display only; gates read `entitled`
  "seats_total":   6,
  "seats_claimed": 4,
  "expires_at":    "2026-09-10T00:00:00Z"
}
```

`tier` is deliberately display-only. Phase 1 decided Teams unlocks no extra
feature, so no gate may branch on it — if one ever does, that is a tier system
arriving by the back door.

**Seat state → entitlement mapping** (phase 4 implements):

| Polar state | Entitled |
|---|---|
| No customer / no seat | No |
| Seat `pending` (assigned, unclaimed) | **No** |
| Seat `claimed`, subscription active | **Yes** |
| Seat `revoked` | No |
| Subscription cancelled or past due | No |
| Polar unreachable | **Unknown** — client keeps its cache; never a downgrade |

That last row is a distinct response, not an error the client guesses at.

**Webhook events** (optional, phase 4): `subscription.created` / `.updated` /
`.canceled`, plus seat assign/claim/revoke. Purpose is only to shorten the lag
before a client re-check — entitlement is correct without them.

### Constraints inherited from Polar

- Claim links expire in **24 hours**; resend is available in the portal.
- Max **1,000 seats**; seat metadata 10 keys / 1KB.
- Reducing seats issues a prorated credit — Polar handles it.
- Billing manager gets no benefits unless they also hold a seat.

### Success criteria

- [x] **Polar seat support confirmed** before any other work in this phase.
- [x] Customer↔user linking decided (native external customer ID) and robust to a
      mismatched email — the failure mode is designed out rather than mitigated.
- [x] Webhook event contract written down before phase 4 starts.
- [x] Entitlement payload defined — sufficient to judge Pro entirely offline.
- [x] ~~Existing key holders~~ — moot; nothing was ever sold (phase 1).
- [ ] **Owner task:** create the seat-based products in Polar per phase 1's tier
      table (Pro = 1 seat, Pro Teams = per-seat, min 2), and issue a
      `POLAR_ACCESS_TOKEN` for the Edge Function. Product IDs are needed before
      phase 6 can build checkout URLs.

### Unresolved

1. Offline cache window — 30 days proposed, still unconfirmed.
2. Whether to keep Polar's default confirmation page (safe) or set a custom
   `success_url` and assign the buyer's seat via API (nicer return-to-app UX, and
   the sharpest failure mode in the plan). Phase 6 decides; default is "keep
   Polar's page" until someone deliberately takes the risk.
