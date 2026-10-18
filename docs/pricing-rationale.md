# Pricing Rationale

Why ColimaUI is priced the way it is. Written for our future selves, so that a
year from now we can tell a deliberate decision from an accident.

Status: **prices are proposed, not live.** Nothing is charged yet. The in-app
Settings entry says "coming soon" and links out; it makes no price promise.

## The proposal

| Tier | Price | Seats | What you get |
|---|---|---|---|
| Free | $0 | — | The whole app as it exists today, **including commercial use** |
| Pro | $6/month or $60/year | 1 | Optional AI-powered fixes, no usage limits |
| Pro Teams | $6/seat/month | 2+ | The same Pro features for N people, plus team and seat management |

## Why Teams is the same features, not more

**Pro Teams unlocks nothing Pro does not.** It is Pro bought for several people,
with a roster, invitations and seat management on top.

This is a deliberate refusal to build a feature ladder:

- Entitlement stays a **boolean**. The moment Teams has exclusive features, every
  gate in the app has to ask *which tier*, and the answer has to be right in the
  cache, offline, forever. That complexity buys nothing until the exclusive
  features actually exist.
- We would have to build those features *before* Teams was worth selling. That
  inverts the order: sell seats to teams who already want Pro, then learn what
  teams actually lack.
- "Team plan = same thing, for your team" is a sentence that needs no pricing
  page to explain.

If a genuinely team-shaped need appears later — shared config presets, a shared
Knowledge Bank — adding a tier then is a known, contained change. Adding it now
is speculation with a permanent tax on every gate.

## Why $6 per seat, with no volume discount

Same number as Pro, deliberately. A discount ladder is a thing we would have to
explain, maintain in the checkout, and justify in this document — for volumes we
have **zero evidence** we will ever see. We have not sold a single copy.

A discount is also easy to add later and awkward to take away. Starting flat
keeps that door open.

## Why the minimum is 2 seats

Two people is a team. Any higher floor forces a two-person startup to buy a seat
nobody sits in, which is a bad first impression on exactly the audience most
likely to adopt a Colima GUI early.

## Why "Pro Teams" and not "Personal / Teams"

"Pro" is already the word in the product, the `PRO` badge, and 42 translated
strings across four locales. Renaming it to "Personal" costs that churn and
discards the recognition for a symmetry nobody asked for.

"Pro Teams" also answers the question "is Teams also Pro?" in its own name —
which the alternative, "Pro / Teams", leaves genuinely ambiguous.

## Why $6

Anchored against OrbStack, the closest comparable product, at $8/month.

Pricing below the anchor is the point, not an accident:

- Our Pro tier ships **fewer** features than OrbStack Pro at launch. Matching
  their price while shipping less invites an unfavorable comparison.
- The free tier already covers everything OrbStack charges for. Pro has to be
  cheap enough that "I might as well" beats "let me think about it", because
  nobody is blocked without it.
- $6 keeps annual at a round $60. Two months free is the conventional annual
  discount and needs no explaining.

## Why the free tier allows commercial use

This is the core differentiator, and it is worth more than the revenue it
costs us.

OrbStack Free prohibits commercial use. That turns every developer using it at
work into either a license violation or a purchase decision that has to go
through procurement. We take the opposite position: use ColimaUI at work, for
free, forever. The core stays MIT.

The bet is that a product people are allowed to use at work spreads inside
companies, and that a fraction of those users will pay for the AI features
because those features save them time — not because a license forces them to.

## Why AI is BYOK at every tier

Users bring their own API key on Free and on Pro alike. There is no AI gateway
in between.

- We never hold a margin-eating cost that scales with usage. A heavy user costs
  us nothing.
- No backend to run, secure, or keep available. No user prompts pass through
  our servers.
- Pro unlocks *features* and removes *limits*. It does not resell tokens.

The trade-off we accept: onboarding is worse, because the user has to obtain a
key before the AI features do anything.

## What we refuse to charge for

1. **Anything the Colima CLI already does for free.** Paywalling a wrapper
   around a free command is a good way to be forked.
2. **The core app.** MIT, in full, no crippled build.

## How entitlement is delivered

**License keys are not used.** Pro is a subscription resolved through the
signed-in account: sign in, and the app asks Polar whether that account holds a
seat. The answer is cached locally with an expiry and honoured offline, so losing
the network does not cost a paying customer their Pro.

Keys were the earlier design and were dropped before launch, when nothing had
been sold. Seats and subscriptions describe a team correctly; distributing N keys
to N people does not. See `plans/260811-1930-subscription-teams-supabase-schema/`.

The trade this accepts: entitlement now depends on having signed in at least
once, and the offline cache has an expiry where a key had none. Signing out does
**not** revoke Pro — it runs until the cache lapses — so no sequence of clicks can
downgrade someone who paid.

## Open questions

These are unresolved and block the phases that depend on them:

1. ~~Which merchant of record? Needs built-in license keys.~~ **Resolved.** Polar,
   and the license-key criterion no longer applies — see "How entitlement is
   delivered" above. Polar is retained for subscriptions, not for keys.
2. Where does the selling legal entity sit? Affects tax and MoR eligibility.
3. Free licenses for OSS maintainers and students — yes or no?
4. **Offline cache window.** 30 days proposed. It also bounds two other things:
   how long a removed team member keeps Pro, and how long a shared machine keeps
   Pro after its owner signs out.
5. **Does Polar support seat-based subscriptions?** Assumed throughout and not yet
   verified. Blocks the entire Teams tier if the answer is no.

## How we will know if this is wrong

The pricing page and waitlist run before any licensing infrastructure is
built. A numeric success threshold has to be committed to in writing *before*
the run starts, or the result cannot be interpreted after the fact. If the
threshold is missed, the answer is to make the free product better and measure
again later — not to build the paid tier anyway.
