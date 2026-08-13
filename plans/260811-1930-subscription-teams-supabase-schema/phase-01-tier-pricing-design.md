---
phase: 1
title: "Tier & Pricing Design"
status: completed
priority: P1
dependencies: []
effort: "0.5-1 day"
---

# Phase 1: Tier & Pricing Design

## Overview
Decide what Personal and Teams actually *are* before any schema exists. Everything
downstream — seat logic, RLS shape, whether the gate needs a tier concept —
depends on the answer.

## Requirements
- Functional: a written tier table (name, price, seats, what is included) that
  Phase 2 can configure in Polar verbatim.
- Non-functional: consistent with `docs/pricing-rationale.md`'s existing
  commitments — Free stays fully usable **including commercial use**, and AI stays
  BYOK at every tier. Those are published promises, not defaults to revisit.

## Decisions to make
1. **Teams price** — per-seat at $6 (same as Pro today), or volume discount?
2. **Does Teams unlock anything Personal lacks?** If purely billing + management,
   the entitlement stays a boolean and no tier concept enters the gate — a large
   simplification. If Teams has exclusive features, `pro.svelte.ts` needs a tier,
   and every `ProGate` call site must be audited.
3. **Minimum seats for Teams** — 2, 3, or 5. Affects the upgrade UX.
4. **Existing key holders** (plan Open Question 3) — honour to term, or convert?
5. **Naming.** "Pro" is the current word. With two paid tiers, is it
   Personal/Teams, or Pro/Teams? The word appears throughout the UI and i18n.

## Related Code Files
- Modify: `docs/pricing-rationale.md` — add the tier table and the *why* for each
  number, in the existing style. This file exists so a year from now a deliberate
  decision is distinguishable from an accident; keep that standard.
- Read: `src/lib/pro.svelte.ts`, `src/components/ProGate.svelte` — to judge the
  cost of introducing a tier concept before committing to one.
- Audit: `src/locales/{en,vi,zh,ja}.json` — inventory every string containing
  "Pro" or "license", since the vocabulary is about to change.

## Implementation Steps
1. Answer the five decisions above; record each with a one-line rationale.
2. Update `docs/pricing-rationale.md` with the tier table and reasoning.
3. Produce the string inventory (which keys change, which are removed) so Phase 6
   has no discovery work.
4. Confirm the Free tier's published promises are untouched.

## Success Criteria
- [ ] Tier table written and agreed; Phase 2 can configure Polar from it directly.
- [ ] Decided whether entitlement stays a boolean or gains a tier — with the
      blast radius counted, not guessed.
- [ ] `docs/pricing-rationale.md` updated, including the license-key retirement.
- [ ] i18n string inventory produced.

## Risk Assessment
- **Deciding schema before product.** Building tables for a tier model that then
  changes is the expensive mistake this phase exists to prevent. Nothing in
  Phase 3 starts until this is signed off.
- **Scope creep into a tier system nobody needs.** If Teams is billing-only, say
  so explicitly and keep the boolean — do not add tiers "for future flexibility".
- **Silent breach of a published promise.** Free-includes-commercial-use and
  BYOK-at-every-tier are in a public doc. Changing either is a separate decision,
  not a side effect of adding Teams.

## Phase 1 execution log — 2026-08-11

Decisions taken on the recommended defaults (owner did not object; each is a
one-line change if revisited). Recorded in `docs/pricing-rationale.md` with
full reasoning.

| Decision | Chosen | Consequence |
|---|---|---|
| Teams value | Billing + management only | **Entitlement stays a boolean.** No tier in the cache, no tier in the gates, `ProGate` untouched |
| Teams price | $6/seat, flat | No discount ladder in checkout or docs |
| Minimum seats | 2 | Lowest friction Pro → Teams |
| Naming | Free / Pro / **Pro Teams** | `pro.gate.*` and the PRO badge survive; no rename churn |

**Open Question 3 (existing key holders) is closed as moot.** `CHECKOUT_URL` and
`PORTAL_URL` are empty, launch is pending, and the UI still reads COMING SOON —
nothing has ever been sold. No migration, no refunds, no dual path in phase 2.

A stale premise was also corrected: `pricing-rationale.md` justified the
merchant-of-record choice by "needs built-in license keys". With keys dropped,
that criterion no longer applies; Polar is retained for subscriptions instead.

### i18n inventory (phase 6 input)

`en.json` holds **262** strings; 28 are paid-vocabulary. Per locale × 4 (en, vi, zh, ja).

**Delete — 16 keys.** The whole `license.*` block goes with the key UI:

```
license.activate
license.activated_inactive
license.activated_ok
license.activating
license.active
license.buy
license.deactivate
license.deactivated
license.enter_prompt
license.expires
license.inactive
license.key_placeholder
license.manage
license.not_configured
license.refresh
license.title
```

**Rewrite — 5 keys.** Two are factually wrong after phase 5, not merely dated:

| Key | Why |
|---|---|
| `account.signin.no_unlock` | FALSE after phase 5 — an account is now exactly what unlocks Pro |
| `compose.diagnose.autofix_ready` | says 'your Pro component'; re-check wording once entitlement is account-based |
| `settings.pro.badge` | 'COMING SOON' — must change when billing goes live |
| `settings.pro.desc` | describes Pro only; needs the Teams sentence |
| `settings.pro.title` | 'ColimaUI Pro' — becomes the Subscription section (phase 6) |

**Keep unchanged — 7 keys.** `pro.gate.*` is model-agnostic; it says
"this is Pro", never "this needs a key":

```
pro.gate.badge
pro.gate.default_desc
pro.gate.dismiss
pro.gate.free_note
pro.gate.get_pro
pro.gate.learn_more
pro.gate.needs_update
```

**New namespaces needed:**

- subscription.* — tier, seats used, renewal date, manage link
- team.* — roster, invite, remove, leave, seat counter, seat-limit refusal
- team.invite.* — accepted / expired / already-used outcomes
- plans.* — the Free / Pro / Pro Teams picker in UpgradeDialog

### Success criteria

- [x] Tier table written and agreed; phase 2 can configure Polar from it.
- [x] Entitlement stays a boolean — decided, with the blast radius counted
      (`ProGate`: 2 files, 0 edits needed).
- [x] `docs/pricing-rationale.md` updated, including the key retirement.
- [x] i18n string inventory produced (above).
- [x] Free tier's published promises verified untouched: commercial use still
      allowed, AI still BYOK at every tier.
