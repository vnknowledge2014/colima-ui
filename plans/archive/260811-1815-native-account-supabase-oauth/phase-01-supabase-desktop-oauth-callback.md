---
phase: 1
title: "Supabase & Desktop OAuth Callback"
status: completed
priority: P2
dependencies: []
effort: "1-2 days"
---

# Phase 1: Supabase & Desktop OAuth Callback

## Overview
Stand up the Supabase client (public config only) and the desktop OAuth return path,
so `signInWithOAuth` can complete a PKCE round-trip back into the app.

## Requirements
- Functional: browser opens the Supabase OAuth URL (Google/GitHub); after consent the
  provider redirects back to the app and the PKCE code is exchanged for a session.
- Non-functional: **no secret in the app** — only `SUPABASE_URL` + anon key + PKCE.
  The Postgres connection string / DB password is never used here or anywhere in the app.

## Architecture
```
signInWithOAuth(provider, { flowType: 'pkce', redirectTo })
   → openExternal(oauth_url)            (src/lib/external-links.ts, existing)
   → provider consent in system browser
   → redirect to  colimaui://auth-callback?code=...   (A: deep-link)
         or        http://localhost:<port>/auth-callback?code=...  (B: loopback)
   → app receives code → supabase.auth.exchangeCodeForSession(code)
   → session in memory (persisted to Keychain in Phase 4)
```
Decide **A (deep-link)** vs **B (loopback)** here:
- **A** — add `tauri-plugin-deep-link`, register scheme `colimaui`, add
  `colimaui://auth-callback` to Supabase redirect allow-list. Cleanest desktop UX.
- **B** — reuse the existing local HTTP server (`src-tauri/src/api_server.rs`): add a
  `/auth-callback` route that captures `code` and forwards it to the webview; register
  `http://localhost:<port>/auth-callback`. No new dependency; port must be stable.

Recommend **A** unless avoiding a new dependency matters more.

## Related Code Files
- Create: `src/lib/supabase.ts` — `createClient(SUPABASE_URL, SUPABASE_ANON_KEY, { auth: { flowType: 'pkce', detectSessionInUrl: false } })`; export constants (empty until provided).
- Modify (A): `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`, `src-tauri/capabilities/default.json` — deep-link plugin + scheme + permission.
- Modify (B): `src-tauri/src/api_server.rs`, `src-tauri/src/routes/` — `/auth-callback` route.
- Reuse: `src/lib/external-links.ts` `openExternal()`.
- Add dep: `@supabase/supabase-js` (npm).

## Implementation Steps
1. Owner: create Supabase project, enable Google + GitHub providers, add the chosen
   redirect URL to the allow-list, and copy the **anon public key**.
2. `pnpm add @supabase/supabase-js`; create `src/lib/supabase.ts` with public config
   (`SUPABASE_URL = https://nakwwkderqjfsxajduwj.supabase.co`, anon key filled in).
3. Implement the chosen callback (A deep-link or B loopback); route the `code` to the webview.
4. Wire `exchangeCodeForSession`; confirm a session object is obtained end-to-end.

## Success Criteria
- [ ] `signInWithOAuth` opens the browser and returns to the app with a valid session.
- [ ] Only public config in the app; no anon-key/secret in logs; no DB string anywhere.
- [ ] Redirect approach (A/B) chosen and documented; Supabase allow-list matches.

## Risk Assessment
- Redirect mismatch → auth fails silently. Mitigation: assert the exact allow-listed URL; log (redacted) failures.
- Deep-link scheme not firing on macOS → fall back to loopback. Keep both paths documented until one is proven.
- **Deep-link hijack (red-team #1):** another app may register `colimaui://` and intercept
  the `code`. Treat the deep-link as **untrusted transport**: rely on PKCE (the code is
  useless without the verifier) **and** verify the OAuth `state` param on return; reject
  a callback whose `state` does not match the one issued.
- **Loopback route (red-team #3):** must be reachable without the API bearer token, accept
  only the expected `state`, forward the code internally, and reply with a bare
  "you can close this window" — never an open redirect, never echo the code to the page.
