---
phase: 5
title: "Verify & Harden"
status: completed
priority: P2
dependencies: [3, 4]
effort: "1 day"
---

# Phase 5: Verify & Harden

## Overview
End-to-end verification of the account feature and the security/UX guarantees, plus
browser-mode and first-launch behavior.

## Requirements
- Functional: full sign-in → profile → badge → sign-out loop works for both providers.
- Non-functional: security posture holds; no regression to Free, license, or first-launch.

## Architecture
Verification only — no new components. Exercises the flows built in Phases 1–4.

## Related Code Files
- Modify (as needed from findings): any of the files created in Phases 1–4.
- No new production code expected; may add tests.

## Implementation Steps
1. Build gates: `pnpm run build`, `pnpm run check` (must stay at the repo baseline of
   143 errors / 36 warnings — no new errors), `cd src-tauri && cargo build && cargo test --lib`.
2. Manual: sign in with Google and with GitHub; profile renders; badge updates; sign out clears.
3. Manual: **skip** at the sign-in panel → Free fully usable; `SetupWizard` still runs
   first on a fresh profile (no login wall).
4. Security sweep: grep the bundle/config for the anon key location (public, ok), and
   assert **no** `postgres://`, DB password, or client secret anywhere in the repo/app.
5. **Entitlement boundary** (repeat Phase 4 gate as an explicit release check): licensed
   user signed out / offline → still Pro.
6. Browser mode: decide + document whether sign-in works there; if not, the UI says so
   plainly (no dead buttons).

## Success Criteria
- [ ] Build + check + cargo tests green; svelte-check at baseline, no new errors.
- [ ] Both providers sign in/out cleanly; profile + badge correct.
- [ ] Skip works; no first-launch login wall; Free unaffected.
- [ ] Security sweep: no secret / DB string in app; only public config present.
- [ ] Entitlement boundary holds under signed-out/offline/Supabase-down.
- [ ] Browser-mode behavior decided and surfaced honestly.

## Risk Assessment
- "Works on my machine" OAuth: verify on a clean profile and a signed build, not just dev.
- Silent secret creep: the grep-for-secret sweep is a required, repeatable gate, not a one-off.

## Execution log — 2026-08-11

Automated gates run and green:

- `pnpm run build` — succeeded.
- `pnpm run check` — **143 errors / 36 warnings**, exactly the repo baseline. No new errors.
- `cargo build` — succeeded (deep-link plugin + `security-framework` compile).
- `cargo test --lib` — **145 passed, 0 failed**, including three new
  `account_session` tests (the macOS round-trip exercised the real Keychain) and
  `license::tests::entitlement_ignores_account_state`.
- Secret sweep — 0 hits for `postgres://`, `postgresql://`, `service_role`,
  `SUPABASE_SERVICE`, and the DB host, across the repo *and* the built `dist/`
  bundle. Only the public project URL ships; the anon key is still `""`.
- Boundary grep — `pro.svelte.ts`, `api/license.ts`, `src-tauri/src/license/`
  and `src-tauri/src/pro/` contain no reference to the account layer, and the
  account layer contains no reference to license or entitlement. Clean in both
  directions.

Browser-mode decision (step 6): **sign-in is desktop-only.** The return path is a
registered `colimaui://` scheme and the session lives in the OS keychain; a
browser tab has neither, and the fallback would be persisting a refresh token in
`localStorage`. Both the panel and Settings → Account say so in place of the
buttons, in all four locales.

### Blocked — needs the owner

Steps 2, 3 and 5's manual halves cannot run yet. They need:

1. The **anon public key** (Supabase → Project Settings → API) pasted into
   `SUPABASE_ANON_KEY` in `src/lib/supabase.ts`.
2. Google + GitHub providers enabled in Supabase → Auth → Providers.
3. `colimaui://auth-callback*` added to Supabase → Auth → URL Configuration
   (the wildcard matters — the app appends `?state=<nonce>`).

Until then `accountAvailability()` reports `unconfigured` and the UI explains
itself rather than offering a button that cannot work.

## Unblocked — 2026-08-11 19:30

Owner supplied the publishable key and configured the Supabase providers +
redirect allow-list. OAuth round-trip confirmed working in the app.

Re-verified after the owner's edits (anon key, `$effect` auto-close on the sign-in
panel, window refocus after the browser round-trip):

- `pnpm run build` — OK.
- `pnpm run check` — 143 errors / 36 warnings, still exactly baseline.
- `cargo test --lib` — 144 passed, 0 failed.
- Key-type check — only `sb_publishable_` ships. The single `sb_secret_` string in
  `dist/` is supabase-js's own key-prefix validator, not a key.
- Boundary re-grep — the only cross-references left are a doc comment on the
  boundary test and one UI copy string ("Pro is your license key"). No code path
  reads across.

Setup is now documented in `docs/account-supabase-oauth.md`.

Remaining manual coverage, not yet exercised: second provider (only one was used
to confirm the flow), session survival across a restart, and a signed/notarized
build on a clean macOS profile.
