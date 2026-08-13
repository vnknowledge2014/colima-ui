# Setup Runbook — Supabase Auth + Polar Billing

Follow this top-to-bottom **once**, and the app has the full loop: sign in →
pick a plan → pay → Pro/Teams badge appears on your account.

Deep-dive docs are linked per step — this file is the ordered checklist, not a
replacement for them:

| Deep-dive | Covers |
|---|---|
| [supabase-provider-setup.md](./supabase-provider-setup.md) | Console click-by-click: Google/GitHub OAuth, redirect allow-list |
| [polar-billing-setup.md](./polar-billing-setup.md) | Console click-by-click: Polar products, benefits, token, test purchase |
| [account-supabase-oauth.md](./account-supabase-oauth.md) | Architecture: how sign-in and entitlement connect |

**Time:** ~1.5 h the first time (mostly console clicking), ~15 min for a re-run.

---

## Phase 0 — Prerequisites

- [ ] `supabase` CLI installed (v2+)
  ```bash
  # Homebrew, or: npm install -g supabase
  brew install supabase/tap/supabase
  ```
- [ ] Accounts ready:
  - [ ] [Supabase](https://supabase.com) project (free tier is enough)
  - [ ] [Polar](https://polar.sh) account — start in the **sandbox**
        ([sandbox.polar.sh](https://sandbox.polar.sh/start), separate org)
  - [ ] Google Cloud Console project (for Google sign-in)
  - [ ] GitHub account (for GitHub sign-in)
- [ ] Repo cloned + `npm install` done

---

## Phase 1 — Supabase project + OAuth providers

**Deep-dive:** [supabase-provider-setup.md](./supabase-provider-setup.md)

- [ ] Create the Supabase project; note the **project ref** (random string in
      `https://<project-ref>.supabase.co`)
- [ ] Copy the **Project URL** (Integrations → Data API) and the **anon key**
      (Settings → API Keys, `sb_publishable_…`)
- [ ] Fill both in `src/lib/supabase.ts`:
      ```ts
      export const SUPABASE_URL = "https://<project-ref>.supabase.co";
      export const SUPABASE_ANON_KEY = "sb_publishable_…";
      ```
- [ ] Enable **Google** provider (OAuth consent screen + credentials →
      Supabase **Auth → Sign In / Providers**)
- [ ] Enable **GitHub** provider (OAuth app → same place)
- [ ] Allow-list the return URL in **Auth → URL Configuration**:
      `colimaui://auth-callback`

> The anon key is **public** — it is an identifier, not a credential. The
> Postgres password and `service_role` key never enter this project.

---

## Phase 2 — Polar: org, products, token

**Deep-dive:** [polar-billing-setup.md](./polar-billing-setup.md) steps 1–4 and 6

- [ ] Create a **sandbox** org (e.g. `colimaui`)
- [ ] Create **two seat-based** monthly subscriptions, $6/seat:
      | | Pro | Pro Teams |
      |---|---|---|
      | Seats | 1 | minimum 2 |
- [ ] Attach a **Custom benefit** (`ColimaUI Pro`) to **both** products —
      this is what grants entitlement to seat members
- [ ] Copy both **product IDs** (UUIDs)
- [ ] Create an **Organization Access Token**
      (**Organization Settings → Developers → New Token**):
      - Scopes: `customers:read`, `checkouts:write` — nothing more
      - Copy it now; it is shown once
- [ ] Set your org slug in `src/lib/external-links.ts`:
      ```ts
      export const POLAR_ORG_SLUG = "colimaui";
      ```

> Billing cycle and pricing model are locked at creation — wrong pick = new
> product.

---

## Phase 3 — Edge functions: secrets + deploy

This is where a purchase becomes Pro. The two functions
(`entitlement`, `checkout`) hold `POLAR_ACCESS_TOKEN` — it never ships in the app.

- [ ] Log in and link the CLI to the project (one-time):
      ```bash
      supabase login
      supabase link --project-ref <project-ref>
      # e.g. supabase link --project-ref nakwwkderqjfsxajduwj
      ```
      (No login = `Access token not provided` on deploy.)
- [ ] Set the secrets:
      ```bash
      supabase secrets set \
        POLAR_ACCESS_TOKEN=polar_oat_… \
        POLAR_ENV=sandbox \
        POLAR_PRODUCT_PRO=<product-uuid> \
        POLAR_PRODUCT_PRO_TEAMS=<product-uuid> \
        OFFLINE_GRACE_DAYS=30
      ```
- [ ] Deploy both functions:
      ```bash
      supabase functions deploy entitlement
      supabase functions deploy checkout
      ```
- [ ] Verify:
      ```bash
      supabase functions list          # both functions: DEPLOYED
      supabase secrets list            # names visible, values masked
      ```

> `POLAR_ENV=sandbox` is the safe default. When you move to production you must
> swap the token **and** the two product IDs at the same time — a production
> token is rejected by the sandbox API and vice versa.

---

## Phase 4 — App config (fallbacks)

- [ ] Optional: while `checkout` is undeployed, keep `CHECKOUT_LINK_URL` in
      `src/lib/external-links.ts` so the buy button still opens *something* —
      the app **warns** that such a purchase will not unlock Pro.
- [ ] Optional: local testing of the functions without deploying:
      put the same secrets in `supabase/.env` (git-ignored) and run
      `supabase functions serve`

---

## Phase 5 — Verify end-to-end (debug bundle)

**Deep-dive:** [polar-billing-setup.md](./polar-billing-setup.md) step 8

Sign-in needs a registered `.app`, so `tauri dev` is not enough:

- [ ] Build the debug bundle and launch:
      ```bash
      npx tauri build --debug
      open src-tauri/target/debug/bundle/macos/ColimaUI.app
      ```
- [ ] **Settings → Account → Sign in** (Google or GitHub). Checkout carries your
      Supabase user id as Polar's external customer id, so sign-in must come first.
- [ ] **Settings → Subscription → See plans →** pick a tier
- [ ] Pay with Stripe test card `4242 4242 4242 4242`, any expiry/CVC
- [ ] **Stay on Polar's confirmation page** until it finishes (that is where your
      seat is claimed)
- [ ] Back in the app: **Settings → Subscription → Refresh**

**Pass criteria (all three):**

1. Subscription page shows **Pro / Pro Teams + ACTIVE**
2. The sidebar user icon shows the matching badge — **PRO** (purple) or
   **TEAMS** (blue) — and **Settings → Account** shows it next to your name
3. In Polar, the customer created by the purchase has your **Supabase user id as
   its external ID** — otherwise the purchase can never grant entitlement

**Failure cheatsheet:** see the table in
[polar-billing-setup.md](./polar-billing-setup.md) — the top causes are: plain
Checkout Link used (no external id), seat not claimed (success URL), token/env
mismatch, or a pending (not claimed) team seat.

---

## Phase 6 — Go to production (only when ready)

- [ ] Create the **production** org + products in [polar.sh](https://polar.sh)
      (not the sandbox) — repeat Phase 2 there
- [ ] Create a production **Organization Access Token**
- [ ] Swap secrets:
      ```bash
      supabase secrets set \
        POLAR_ACCESS_TOKEN=polar_oat_… \
        POLAR_ENV=production \
        POLAR_PRODUCT_PRO=<prod-uuid> \
        POLAR_PRODUCT_PRO_TEAMS=<prod-uuid>
      ```
- [ ] Re-run Phase 5 against production (real card — the test card only works in
      the sandbox)
- [ ] Empty `CHECKOUT_LINK_URL` in `src/lib/external-links.ts` so the
      un-attributable fallback can never be offered
