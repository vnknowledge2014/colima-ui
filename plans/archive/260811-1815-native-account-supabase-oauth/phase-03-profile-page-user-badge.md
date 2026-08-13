---
phase: 3
title: "Profile Page & User Badge"
status: completed
priority: P2
dependencies: [2]
effort: "1 day"
---

# Phase 3: Profile Page & User Badge

## Overview
The signed-in surfaces: a profile card in Settings, and a small identity badge in the
sidebar footer.

## Requirements
- Functional: profile shows avatar/name/email/provider + sign out + "manage account"
  link; badge shows avatar+name (collapsed→avatar), or a "Sign in" affordance when out.
- Non-functional: **identity only** — no Pro/entitlement state on either surface.
- **Read-only** — the profile displays OAuth-provided fields; no in-app editing of name/
  avatar (no Supabase tables). Changing identity is done at the provider.

## Architecture
- Profile lives in Settings alongside `License.svelte` — the two are adjacent but
  independent: License = "paid" (Polar), Account = "who" (Supabase).
- Badge sits in the existing sidebar footer region (`Sidebar.svelte`), which already
  hosts status/clock/actions and has a `collapsed` variant to respect.

## Related Code Files
- Create: `src/pages/settings/Account.svelte` — profile card; reuse `openExternal()` for
  the Supabase account portal; sign out via `account.svelte.ts`.
- Create: `src/components/account/UserBadge.svelte` — avatar (fallback to initials) +
  name; signed-out → button opening `SignInPanel` (via `account-dialog` store);
  collapsed-aware.
- Modify: `src/components/Sidebar.svelte` — mount `UserBadge` in the footer block.
- Modify: `src/pages/Settings.svelte` — add `<Account />` next to `<License />`.
- Modify: `src/locales/{en,vi,zh,ja}.json` — `account.*` strings.

## Implementation Steps
1. `UserBadge.svelte` — read `accountState`; render avatar/name or "Sign in"; handle the
   `sidebar.collapsed` state (avatar only); keep it visually distinct from status rows.
2. Wire `UserBadge` into the sidebar footer.
3. `Account.svelte` — profile card + sign out + manage link; add to Settings.
4. i18n; ensure no Pro/entitlement wording appears on these surfaces.

## Success Criteria
- [ ] Badge shows the signed-in identity; collapsed shows avatar; signed-out shows "Sign in".
- [ ] Profile card shows avatar/name/email/provider, signs out, and links to the portal.
- [ ] Neither surface references Pro/license state.

## Risk Assessment
- Badge implying "signed in = Pro": deliberately omit any Pro indicator here; Pro lives
  only on the License page.
- Avatar load failure / no avatar_url: fall back to an initials circle; never a broken image.
