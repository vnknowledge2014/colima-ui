# Setup Runbook — Polar Billing (Historical Reference)

> **Note:** All Pro features are unlocked by default in the free open-source edition (`07a2dbc`), and the in-app Sign-in / Account stack has been removed. This runbook is retained only as historical reference for the optional Polar billing functions.

Deep-dive docs are linked per step — this file is the ordered checklist, not a
replacement for them:

| Deep-dive | Covers |
|---|---|
| [polar-billing-setup.md](./polar-billing-setup.md) | Console click-by-click: Polar products, benefits, token, test purchase |

---

## Phase 0 — Prerequisites

- [ ] `supabase` CLI installed (v2+) if deploying legacy Edge Functions
  ```bash
  # Homebrew, or: npm install -g supabase
  brew install supabase/tap/supabase
  ```
- [ ] Accounts ready:
  - [ ] [Polar](https://polar.sh) account — start in the **sandbox**
        ([sandbox.polar.sh](https://sandbox.polar.sh/start), separate org)
- [ ] Repo cloned + `npm install` done

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

## Phase 5 — Verify Polar checkout link (debug bundle)

**Deep-dive:** [polar-billing-setup.md](./polar-billing-setup.md) step 8

- [ ] Build the debug bundle and launch:
      ```bash
      npx tauri build --debug
      open src-tauri/target/debug/bundle/macos/ColimaUI.app
      ```
- [ ] Pay with Stripe test card `4242 4242 4242 4242`, any expiry/CVC
- [ ] **Stay on Polar's confirmation page** until it finishes (that is where your
      seat is claimed)

**Failure cheatsheet:** see the table in
[polar-billing-setup.md](./polar-billing-setup.md) — the top causes are: seat
not claimed (success URL), token/env mismatch, or a pending (not claimed) team
seat.

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
