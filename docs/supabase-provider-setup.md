# Supabase OAuth Provider Setup — Google & GitHub

Step-by-step console guide for enabling **Google** and **GitHub** sign-in in the
ColimaUI desktop app via Supabase. Identity only — see
[account-supabase-oauth.md](./account-supabase-oauth.md) for the architecture and
the identity-vs-entitlement boundary.

**Time:** ~30 minutes. **Cost:** free tier is enough.

---

## What you end up with

| Piece | Where it lives |
|---|---|
| `SUPABASE_URL` | `src/lib/supabase.ts` |
| `SUPABASE_ANON_KEY` (publishable) | `src/lib/supabase.ts` |
| Google OAuth credentials | Google Cloud Console → Supabase **Auth → Sign In / Providers** |
| GitHub OAuth credentials | GitHub Developer settings → Supabase **Auth → Sign In / Providers** |
| `colimaui://auth-callback*` | Supabase **Auth → URL Configuration** |

Only public config ever ships in the app. The Postgres connection string, the
database password and any `sb_secret_…`/`service_role` key are server-side
secrets — they never appear in this project.

---

## 1. Create the Supabase project

1. Go to [supabase.com](https://supabase.com) and sign in.
2. **New project** → pick a name (e.g. `colimaui`), a region, and set the
   database password.
3. Wait for the project to provision (~2 minutes).

You now have a project ref — the random string in your project URL
(`https://<project-ref>.supabase.co`).

---

## 2. Copy the public project URL + anon key

> UI note (2026): the old *Project Settings → API* page has been split.

1. **URL** — sidebar **Integrations → Data API**: copy the **Project URL**
   (`https://<project-ref>.supabase.co`).
2. **Anon key** — sidebar **Settings → API Keys**: copy the **publishable**
   (`anon`, `sb_publishable_…`) key.

Both are public by design — the anon key is an identifier, not a credential,
and PKCE makes an intercepted auth code useless.

Fill them in `src/lib/supabase.ts`:

```ts
export const SUPABASE_URL = "https://<project-ref>.supabase.co";
export const SUPABASE_ANON_KEY = "sb_publishable_…";
```

Until the key is filled in, the app reports accounts as unconfigured and
explains itself instead of showing dead buttons.

---

## 3. Google provider

### 3a. Create an OAuth client in Google Cloud Console

1. Open the [Google Cloud Console](https://console.cloud.google.com) — create a
   project if you don't have one.
2. **APIs & Services → OAuth consent screen**:
   - Choose **External** (or Internal if using a Workspace account).
   - Fill the required app name + support email. Add
     `openid`, `.../auth/userinfo.email`, `.../auth/userinfo.profile` scopes
     (email + profile are added by default; add `openid` manually).
   - Add yourself as a **test user** while the app is unverified.
3. **APIs & Services → Credentials → Create Credentials → OAuth client ID**:
   - Application type: **Web application**.
   - **Authorized redirect URIs**: add Supabase's callback URL (step 3b) —
     leave this empty for now and add it after step 3b, or paste the URL from
     the Supabase Google provider page in advance.
   - Click **Create** → copy the **Client ID** and **Client secret**.

### 3b. Enable Google in Supabase

1. Supabase dashboard → **Authentication → Sign In / Providers → Google**.
2. Toggle **Enable Sign in with Google** on.
3. Paste the **Client ID** and **Client secret** from 3a.
4. The page shows Supabase's **Callback URL** — copy it:
   ```
   https://<project-ref>.supabase.co/auth/v1/callback
   ```
5. Back in Google Cloud Console, add that exact URL under the OAuth client's
   **Authorized redirect URIs** and **Save**. Wait 1–2 minutes for propagation.

---

## 4. GitHub provider

1. GitHub → **Settings → Developer settings → OAuth Apps → New OAuth App**:
   - **Application name**: `ColimaUI`
   - **Homepage URL**: anything reachable (e.g. your repo URL)
   - **Authorization callback URL**: Supabase's callback URL —
     `https://<project-ref>.supabase.co/auth/v1/callback`
2. Click **Register application** → copy the **Client ID** and generate +
   copy the **Client secret** (shown once).
3. Supabase dashboard → **Authentication → Sign In / Providers → GitHub**:
   - Toggle **Enable Sign in with GitHub** on.
   - Paste **Client ID** + **Client secret**.
4. The Supabase GitHub page shows its own **Callback URL** — verify it matches
   exactly what you entered at GitHub. Any difference (trailing slash, `http`
   vs `https`) rejects every sign-in.

---

## 5. Redirect allow-list (the part everyone forgets)

Supabase → **Authentication → URL Configuration → Redirect URLs** → **Add URL**:

```
colimaui://auth-callback*
```

**The trailing `*` is mandatory.** The app appends `?state=<nonce>` to the
redirect, so an exact-match entry without the wildcard rejects every real
callback.

> The `colimaui://` scheme is configured **only in Supabase**. Google and GitHub
> redirect to Supabase's `https://…/auth/v1/callback`; Supabase then redirects
> back to the app. Never put `colimaui://` in Google/GitHub — they do not accept
> custom schemes.

---

## 6. Build & verify

1. Build the debug bundle (deep links do not work under `npm run tauri dev` on
   macOS — the raw dev binary is not a registered `.app`):
   ```bash
   npx tauri build --debug
   open src-tauri/target/debug/bundle/macos/ColimaUI.app
   ```
2. In the app: **Settings → Account → Sign in** → pick Google or GitHub.
3. Consent happens in your system browser; after authorizing, the browser
   redirects to Supabase, Supabase redirects to `colimaui://auth-callback`,
   and the app window comes back to front, signed in.

### Failure modes

| Symptom | Cause | Fix |
|---|---|---|
| `redirect_uri is not associated with this application` (Google) | Supabase's callback URL missing from the OAuth client's redirect URIs, or the wrong client's credentials pasted | Step 3b #5; verify client ID/secret pair |
| Redirect to `colimaui://` opens nothing / app stays in browser | Scheme not registered (running `tauri dev`), or redirect URL missing from allow-list | Use the debug bundle; step 5 |
| Callback rejected, no session | `state` mismatch or redirect URL without `*` | Step 5; check `[account] auth callback rejected` in the webview console |
| Sign-in buttons disabled | anon key empty in `src/lib/supabase.ts` | Step 2 |

---

## Security notes (from the architecture doc)

- **The deep link is untrusted transport.** Any macOS app can register
  `colimaui://`, so callbacks are protected by **PKCE** (code is useless
  without the keychain-held verifier) and a single-use **`state` nonce**.
- **Nothing sensitive on disk.** Session + verifier live in the OS keychain,
  not `localStorage`.
- **Consent always in the system browser**, never the app webview — Google
  refuses embedded user-agents outright.
