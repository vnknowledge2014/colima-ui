# Account — Supabase OAuth setup

How the in-app account works, and what has to be configured in Supabase for it to
work. Identity only: see [The boundary](#the-boundary-identity-is-not-entitlement)
before changing anything here.

## Identity and entitlement

The account answers **"who is signed in"**. Since license keys were retired, it
is also how Pro reaches a customer: entitlement is a Polar seat subscription
resolved for the signed-in user, cached in `~/.colima-ui/subscription.json` and
judged offline (`src-tauri/src/subscription/`).

> An earlier version of this document said the two were strictly separate and
> that signing in unlocked nothing. That was true of the license-key model and is
> no longer true. See `plans/260811-1930-subscription-teams-supabase-schema/`.

What still holds — and what the tests pin:

- **Supabase is never consulted to decide entitlement.** It provides identity and
  nothing else; the app stores no application data there. A Supabase outage
  cannot take Pro away.
- **An unreachable server is not "not entitled".** Only a definite answer from the
  entitlement oracle changes the cache. Offline, the cache stands until it expires.
- **Signing out does not revoke Pro.** It runs until the cache lapses, so no
  sequence of clicks downgrades someone who paid.
- **The cache is keyed by user id.** A different user signing in resolves their
  own entitlement and never inherits the previous one.

If you find yourself making Pro depend on a live network call, or on a Supabase
row, stop — those are the two failures this design exists to prevent.

## What ships in the app

Only public config, in `src/lib/supabase.ts`:

| Constant | Value | Secret? |
|---|---|---|
| `SUPABASE_URL` | `https://<project-ref>.supabase.co` | No — public |
| `SUPABASE_ANON_KEY` | `sb_publishable_…` | No — public by design |
| `AUTH_REDIRECT_URL` | `colimaui://auth-callback` | No |

The publishable/anon key is a public identifier, not a credential — the same
class of thing as `POLAR_ORG_ID`. PKCE is what makes an intercepted auth code
useless.

> **Never put these in the app, any config file, or any log:** the Postgres
> connection string, the database password, a `sb_secret_…` key, or the
> `service_role` key. They are server-side secrets with no role in this desktop
> client. `docs/telemetry.md` and the release checklist both grep for them.

## One-time Supabase configuration

> Step-by-step console guide (Google Cloud Console + GitHub + Supabase):
> [supabase-provider-setup.md](./supabase-provider-setup.md)

### 1. Providers

**Auth → Providers**, enable and fill in the OAuth app credentials for:

- **Google** — create an OAuth client in Google Cloud Console
  (APIs & Services → Credentials → OAuth client ID → *Web application*).
- **GitHub** — Settings → Developer settings → OAuth Apps → New OAuth App.

For **both**, the provider's own *Authorization callback URL* is Supabase's, not
ours:

```
https://<project-ref>.supabase.co/auth/v1/callback
```

The `colimaui://` scheme is only ever configured in Supabase, never at Google or
GitHub — they redirect to Supabase, and Supabase redirects to us.

### 2. Redirect allow-list

**Auth → URL Configuration → Redirect URLs**, add:

```
colimaui://auth-callback*
```

**The trailing `*` matters.** The app appends a `?state=<nonce>` query parameter
(see [Security](#security)), so an exact-match entry without the wildcard rejects
every real callback.

### 3. The key

Copy the **publishable** key from **Project Settings → API** into
`SUPABASE_ANON_KEY` in `src/lib/supabase.ts`. That is the only code change.

Until it is filled in, `accountAvailability()` reports `unconfigured` and the UI
explains itself instead of showing buttons that cannot work.

## How the flow works

```
signInWithOAuth(provider, { redirectTo: colimaui://auth-callback?state=<nonce> })
  → PKCE code_verifier written to the OS keychain
  → openExternal(authorize URL)         system browser, not the webview
  → user consents at Google / GitHub
  → provider  →  https://<ref>.supabase.co/auth/v1/callback
  → Supabase  →  colimaui://auth-callback?state=<nonce>&code=<code>
  → tauri-plugin-deep-link event
  → state compared against the stored nonce      ← rejected here if it differs
  → exchangeCodeForSession(code)
  → session written to the OS keychain
```

Consent happens in the **system browser**, never the app webview: Google refuses
embedded user-agents outright, and it keeps the user's existing browser session
available.

### Code map

| File | Role |
|---|---|
| `src/lib/supabase.ts` | Public config, availability check, keychain storage adapter |
| `src/lib/account-oauth.ts` | `state` nonce, authorize URL, deep-link listener, code exchange |
| `src/lib/account.svelte.ts` | `accountState` store, user normalization, sign in/out |
| `src/store/account-dialog.svelte.ts` | Opens the sign-in panel from anywhere |
| `src/components/account/SignInPanel.svelte` | The two-button offer |
| `src/components/account/UserBadge.svelte` | Sidebar footer identity |
| `src/pages/settings/Account.svelte` | Read-only profile |
| `src-tauri/src/account_session.rs` | Keychain get/set/delete commands |

## Security

### The deep-link is untrusted transport

Any macOS app can register the `colimaui://` scheme, so a callback may arrive
from something that is not the browser we opened. Two independent defenses:

1. **PKCE** — the authorization code is worthless without the `code_verifier`,
   which lives only in this app's keychain entry.
2. **`state` nonce** — minted per attempt, carried on the redirect, and required
   to match on return. Single-use: consumed whether or not it matches, so a
   captured callback cannot be replayed.

A callback that fails either check is dropped before `exchangeCodeForSession` is
ever called. Neither the code nor the verifier is logged.

### Nothing sensitive on disk

supabase-js defaults to `localStorage`, which in a webview means the access
token, the long-lived refresh token **and** the PKCE verifier sit in plaintext in
the app's data directory. Instead, `keychainStorage` in `src/lib/supabase.ts`
routes **every** key supabase-js writes through the Rust `account_session_*`
commands into the macOS Keychain, under service `com.colima.ui.account`.

Routing the verifier too — not just the tokens — is the point. Leaving it in
`localStorage` would defeat the PKCE protection the deep-link flow depends on.

### No tables, therefore no RLS surface

The profile is **read-only**: name, avatar, email and provider all come from the
OAuth provider. There are no Supabase tables, so there is no Row Level Security
policy to get wrong. The anon key can read exactly nothing.

If a table is ever added, **RLS becomes mandatory** — the anon key ships in every
copy of the app, and whatever RLS permits, the public can read.

## The one server component

`supabase/functions/entitlement/` — the entitlement oracle, and the only
server-side code in the product.

It exists for a single reason: reading Polar seat state requires
`POLAR_ACCESS_TOKEN`, an organization-wide secret that cannot ship in a desktop
app. The retired license-key endpoint needed no secret, which is why that model
worked with no backend at all.

```
app --(user JWT)--> entitlement --(POLAR_ACCESS_TOKEN)--> Polar customer state
                        └-> { entitled, tier, seats, expires_at }
                              cached to ~/.colima-ui/subscription.json
```

| Rule | Why |
|---|---|
| Identity comes only from the verified JWT | The body is never read; there is deliberately nothing in it to trust |
| Polar unreachable → `503 upstream_unavailable`, **not** "not entitled" | The client leaves its cache alone. Collapsing the two would downgrade payers whenever Polar had a bad minute |
| `expires_at` = earlier of the paid period end and a 30-day grace | Never grants past what was paid; never lets a cancellation go unnoticed for months |
| Nothing is written anywhere | No tables exist; the function only reads |

**Deployment:** `supabase functions deploy entitlement`, then
`supabase secrets set POLAR_ACCESS_TOKEN=... POLAR_ENV=production`. See
`supabase/.env.example`. The token needs only the `customers:read` scope.

> Full console walkthrough — products, checkout links, token, test purchase:
> [polar-billing-setup.md](./polar-billing-setup.md)

Until it is deployed, `refreshEntitlement()` is a silent no-op and the app runs
purely on whatever is cached — which is the correct degradation, not a fallback
hack.

## Platform behavior

| Situation | Behavior |
|---|---|
| macOS desktop | Full support; session persists in Keychain |
| Other desktop OS | Signs in, but no keychain backend → session is in-memory; the profile says you will sign in again next launch |
| Keychain denied by user | Same as above — degrades, never crashes |
| Browser mode (port 11420) | **Sign-in unavailable.** No URL scheme, no keychain; the alternative was a refresh token in `localStorage`. Both the panel and the profile say so |
| Anon key not set | Reports `unconfigured`, explains instead of offering dead buttons |
| Token refresh fails offline | Last known identity is kept and retried. Only an explicit `SIGNED_OUT` clears the badge, so a flaky network does not make it flicker |

## Troubleshooting

**Browser shows "requested path is invalid"** — the redirect allow-list is
missing `colimaui://auth-callback*`, or was added without the trailing `*`.

**Consent succeeds but the app never reacts** — the `colimaui://` scheme is not
registered. Scheme registration comes from `tauri.conf.json` → `plugins.deep-link.desktop.schemes`
and only takes effect in a **bundled** app; `pnpm tauri dev` may not register it
on macOS. Test with a real build.

**Console shows `auth callback rejected: state mismatch`** — working as intended.
The callback did not originate from an attempt this app started: a stale browser
tab, a second window, or a replay.

**Signed in, but Pro did not appear** — correct. The account grants nothing;
activate a license key in Settings → License.

## Changing identity

The profile is read-only by design. Name and picture come from the provider and
are changed there — Settings → Account links out to the right page for whichever
provider was used.
