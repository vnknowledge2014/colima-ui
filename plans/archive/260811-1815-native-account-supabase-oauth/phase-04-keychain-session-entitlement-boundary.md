---
phase: 4
title: "Keychain Session & Entitlement Boundary"
status: completed
priority: P2
dependencies: [2]
effort: "1-2 days"
---

# Phase 4: Keychain Session & Entitlement Boundary

## Overview
Persist the Supabase session securely in the macOS Keychain, and prove the hard wall
between account identity and Pro entitlement.

## Requirements
- Functional: session (access + refresh token) survives restart via Keychain; sign-out
  clears it; token refresh works.
- Non-functional: **no plaintext token on disk**; **entitlement never routes through the
  account** — a licensed user stays Pro when signed out / offline / Supabase down.

## Architecture
```
Supabase JS default persistence = localStorage (webview) → NOT acceptable for tokens.
Instead: custom storage adapter → Rust Keychain commands.

**The adapter must cover the PKCE `code_verifier` too (red-team #2), not only the
access/refresh tokens.** Supabase stores the verifier via the same storage interface
between the authorize request and the callback; leaving it in localStorage exposes it
to a local reader / XSS, defeating PKCE. Route ALL Supabase auth-storage keys to Keychain.

src-tauri/src/account_session.rs (or a keychain command module)
  keychain_set(key, value)  /  keychain_get(key)  /  keychain_delete(key)
     → macOS Keychain (security-framework crate or `security` CLI bridge)
src/lib/account.svelte.ts
  supabase client `auth.storage` adapter → invoke the Keychain commands
```
Entitlement stays in `src-tauri/src/license/` (Polar, offline-first). `pro.svelte.ts`
must keep reading entitlement from the license, not the account.

## Related Code Files
- Create: `src-tauri/src/account_session.rs` — Keychain get/set/delete Tauri commands.
- Modify: `src-tauri/src/lib.rs` — register the commands.
- Modify: `src-tauri/Cargo.toml` — Keychain crate if used (e.g. `security-framework`).
- Modify: `src/lib/account.svelte.ts` — Supabase `auth.storage` adapter → Keychain commands.

## Implementation Steps
1. Implement the Keychain command module (set/get/delete under a stable service name).
2. Wire a Supabase storage adapter that persists tokens via those commands.
3. Confirm session restore on restart; sign-out deletes the Keychain entry.
4. **Entitlement boundary test:** with a valid Polar license cached, sign out and go
   offline → confirm `pro.svelte.ts` / License still reports Pro. Add an automated test
   where feasible (the license side already has offline-first unit tests to mirror).

## Success Criteria
- [ ] Session persists across restart via Keychain; no token in plaintext on disk.
- [ ] Sign-out fully clears the session (Keychain + memory).
- [ ] **Proven**: signed out / offline / Supabase down does not downgrade a licensed user.
- [ ] No client secret and no DB connection string anywhere in the bundle.

## Risk Assessment
- Accidentally coupling entitlement to account: the highest risk. Mitigation: keep the
  account store free of any license read; the boundary test is a required gate.
- Keychain access prompts / entitlements: signed dev build may prompt; document the
  behavior and the service-name convention.
