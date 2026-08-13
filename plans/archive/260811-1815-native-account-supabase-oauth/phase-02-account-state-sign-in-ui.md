---
phase: 2
title: "Account State & Sign-in UI"
status: completed
priority: P2
dependencies: [1]
effort: "1-2 days"
---

# Phase 2: Account State & Sign-in UI

## Overview
A small reactive account store wrapping the Supabase session, and a skippable,
non-blocking sign-in panel with Google + GitHub buttons.

## Requirements
- Functional: `signIn(provider)`, `signOut()`, `loadSession()`, `isSignedIn()`; the UI
  reads a normalized user `{ name, email, avatar, provider }`.
- Non-functional: the panel never blocks a Free action; "signup" is just the first
  OAuth login (no separate form). Failures fall back to signed-out silently.

## Architecture
```
src/lib/account.svelte.ts
  accountState = $state<{ user: AccountUser | null; loaded: boolean }>
  signIn(p)      → supabase.auth.signInWithOAuth(...)   (Phase 1 flow)
  signOut()      → supabase.auth.signOut() + clear Keychain (Phase 4)
  loadSession()  → supabase.auth.getSession() → normalize user_metadata
  isSignedIn()   → accountState.user != null
```
`AccountUser` maps `user_metadata`: `full_name`/`name`, `avatar_url`, `email`, and the
provider from `app_metadata.provider`.

## Related Code Files
- Create: `src/lib/account.svelte.ts` — the store + normalization.
- Create: `src/components/account/SignInPanel.svelte` — "Continue with Google" /
  "Continue with GitHub" + "Skip". Store-driven open/close (mirror `store/upgrade.svelte.ts`).
- Create: `src/store/account-dialog.svelte.ts` — `showSignIn()` / `closeSignIn()`.
- Modify: `src/App.svelte` — `loadSession()` on startup (next to `loadProStatus`/`loadLicense`); mount `<SignInPanel />`.
- Modify: `src/locales/{en,vi,zh,ja}.json` — `account.*` strings.

## Implementation Steps
1. `account.svelte.ts` — wrap the Supabase client from Phase 1; implement the four
   functions + `AccountUser` normalization; safe defaults on error.
2. `SignInPanel.svelte` — two provider buttons calling `signIn`, a prominent "Skip";
   dialog styling consistent with `ConfirmDialog`/`UpgradeDialog`.
3. `account-dialog.svelte.ts` store so any surface (badge, Settings) can open sign-in.
4. `App.svelte` — call `loadSession()` at startup; mount the panel globally.
5. i18n keys for all visible strings.

## Success Criteria
- [ ] Signing in via either provider populates `accountState.user`; sign-out clears it.
- [ ] The panel is skippable and never blocks; Free works fully when skipped.
- [ ] `loadSession()` restores an existing session on launch.

## Risk Assessment
- Store leaking entitlement: the account store must NOT read or expose license/Pro
  state — keep it purely identity. (Enforced again in Phase 4.)
- Reactivity: use Svelte 5 `$state`; do not name a variable `state` (collides with the rune).
