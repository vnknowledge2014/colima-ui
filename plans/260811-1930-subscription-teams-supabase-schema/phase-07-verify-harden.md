---
phase: 7
title: "Verify & Harden"
status: partial
priority: P1
dependencies: [5, 6]
effort: "1-2 days"
---

# Phase 7: Verify & Harden

## Overview
Prove the security and entitlement guarantees rather than assert them. This phase
assumes the attacker has the anon key and a valid account — because both are
trivially obtainable.

## Requirements
- Functional: the whole loop works for Personal and for Teams, both providers.
- Non-functional: no forgery path, no secret shipped, no silent downgrade.

## Implementation Steps

### 1. Standard gates
- `pnpm run build`, `pnpm run check`, `cargo build`, `cargo test --lib`.
- Baseline was **143 errors / 36 warnings** before this plan. It will legitimately
  move: this plan deletes files (`License.svelte`, `license/`) and adds others.
  Re-measure on a clean checkout at the start of Phase 1 and compare against
  *that*, not against 143 — a stale number either hides a new error or blocks on
  one that was never there.
- Migrations apply cleanly from an empty database, and reverse.

### 2. Red-team the data layer — **mostly not applicable**
Phase 3 is cancelled: there are no application tables, so there is no RLS surface
and no client write path. Confirm that is still true rather than assuming it:

- [ ] `supabase/migrations/` does not exist, or contains no application tables.
- [ ] The app makes no Supabase data reads or writes — auth calls only.

### 3. Red-team the entitlement oracle
Call the function **directly**, with a valid token for a different user.

- [ ] Body-supplied `user_id` overriding the JWT identity → ignored.
- [ ] Unauthenticated call → rejected.
- [ ] User A cannot read user B's entitlement.
- [ ] Assigned-but-unclaimed (`pending`) seat → **not** entitled.
- [ ] Revoked seat → not entitled.
- [ ] Polar unreachable → distinct from "not entitled"; client keeps its cache.
- [ ] Unsigned webhook (if built) → rejected.

### 4. Entitlement boundary (the new invariant)
- [ ] **Supabase fully unreachable → Pro unaffected.** Block the host and confirm.
- [ ] Offline past `expires_at` → Free; before it → Pro.
- [ ] Tampered cache → fails closed to Free.
- [ ] Cancelled subscription → Free after the next successful check.
- [ ] Sign out → still Pro until expiry (the decided behavior).
- [ ] Sign out, then sign in as a **free** account → **Free**, not Pro. This is
      the bypass the per-user cache key exists to close; test it directly.
- [ ] Removed team member → Free after cache expiry (document the actual window
      observed; it is the enforcement lag).

### 4b. The purchase actually grants Pro
The failure this catches is silent and total: money taken, nothing unlocked.

- [ ] Buy Pro (1 seat) end to end → the buyer holds a **claimed** seat and is Pro.
- [ ] Buy Pro Teams → buyer is Pro, and remaining seats are assignable.
- [ ] If a custom `success_url` is ever introduced, re-run both — it disables
      Polar's automatic self-claim.

### 5. Secret sweep
Repeatable gate, not a one-off. Across repo **and** built bundle:
`POLAR_ACCESS_TOKEN`, `service_role`, `sb_secret_`, `SUPABASE_SERVICE`,
`postgres://`, `postgresql://`, the DB host, and the Polar webhook secret.

`POLAR_ACCESS_TOKEN` is the highest-value secret in the product — it is
organization-wide, not per-customer.

Note: supabase-js contains the literal string `sb_secret_` in its own key-prefix
validator. Match on assignment, not mere presence, or the gate cries wolf.

### 6. Migration safety
- [ ] Applies from empty; reverses.
- [ ] No destructive statement runs unreviewed.
- [ ] RLS enabled in the same migration as each table's creation.

### 7. Docs
- [ ] `docs/account-supabase-oauth.md` corrected — its identity-≠-entitlement
      section is false after Phase 5.
- [ ] `docs/account-supabase-oauth.md` documents the one server component.
- [ ] `docs/pricing-rationale.md` reflects the final tiers.
- [ ] `docs/telemetry.md` covers invite-email PII and retention.

## Success Criteria
- [ ] All gates green; svelte-check at baseline.
- [ ] Every red-team item above refused or behaving as specified.
- [ ] Supabase-down leaves entitlement untouched, proven.
- [ ] No secret in repo or bundle.
- [ ] Migrations reversible; docs true.

## Risk Assessment
- **"Works in the app" proving nothing.** The app is a cooperative client; every
  data-layer test must use a raw client to mean anything.
- **Enforcement lag is real and must be stated**, not discovered by the owner
  later: a removed member keeps Pro until their cache expires.
- **Gate decay.** The secret sweep and the Supabase-down test are worth nothing as
  one-off checks; wire them into the release checklist.

## Phase 7 execution log — 2026-08-11 (PARTIAL)

Everything that can run without live products, a deployed function and a real
payment has been run. The rest is listed as blocked, not quietly skipped.

### Passed

**1. Standard gates**

| Gate | Result |
|---|---|
| `pnpm run build` | OK |
| `pnpm run check` | **143 errors / 36 warnings** — baseline, no new |
| `cargo build` | 0 errors |
| `cargo test --lib` | **151 passed, 0 failed** |

Baseline note from the phase-validation pass: 143/36 was re-measured after this
plan added and deleted files, and lands on the same number it started at.

**Edge Functions typechecked** — first with a tsc shim, then properly. See the
Deno section below: the shim passed something `deno check` rejected.

Original shim approach (superseded): the repo's own `tsc` under `strict`, plus a
minimal ambient shim for `Deno.env` / `Deno.serve` / the npm: specifier.
Confirmed via `--listFiles` that all four files were actually processed —
an empty file set would also have produced "no errors":

```
supabase/functions/_shared/auth.ts
supabase/functions/_shared/polar.ts
supabase/functions/checkout/index.ts
supabase/functions/entitlement/index.ts
```

**Zero type errors.** This catches syntax and type mistakes in our code; it does
**not** validate npm: resolution or Deno runtime behaviour.

**2. No application tables.** No `supabase/migrations/` directory; **0** Supabase
data calls in the app (`.from(` / `.rpc(`, excluding `Array.from`). Phase 3's
cancellation holds — there is nothing to attack.

**3. Function red-team — static proof.** The functions are not deployed, so these
are proven by reading rather than by calling:

| Property | Evidence |
|---|---|
| Identity never from the body | `entitlement` reads the body **0** times; `checkout` reads only `body.tier` and `body.seats`; **0** occurrences of `body.user_id`-style identity anywhere |
| Identity only from the token | Both call `getAuthedUser(req)` as their first act after preflight |
| Unauthenticated rejected | `json({ error: "unauthorized" }, 401)` in both |
| Unreachable ≠ not entitled | `503 upstream_unavailable` in the oracle; the client returns `null` on any non-2xx and changes nothing |
| No `success_url` | Only the comment explaining its deliberate absence |

**4. Entitlement boundary.** 15 subscription tests pass, covering expiry,
fail-closed on missing/unparseable dates, per-user isolation, sign-out keeping
Pro, and entitlement being a pure function of cache + clock.

**5. Secret sweep.** 0 hits across `src/`, `src-tauri/src/`, `dist/` and
`supabase/functions/` for `POLAR_ACCESS_TOKEN=`, `polar_oat_…`, `sb_secret_…`,
`service_role`, `postgres://`. `supabase/.env` is git-ignored and uncommitted.

**6. Migration safety.** N/A — no migrations exist.

**7. Docs.** All four setup/architecture docs present and indexed. The stale
"identity is never entitlement" claim is gone; **0** references to license keys
as a live mechanism.

### Blocked — needs owner artifacts

None of these are code problems; they need products, a token and a payment.

- Live purchase → **claimed seat** → Pro (the `success_url` trap test).
- Live purchase → the Polar customer carries the **Supabase user id as external
  ID** (the checkout-link defect found on 2026-08-11; this is the test that would
  have caught it).
- Calling the functions directly: forged body identity, cross-user read,
  unauthenticated call — all proven statically above, none exercised at runtime.
- `pending` vs `claimed` seat behaviour against real Polar data.
- Cancelled subscription → Free after the next check.
- Removed team member → Free after cache expiry (and the actual lag observed).
- `supabase functions serve` / `deno check` — the only thing that validates npm:
  resolution and Deno runtime behaviour.

### Residual gap worth stating

**A billing manager holding no claimed seat is currently treated as entitled.**
The oracle prefers a subscription that granted this user a benefit, but falls
back to any live subscription so that a payer is never denied over a
benefit-configuration detail. For 1-seat Pro that is correct (the buyer's seat is
auto-claimed). For a Teams buyer who deliberately holds no seat, it grants Pro to
someone who arguably should not have it.

That is an over-grant, not a security hole, and it errs in the safer direction —
but it should be confirmed against real data before launch, and tightened if the
fallback proves unnecessary once benefits are configured.

## Deno toolchain added — 2026-08-11 22:20

Installed Deno 2.9.5 via Homebrew (the official installer is a `curl | sh` pipe,
which this environment blocks and which is worse practice anyway).

### The shim was giving a false pass

`deno check` immediately found a type error the tsc shim had accepted:

```
TS2769: No overload matches this call.
  Argument of type 'Uint8Array<ArrayBufferLike>' is not assignable to 'BufferSource'
  at supabase/functions/_shared/polar.ts:133  (crypto.subtle.importKey)
```

`Uint8Array.from(...)` yields `Uint8Array<ArrayBufferLike>`, which admits
`SharedArrayBuffer` and is therefore rejected by `importKey`. Deno's lib types
make that distinction; the shim's DOM lib did not.

Fixed by allocating the buffer explicitly, which pins the type to
`Uint8Array<ArrayBuffer>`. In the webhook signature verification — currently
unused, so it would have failed the day someone deployed the webhook, not before.

**The lesson worth keeping:** a hand-rolled shim verifies the code against the
*shim*, not the runtime. It was better than nothing while no toolchain existed,
and it should not have been reported as equivalent.

### Now green

| Gate | Result |
|---|---|
| `deno check` (both functions) | clean |
| `deno lint` | clean, 4 files |
| `deno fmt --check` | clean, 5 files |
| `pnpm run build` | OK |
| `pnpm run check` | 143 / 36 — baseline |
| `cargo test --lib` | 151 passed |

### `supabase/functions/deno.json` added

- `fmt.lineWidth: 100` — Deno's default 80 reflowed readable expressions into
  fragments; 100 matches the rest of the repo's TypeScript.
- `lint.rules.exclude: ["no-import-prefix"]` — a deliberate deviation, with the
  reasoning in the file. The rule wants npm: dependencies declared in an import
  map and imported bare. Supabase's own Edge Function documentation uses the
  inline `npm:@supabase/supabase-js@2` form and it is guaranteed to resolve on
  their runtime; an import map introduces a deployment-time failure that would
  surface as *auth breaking in production*. The version is pinned inline, so the
  floating-version risk the rule guards against does not apply.

### Still not verified

`deno check` validates types, not behaviour. The functions have still never
executed. `supabase functions serve` (CLI not installed) and the live-purchase
tests remain blocked.

## Functions executed — 2026-08-11 22:30

Installed the Supabase CLI (2.113.0). `supabase functions serve` turned out to
require the full local stack (`supabase start` — ~10 containers), which is far
more than this needed: the functions are plain `Deno.serve` HTTP servers, so
`deno run --env-file` runs the real code directly.

`supabase/.env` holds only the project's **public** config (same URL and
publishable key that ship in the app). `POLAR_ACCESS_TOKEN` was left unset on
purpose. Git-ignored, and confirmed so.

### entitlement — 6/6 pass

| Test | Result |
|---|---|
| `OPTIONS` → 204 with `access-control-allow-headers: authorization, content-type` | PASS |
| No `Authorization` → 401 `{"error":"unauthorized"}` | PASS |
| Garbage bearer token → 401 | PASS |
| **Body claiming `{user_id, entitled:true}` with no token → 401** | PASS |
| `DELETE` → 405 | PASS |
| Error responses carry CORS headers | PASS |

### checkout — 5/5 pass

| Test | Result |
|---|---|
| `OPTIONS` → 204 + CORS | PASS |
| No token → 401 | PASS |
| Garbage bearer → 401 | PASS |
| **Body with `user_id` / `external_customer_id` naming another user → 401** | PASS |
| `GET` → 405 (POST-only) | PASS |

### What this actually proves

- **The CORS preflight fix is real.** It was written blind in phase 4 after
  reasoning that the `Authorization` header makes the request preflighted; the
  204 + correct `allow-headers` is now observed, not argued.
- **Identity cannot come from the body.** Previously a static claim backed by
  grep; now a forged body is demonstrably rejected on both endpoints.
- **JWT verification reaches real infrastructure.** The garbage-token case round-
  tripped to the live Supabase project and was rejected there — `getAuthedUser`
  works end to end, not just in principle.

Machine left clean: both processes stopped, no Supabase containers started.

### Superseded — the authenticated paths were reached after all

See "Authenticated paths verified" below: a local Supabase stack provides a real
auth server, so a genuine user JWT can be minted without completing OAuth. The
list below is kept for the record; most of it is now covered.

Previously believed blocked on a live sign-in plus Polar products:

- Entitled / not-entitled / cancelled mapping against real customer state.
- `pending` vs `claimed` seat.
- **`POLAR_ACCESS_TOKEN` unset → 503 `upstream_unavailable`, not a denial.** The
  code path is written and typechecked but sits behind the auth gate, so it could
  not be exercised here. This is the single most consequential untested branch:
  if it ever returned "not entitled" instead, every paying customer would be
  downgraded at once.
- Checkout session creation with `external_customer_id`.

## Authenticated paths verified — 2026-08-11 22:40

`supabase start` brings up a full local stack, including a real GoTrue. That
provides genuine, correctly-signed user JWTs without needing to complete OAuth —
which is what previously blocked every authenticated test.

Functions served through the real edge runtime (`supabase functions serve`), not
`deno run`, so the gateway, JWT verification and env injection are all exercised
as they will be in production.

**Env note:** `SUPABASE_URL` / `SUPABASE_ANON_KEY` must be left *unset* in
`supabase/.env`. The runtime injects them pointing at the stack's internal
address; overriding them with the remote project makes verification reject
locally-minted tokens. Worth knowing before anyone debugs that for an hour.

### Round 1 — `POLAR_ACCESS_TOKEN` unset (6/6 pass)

| Test | Result |
|---|---|
| **Valid JWT + no Polar token → 503 `upstream_unavailable`, NOT `{entitled:false}`** | PASS |
| Valid JWT authenticates (past 401 into real logic) | PASS |
| `GET` with valid JWT reaches logic | PASS |
| Valid JWT + forged body → still 503, body ignored | PASS |
| `checkout` + valid JWT + no product ids → 503 `not_configured` | PASS |
| A second, distinct user handled independently | PASS |

The first row is the one that mattered. Had it returned `200 {entitled:false}`,
the client would have cached a denial and **every paying customer would have been
downgraded at once** the moment a token was rotated, expired or mis-deployed.
It is now observed behaviour, not a design intention.

### Round 2 — deliberately invalid `POLAR_ACCESS_TOKEN` (3/3 pass)

This round makes real HTTPS calls to `sandbox-api.polar.sh`, which rejects the
bogus token with 401 — exercising the actual network path.

| Test | Result |
|---|---|
| Polar rejects our token → 503 `upstream_unavailable`, not a denial | PASS |
| `checkout` with rejected token → 502 `checkout_failed`, **no URL returned** | PASS |
| Hostile `tier` (`'; DROP TABLE--`) and `seats: 99999` → handled, no crash, no URL | PASS |

"Polar refused *us*" is correctly classified as "cannot determine" rather than
"this customer is not paying" — the distinction the whole design rests on.

### Machine left clean

`supabase stop` run; no containers, no processes, `supabase/.env` restored to
placeholders and still git-ignored.

### What genuinely remains

Only what needs real money and real products:

- A purchase → **claimed seat** → Pro.
- The purchase's Polar customer carrying the Supabase user id as external ID.
- `pending` vs `claimed` seat, cancellation, and removed-member behaviour against
  real customer state.
- The `not_found` → deny path (needs a valid Polar token and an unknown user).

Everything reachable without a live Polar account is now executed, not reasoned.
