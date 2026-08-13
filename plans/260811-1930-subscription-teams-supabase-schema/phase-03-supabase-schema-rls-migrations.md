---
phase: 3
title: "Supabase Schema, RLS & Migrations"
status: cancelled
priority: P1
dependencies: [1]
effort: "2-3 days"
---

# Phase 3: Supabase Schema, RLS & Migrations — CANCELLED

> **This phase is cancelled. No Supabase tables will be created.**
>
> Phase 2's Step 0 found that Polar models the roster natively — Customer
> (`type: "team"`), Member (with roles), and CustomerSeat (with invitation tokens
> and `pending`/`claimed`/`revoked` status). Every table designed below already
> exists there, maintained by the payment processor, with invitation emails,
> claim links, revocation and seat-count changes built in.
>
> Building a mirror would have re-created that state in a second place and taken
> on the drift between them, in exchange for a team page that users open rarely.
>
> **What this cancellation removes from the plan:**
>
> - Both High-severity RLS risks (cross-team roster reads via the public anon key).
> - The "client forges membership by writing directly" risk — there is no table to
>   write to.
> - The invite-email PII concern. Those addresses now live only at Polar, which is
>   already the merchant of record and therefore already a processor of them.
> - The migration workflow, and the risk of a destructive migration on a live project.
>
> Supabase remains **auth-only**, exactly as the account plan left it. The one
> server-side piece still needed is the entitlement oracle — see phase 4.
>
> The original design is preserved below. If a genuine need for our own roster
> data ever appears, this is the reviewed starting point, and the security posture
> it describes (deny-by-default, no client writes, RLS in the creating migration)
> still applies.

---

## Original design (not implemented)

# ⚠ Security core

These are the **first tables in the project**. Until now the app created none,
which is why Row Level Security has been a non-issue. The public anon key ships in
every copy of ColimaUI, and **whatever RLS permits, the public can read.** Assume
an attacker has the anon key, because they do.

## Overview
Define the roster schema, lock it with deny-by-default RLS, and establish a
versioned migration workflow.

## Requirements
- Functional: represent teams, membership and pending invites; support "my team",
  "my members", "seats used".
- Non-functional: **no client write access to any table.** Reads scoped to the
  caller's own team. Migrations versioned, reviewed, reversible.

## Architecture

Proposed tables — the roster only, never entitlement:

```
teams
  id                uuid pk
  name              text
  owner_user_id     uuid   → auth.users
  polar_subscription_id text   -- link to the billing truth
  seats             int      -- MIRROR of Polar; never authoritative
  created_at        timestamptz

team_members
  team_id           uuid → teams
  user_id           uuid → auth.users
  role              text   -- 'owner' | 'member'
  joined_at         timestamptz
  primary key (team_id, user_id)

team_invites
  id                uuid pk
  team_id           uuid → teams
  email             citext        -- ⚠ first PII stored by this project
  token_hash        text          -- store a HASH, never the raw token
  expires_at        timestamptz
  accepted_at       timestamptz null
```

**`teams.seats` is a mirror for display.** Entitlement asks Polar, never this
column. A stale or tampered value must be incapable of granting Pro — Phase 5's
resolution path never reads it.

### RLS posture

Start from **deny everything**, then add only read policies:

- `teams` — SELECT where the caller is a member.
- `team_members` — SELECT where the caller shares the team.
- `team_invites` — SELECT restricted to the team **owner**. Invite emails are
  other people's addresses; members have no business enumerating them.
- **No INSERT / UPDATE / DELETE policies for `anon` or `authenticated`, anywhere.**
  All writes go through Edge Functions on `service_role`, which bypasses RLS.

That last point is the whole design. If any client write policy exists, a user can
forge membership with a key they already have.

### Migration workflow

Supabase CLI, migrations committed to the repo:

```
supabase/
  migrations/<timestamp>_<name>.sql
  config.toml
```

Local-first: `supabase start` → apply → test → then push. No secrets committed —
`config.toml` holds no keys, and `.env` stays git-ignored.

## Related Code Files
- Create: `supabase/migrations/*.sql` — schema + policies, one concern per file.
- Create: `supabase/config.toml`.
- Create: `docs/supabase-schema.md` — tables, policies, and the reasoning for
  each, especially *why* there are no client write policies.
- Modify: `.gitignore` — Supabase local artifacts and any env file.
- Modify: `docs/telemetry.md` — invite emails are PII; the privacy story changes.

## Implementation Steps
1. Install/configure the Supabase CLI; `supabase init`; verify local start.
2. Write the schema migration. Enable RLS on **every** table in the same
   migration that creates it — never in a follow-up.
3. Write read policies one at a time, each with a comment stating who it lets in.
4. Write the **policy test suite**: for each table, assert an authenticated user
   from team A can read nothing of team B, and that every write is refused.
5. Verify with a raw anon-key client (not the app) that no write succeeds.
6. Document retention for `team_invites` — expired invites are stale PII and
   should be purged on a schedule.

## Success Criteria
- [ ] Migrations apply cleanly from empty, and are reversible.
- [ ] RLS enabled on every table, created in the same migration as the table.
- [ ] Cross-team read returns **zero rows**, proven by test, not inspection.
- [ ] Every write with the anon key is refused, proven with a raw client.
- [ ] Invite tokens stored hashed; raw tokens exist only in transit.
- [ ] `docs/supabase-schema.md` written; `docs/telemetry.md` updated for PII.
- [ ] No secret committed.

## Risk Assessment
- **A single permissive policy leaks every team's roster and invite emails.**
  Highest-severity item in the plan. Mitigation: deny-by-default, per-policy
  tests, and a review pass that reads each policy aloud against "who can do this".
- **RLS enabled in a later migration than the table** leaves a window where the
  table is world-readable. Always same migration.
- **`citext`/extension availability** — confirm before relying on it; a
  case-sensitive email column causes duplicate invites.
- **Invite token in a URL** ends up in logs and browser history. Hash at rest,
  short expiry, single use.
- **Destructive migration on a live project.** Local-first, reviewed, reversible;
  no `drop`/`truncate` without an explicit documented step.
