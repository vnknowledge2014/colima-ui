# Native Account (Supabase OAuth) — implementation report

**Date:** 2026-08-11 · **Branch:** `dev` · **Plan:** `plans/260811-1815-native-account-supabase-oauth/`

## Outcome

Phases 1–4 built; phase 5 green on every gate that can run without live
credentials, blocked on three owner-supplied Supabase artifacts.

## Decisions taken

| | |
|---|---|
| Redirect | **A — deep-link `colimaui://auth-callback`**. Loopback fallback not built, so `api_server.rs` gained no unauthenticated route. |
| Anon key | Placeholder `""`. `accountAvailability()` reports `unconfigured` and the UI explains itself rather than showing dead buttons. |
| Browser mode | **Sign-in is desktop-only.** It needs the registered URL scheme and the OS keychain; the alternative was a refresh token in `localStorage`. Stated in the UI, all four locales. |
| Keychain crate | `security-framework` under `cfg(target_os = "macos")` only. Other platforms degrade to an in-memory session rather than pulling in a dbus/secret-service dependency. |

## Files

**Created** — `src-tauri/src/account_session.rs`, `src/lib/supabase.ts`,
`src/lib/account-oauth.ts`, `src/lib/account.svelte.ts`,
`src/store/account-dialog.svelte.ts`, `src/components/account/SignInPanel.svelte`,
`src/components/account/UserBadge.svelte`, `src/pages/settings/Account.svelte`.

**Modified** — `src-tauri/{Cargo.toml,tauri.conf.json,capabilities/default.json}`,
`src-tauri/src/lib.rs`, `src-tauri/src/license/mod.rs` (boundary test),
`src/App.svelte`, `src/components/Sidebar.svelte`, `src/pages/Settings.svelte`,
`src/locales/{en,vi,zh,ja}.json`, `package.json`.

## Gates

- `pnpm run build` — OK.
- `pnpm run check` — **143 errors / 36 warnings**, exactly the repo baseline.
- `cargo build` — OK. `cargo test --lib` — **145 passed, 0 failed**.
- New tests: three in `account_session` (the macOS round-trip hit the real
  Keychain), plus `license::tests::entitlement_ignores_account_state`.
- Secret sweep — 0 hits for `postgres://`, `postgresql://`, `service_role`,
  `SUPABASE_SERVICE`, DB host — across the repo *and* the built `dist/`. Only
  the public project URL ships.

## Entitlement boundary

Clean in both directions by grep: `pro.svelte.ts`, `api/license.ts`,
`src-tauri/src/license/`, `src-tauri/src/pro/` name nothing in the account layer,
and the account layer names nothing in license/entitlement. `LicenseState::from_cache`
takes only the cache — there is no account value to pass it, so signing out
(which deletes the keychain entry) cannot reach entitlement.

## Defect found and fixed in self-review

`accountState.busy` was cleared only when a callback *succeeded*, so a rejected
one (state mismatch, cancelled consent, failed exchange) left the panel showing
"finish signing in in your browser" forever. The listener now fires on every
settled callback.

## Not verified

The live OAuth round-trip. It needs, from the owner:

1. **Anon public key** → `SUPABASE_ANON_KEY` in `src/lib/supabase.ts` (one line).
2. Google + GitHub enabled in Supabase → Auth → Providers.
3. `colimaui://auth-callback*` in Supabase → Auth → URL Configuration — the
   wildcard matters, the app appends `?state=<nonce>`.

Also unverified until then: both-provider sign-in/out, badge and profile against
a real session, session survival across restart, and a signed/clean-profile build
(dev-mode scheme registration can differ from a bundled app on macOS).

## Open question

Deep-link `state` transport assumes Supabase appends `&code=` to a `redirectTo`
that already carries a query string. Documented behavior, but it is the one thing
in the flow that only the first live sign-in can confirm. If it arrives without
`state`, the callback is rejected — a visible failure, not a silent insecure pass.
