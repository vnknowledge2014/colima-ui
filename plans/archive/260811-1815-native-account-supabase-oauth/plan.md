---
title: "Native Account — Supabase OAuth (login/signup/profile/badge)"
description: "Identity-only native account for ColimaUI via Supabase Auth (Google/GitHub OAuth). Login/signup/profile UI + a small user badge in the sidebar. Entitlement stays the Polar license key."
status: completed
priority: P2
branch: "dev"
tags: [account, auth, supabase, oauth, ui]
blockedBy: [260810-2258-colimaui-commercial-foundation]
blocks: []
created: "2026-08-11T11:16:33.113Z"
createdBy: "ck:plan"
source: skill
---

# Native Account — Supabase OAuth (login/signup/profile/badge)

## Context

Detailed execution plan for **Phase 8** of `260810-2258-colimaui-commercial-foundation`
(that phase file is the high-level entry; this plan is the buildable detail).

The owner wants a native in-app account (login + signup + profile) and a small user
badge, like OrbStack. It is a **deliberate override** of the earlier "no backend"
decision — accepted, using a BaaS (Supabase) so no auth server is written or run.
See "Ghi đè quyết định — Tài khoản" in the commercial-foundation `plan.md`.

**The single most important boundary:** the account is **identity only**. "Has the
user paid?" stays the **Polar license key** (already built in the commercial plan's
Phase 2, `src-tauri/src/license/`), judged offline-first. Sign-in unlocks **no
feature**. Account down / signed out / offline **never** downgrades a paying
customer, because entitlement does not travel through the account.

## Locked decisions

| | |
|---|---|
| BaaS | **Supabase Auth** |
| Methods | **OAuth Google + GitHub only** — "signup" = first login; no email/password |
| Badge | **Identity only** (avatar + name) in the **sidebar footer**; no Pro dot |
| First launch | **No login wall.** `SetupWizard` stays first; sign-in is skippable, non-blocking |
| Secrets | **None in app.** Only public `SUPABASE_URL` + **anon key** + PKCE |
| Session | Stored in **macOS Keychain**, never plaintext on disk |
| Redirect | Decided at build: **try deep-link `colimaui://auth-callback` first, fall back to loopback** (reusing `api_server.rs`) |
| Profile | **Read-only** (avatar/name/email/provider from OAuth). No in-app editing → **no Supabase tables**, so RLS is a non-issue |
| First launch nudge | **Pure opt-in.** No sign-in prompt at all; the user signs in from the badge/Settings when they choose |

## SECURITY — what the app uses, and what it must never touch

The owner supplied a Postgres connection string
(`postgresql://postgres:[PASSWORD]@db.nakwwkderqjfsxajduwj.supabase.co:...`). That is
a **server-side secret** and must **NEVER** be embedded in the desktop app, committed,
or pasted where it can be logged. The app is a public client and uses only:

- `SUPABASE_URL` = `https://nakwwkderqjfsxajduwj.supabase.co` (derived from the project
  ref `nakwwkderqjfsxajduwj` — public).
- `SUPABASE_ANON_KEY` = the **anon/public** key from Supabase → Project Settings → API
  (public; safe to embed, like `POLAR_ORG_ID`). **Still needed — provide this.**

The Postgres password / connection string has **no role** in this plan. If a small
backend is ever built (e.g. the optional Supabase-user↔Polar-customer glue), that
secret lives only in that backend's server env, never in the client.

## Phases

| Phase | Name | Status |
|-------|------|--------|
| 1 | [Supabase & Desktop OAuth Callback](./phase-01-supabase-desktop-oauth-callback.md) | Completed |
| 2 | [Account State & Sign-in UI](./phase-02-account-state-sign-in-ui.md) | Completed |
| 3 | [Profile Page & User Badge](./phase-03-profile-page-user-badge.md) | Completed |
| 4 | [Keychain Session & Entitlement Boundary](./phase-04-keychain-session-entitlement-boundary.md) | Completed |
| 5 | [Verify & Harden](./phase-05-verify-harden.md) | Completed |

## Reuse (don't rebuild)

- `src/lib/external-links.ts` `openExternal()` — open the OAuth browser flow + portal.
- `src/lib/pro.svelte.ts` / `src/lib/api/license.ts` — entitlement source; the account
  layer must stay separate from it.
- `src/components/ConfirmDialog.svelte` / `store/upgrade.svelte.ts` — store-driven
  dialog pattern for the sign-in panel.
- Sidebar footer region in `src/components/Sidebar.svelte` — where the badge mounts.

## Acceptance Criteria

- [ ] Sign in with Google and GitHub natively in-app via Supabase; no secret in bundle.
- [ ] Skippable; Free fully usable when signed out; no first-launch login wall.
- [ ] User badge in sidebar footer (avatar+name; collapsed→avatar; signed-out→"Sign in").
- [ ] Profile page (Settings → Account): avatar/name/email/provider, sign out, manage link.
- [ ] Session token in Keychain, not plaintext; no client secret; no DB string in app.
- [ ] **Entitlement boundary proven**: signed out / account down / offline → a licensed
      user stays Pro (test).
- [ ] BYOK unchanged; no new AI gateway.

## Dependencies

- **blockedBy** `260810-2258-colimaui-commercial-foundation` — needs the Polar license
  entitlement (Phase 2) to exist as the separate "paid" source of truth this plan
  keeps the account away from.
- **External artifacts (owner):** Supabase project with Google + GitHub providers
  enabled and the desktop redirect URL allow-listed; the **anon public key**.

## Red Team Review — 2026-08-11

Adversarial pass (security + failure-mode). All accepted; applied to phases.

| # | Finding | Sev | Applied |
|---|---|---|---|
| 1 | **Deep-link hijack:** another macOS app can register `colimaui://` and intercept the callback `code` | High | Phase 1: rely on PKCE (code useless without verifier) **and** verify the OAuth `state` param; treat deep-link as untrusted transport |
| 2 | **PKCE `code_verifier` must be stored securely** — not just the tokens. In webview localStorage it is exposed to XSS / a local reader | High | Phase 4: the secure-storage adapter must cover the PKCE verifier too, not only access/refresh tokens |
| 3 | **Loopback `/auth-callback`** must be reachable without the API bearer token (a browser redirect has none) yet must not become an open redirect or leak the code | High | Phase 1: if loopback chosen, the route accepts only the expected `state`, responds with a bare "you can close this", and forwards the code internally |
| 4 | **Anon key + RLS:** the public anon key can read whatever Row Level Security allows | Med | This plan: auth-only, **create no tables with permissive RLS**; if tables are added later, RLS is mandatory. Noted in SECURITY section |
| 5 | **Browser mode must be decided, not deferred** — otherwise the sign-in buttons are dead there | Med | Phase 5: decide + surface honestly; if unsupported in browser mode, disable with a clear note |
| 6 | **Token refresh offline** flaps the badge to signed-out on transient network loss | Low | Phase 3/4: treat a failed refresh as "keep last known identity, retry"; never let it touch entitlement |
| 7 | **Keychain unavailable / denied** → session can't persist | Low | Phase 4: degrade gracefully (re-login each launch), never crash |

**Scope note (accepted):** this feature is P2, post-launch, and unlocks no feature —
it is the owner's explicit override (identity/native-feel). Opportunity cost flagged;
build order remains behind revenue-path phases.

## Validated (2026-08-11)

- **Profile is read-only** — no editing, no Supabase tables. Kills the RLS surface
  (red-team #4 becomes moot as long as no table is added).
- **First launch = pure opt-in** — no nudge, no prompt; sign-in only from badge/Settings.
- **Redirect = decided at build** — deep-link first, loopback fallback (Phase 1).

## Open Questions

1. Optional Supabase-user ↔ Polar-customer glue (auto-fetch license after sign-in) —
   introduces a small backend. Out of scope here; evaluate separately if wanted.
2. Keychain mechanism (crate `security-framework` / `keyring` / `tauri-plugin-stronghold`) —
   implementation detail, decide at Phase 4.

## Implementation status — 2026-08-11

Phases 1–4 are built and their automated gates pass. Phase 5 is green on
everything that can run without live credentials and blocked on the rest.

**Redirect decision (Phase 1 A/B):** **A — deep-link `colimaui://auth-callback`.**
`tauri-plugin-deep-link` registers the scheme; the loopback fallback was not
built, so `api_server.rs` is untouched by this feature and gained no
unauthenticated route.

Red-team items as applied:

| # | Where it landed |
|---|---|
| 1 | `src/lib/account-oauth.ts` — a per-attempt `state` nonce rides the redirect and must match on return; PKCE makes the code useless alone |
| 2 | `src/lib/supabase.ts` — the storage adapter routes **every** supabase-js auth key to the keychain, PKCE `code_verifier` included, not just tokens |
| 3 | N/A — no loopback route was added (decision A) |
| 4 | No Supabase tables created; profile is read-only, so there is no RLS surface |
| 5 | Decided: sign-in is desktop-only, stated in the UI instead of dead buttons |
| 6 | `account.svelte.ts` — a failed refresh keeps the last known identity; only an explicit `SIGNED_OUT` clears it |
| 7 | `account_session.rs` — no keychain backend or a denied prompt degrades to an in-memory session; the profile says it will not persist |

**Still open:** the owner's three Supabase artifacts (anon key, providers,
redirect allow-list) — see phase 5. No code change is expected once they land,
only the one-line key.

## Closed — 2026-08-11

All five phases complete. OAuth confirmed working end-to-end with the owner's
Supabase project. Setup documented in `docs/account-supabase-oauth.md`.

Acceptance criteria: all met. The two "unknowns" flagged at implementation time
both resolved in practice — Supabase does append `&code=` to a `redirectTo` that
already carries a query string, so the `state` nonce survives the round-trip.

Not covered by this plan, deliberately: subscriptions, team plans, and any
Supabase table. Those cross the identity/entitlement boundary this plan was built
to protect and belong to a separate, explicitly-scoped plan.

## Superseded in part — see 260811-1930-subscription-teams-supabase-schema

This plan's central rule — *entitlement never routes through the account* — is
**deliberately reversed** by the subscriptions/teams plan, at the owner's
decision. License keys are retired; Pro becomes a Polar seat subscription
resolved through the signed-in account.

What survives: the OAuth flow, the keychain session, PKCE + `state`, the badge and
the read-only profile. What does not: the identity/entitlement wall, and the
in-app copy claiming an account unlocks nothing.

`docs/account-supabase-oauth.md` is accurate as of today and becomes wrong when
phase 5 of that plan lands. It is listed there for correction.
