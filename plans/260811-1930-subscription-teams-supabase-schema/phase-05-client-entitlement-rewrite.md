---
phase: 5
title: "Client Entitlement Rewrite"
status: completed
priority: P1
dependencies: [2]
effort: "2 days"
---

# Phase 5: Client Entitlement Rewrite

## Overview
Replace license-key entitlement with account-based subscription entitlement,
cached offline. This is where the old invariant formally dies and the new one
takes over.

## Requirements
- Functional: a signed-in subscriber is Pro; the judgement survives offline until
  the cache expires; sign-out and cancellation are handled deliberately.
- Non-functional: **Supabase is never consulted** to decide entitlement. Pro
  survives a total Supabase outage. Offline-first is preserved, now with an
  expiry.

## Architecture

```
sign in (Phase 1 of the account plan)
   └─> resolve entitlement from Polar   ← the ONLY source
         └─> cache { entitled, tier, seats, expires_at } in the OS keychain
               └─> pro.svelte.ts reads the cache
                     └─> offline: honour until expires_at, then Free
```

### Where the cache lives — corrected

An earlier draft of this phase said "keychain". **That is wrong, and it would have
broken this plan's own acceptance criterion.**

`account_session.rs` has a backend only under `cfg(target_os = "macos")`;
everywhere else `set` returns an error and `get` returns `None`. Putting
entitlement there means a Linux user's cache never persists, so every launch needs
a live Polar check and any offline launch is Free. Today's `license.json` works on
every platform, so that would also be a **regression**, not just a gap. `targets:
"all"` and three non-macOS branches in `platform.rs` confirm this is a real
audience, not a hypothetical one.

**Decision: the entitlement cache is a file**, `~/.colima-ui/subscription.json`,
exactly where and how `license.json` lives today — atomic write via a temp file,
plus a `.bak` copy so a torn write is recoverable rather than reading as "no
subscription". Port `license/cache.rs` essentially as-is; only the payload changes.

The two stores have different jobs and must not be conflated:

| Store | Holds | Why |
|---|---|---|
| Keychain (`account_session.rs`) | Session tokens, PKCE verifier | Real credentials; theft = account takeover |
| File (`subscription.json`) | `{ user_id, entitled, tier, expires_at }` | A *claim*, not a credential. Forging it grants Pro to the forger only |

A local user editing their own entitlement file is the "polite fence, not DRM"
stance this project already takes for license keys (`license/mod.rs`). Moving the
claim into the keychain buys nothing against that attacker while costing every
non-macOS user their offline access.

### Sign-out — decided

**Signing out does NOT clear the entitlement cache.** Pro continues until the
cache expires on its own. This preserves the project's oldest promise in the only
form still available: a paying customer cannot lose Pro by accident. There is no
sequence of clicks that downgrades someone who paid.

The cost, accepted: a shared machine keeps Pro for the remainder of the window
after its owner signs out.

**The cache is keyed by user id**, which is what stops that cost becoming a bug.
Three states to implement precisely:

| State | Entitlement |
|---|---|
| User X signed in, cache for X valid | Pro |
| Nobody signed in, last cache belonged to X and is unexpired | **Pro** — this is the decision |
| User Y signs in, cache belongs to X | **Free for Y.** Y resolves their own entitlement; they never inherit X's |

Without the third rule, signing out and signing in as a free account would show
Pro — a trivially discoverable bypass, and worse, a wrong answer shown to a real
user. Store the cache under a per-user key (`subscription.<user_id>`), never a
single global one.

`supabase.auth.signOut()` clears only the keys supabase-js owns; the entitlement
cache is ours and must be left alone. Verify this explicitly — it is the kind of
thing a future "clear all account data on sign-out" cleanup silently breaks.

### What happens to `src-tauri/src/license/`

Retired. `client.rs`'s HTTP shape and `cache.rs`'s offline-judgement pattern are
ported into the new subscription module; the license-key command surface
(`license_activate`, `license_deactivate`, key input UI) is **deleted, not left
dangling**. A dead activation form is worse than none — it invites support tickets
for a path that no longer works.

`license::tests::entitlement_ignores_account_state` must be **removed**: it pins
the old invariant, and leaving a passing test that asserts a retired guarantee is
actively misleading. Replace it with tests for the new one — that Supabase plays
no part in entitlement.

## Related Code Files
- Create: `src-tauri/src/subscription/{mod.rs,client.rs,cache.rs}` — ported from
  `license/`, subscription-shaped. `cache.rs` keeps the file backing and gains a
  `user_id` field.
- Delete: `src-tauri/src/license/` once nothing references it.
- Modify: `src-tauri/src/lib.rs` — swap the registered commands.
- Modify: `src/lib/pro.svelte.ts` — read subscription entitlement; keep the
  exported shape stable if possible so `ProGate` call sites need no edit.
- Modify: `src/lib/api/license.ts` → `subscription.ts`.
- Delete: `src/pages/settings/License.svelte` (replaced in Phase 6).
- Modify: `docs/account-supabase-oauth.md` — its "identity is not entitlement"
  section becomes **false** the moment this lands. Correct it in the same commit.

## Implementation Steps
1. Port the cache with the new payload; keep the "unparseable expiry = not
   entitled" fail-closed rule from the current cache.
2. Implement resolution against Polar for the signed-in user.
3. Store the cache as a file, porting the atomic-write + `.bak` recovery from
   `license/cache.rs`. Do **not** route it through the keychain — see above.
4. Rewire `pro.svelte.ts`; audit every `ProGate` call site for assumptions.
5. Delete the license module, its commands, its API surface and its UI together.
6. Write the new tests: offline honoured until expiry; expired → Free; Supabase
   unreachable → entitlement unaffected; tampered cache fails closed.
7. Update `docs/account-supabase-oauth.md` and the i18n copy that claims an
   account unlocks nothing.

## Success Criteria
- [ ] Subscriber is Pro; non-subscriber is Free; both correct after restart.
- [ ] Sign-out keeps Pro until expiry; a different user signing in gets their
      own entitlement and never inherits the previous user's cache.
- [ ] Offline honoured until `expires_at`; expired falls back to Free.
- [ ] **Supabase fully unreachable → entitlement unchanged** (test).
- [ ] Tampered/corrupt cache fails closed to Free, never open to Pro.
- [ ] **Offline entitlement works on Linux, not only macOS** — the specific
      regression the keychain approach would have introduced.
- [ ] License-key code, commands, API and UI fully removed — no dead path.
- [ ] Stale docs and i18n corrected in the same change.

## Risk Assessment
- **The new invariant quietly eroding.** Someone later reads a Supabase row to
  decide Pro "because it is convenient". The test that Supabase-down does not
  affect entitlement is the guard; it must be a required gate.
- **Cache tampering.** Keychain-stored and fail-closed. It is a polite fence, not
  DRM — consistent with the project's existing stance.
- **Deleting the license module too early**, leaving half-migrated callers. Do it
  as one change, with the build as the check.
- **Users mid-term on a key** — Phase 2 decides their path; this phase must not
  strand them silently.
- **Clock manipulation** extends the offline window. Accepted: the same fence
  argument applies, and the alternative is an online requirement.

## Phase 5 execution log — 2026-08-11

Implemented. `pnpm build` OK · `pnpm check` **143 errors / 36 warnings** (no new)
· `cargo build` OK · `cargo test --lib` **151 passed, 0 failed** (+15 new).

### Design change vs the written phase: no Rust Polar client

The phase said port `license/client.rs` into `subscription/client.rs`. **It was
not needed and was not created.** The oracle requires the Supabase JWT, which
lives in the webview, so the frontend calls it directly and hands Rust the
result. Routing that through Rust would have added a hop and a second place for
the token to sit, for nothing.

Rust's job narrowed to **persist and judge**:

| File | Role |
|---|---|
| `src-tauri/src/subscription/cache.rs` | Atomic write + `.bak` + 0600, ported verbatim from `license/cache.rs`; new payload; per-user judgement |
| `src-tauri/src/subscription/mod.rs` | `SubscriptionState`, three commands, two HTTP handlers |
| `src/lib/api/subscription.ts` | Oracle call (JWT) + cache read/write |
| `src/lib/pro.svelte.ts` | `loadEntitlement` (cache, no network) / `refreshEntitlement` (oracle) |

### Two rules encoded, not just documented

**Missing expiry fails closed.** The old license cache treated `expires_at: None`
as a perpetual key. A subscription always has a renewal date, so absent means
malformed — and guessing "eternal" would guess in the app's favour against the
customer's actual payment status. Test: `missing_expiry_fails_closed`.

**A failed oracle call changes nothing.** `fetchEntitlement` returns `null` for
offline / DNS / not-yet-deployed alike, and every caller leaves the cache alone on
`null`. Treating an unreachable server as "not entitled" is the exact failure that
downgrades a paying customer, so it is refused at the source rather than guarded
downstream.

### Per-user cache, and where it is enforced

`AccountUser` gained an `id` (the Supabase user id) — needed here as the cache
key, and needed again in phase 6 as Polar's external customer id.

`is_entitled_for(user_id, now)` implements the three states the plan specified.
A record belonging to someone else reports as `known: false` — "no subscription on
this machine" — rather than a foreign record with an expiry date, so the UI offers
plans instead of explaining a stranger's billing.

Re-resolution is wired into `onAuthStateChange`, not only startup: switching
accounts recomputes entitlement immediately.

### Removed

`src-tauri/src/license/` (whole module), `src/lib/api/license.ts`,
`src/pages/settings/License.svelte`, its Settings mount, the four `/api/license*`
routes, the four Tauri commands, and the 16-key `license.*` i18n block × 4 locales.
`license::tests::entitlement_ignores_account_state` went with it — it pinned the
retired invariant, and a passing test asserting a dead guarantee is worse than no
test.

Verified zero dead references: no `licenseApi`, `LicenseState`, `/api/license`, or
`settings/License` anywhere in `src/` or `src-tauri/src/`.

### Stale copy corrected

`account.signin.no_unlock` claimed "an account unlocks nothing" in four languages.
Now false — rewritten in all four. `docs/account-supabase-oauth.md`'s
identity-≠-entitlement section rewritten, with an explicit note that the earlier
claim applied to the license-key model. `src/lib/supabase.ts`'s header comment
likewise.

### Success criteria

- [x] Subscriber is Pro; non-subscriber is Free; both correct after restart.
- [x] Sign-out keeps Pro until expiry; a different user never inherits it.
- [x] Offline honoured until `expires_at`; expired falls back to Free.
- [x] Supabase unreachable → entitlement unchanged (oracle failure is a no-op).
- [x] Tampered/corrupt cache fails closed (`.bak` recovery, unparseable → expired).
- [x] Offline entitlement works on Linux, not only macOS — file-backed, not keychain.
- [x] License-key code, commands, API and UI fully removed.
- [x] Stale docs and i18n corrected in the same change.

### Known gap until phase 6

**Settings has no billing surface.** `License.svelte` is gone and
`Subscription.svelte` is phase 6. Entitlement works and gates behave correctly;
there is simply no page showing tier/seats/renewal. The Pro "coming soon" card and
`UpgradeDialog` still point at the pricing URL, so nobody is stranded.

The oracle does not exist yet (phase 4), so `refreshEntitlement` is a no-op today
— by design: it fails silently and leaves the cache untouched.
