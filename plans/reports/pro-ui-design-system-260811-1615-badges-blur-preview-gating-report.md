# Pro Component Design System — Visual Language & Gating

**Audience:** any AI or developer designing a new Pro feature for ColimaUI.
**Read this before** you add a `ProGate`, a locked preview, a "PRO" badge, or any
UI that differs between Free and paid users. It exists so that every Pro surface
looks and behaves as one system instead of being reinvented per feature.

**Status:** design philosophy, agreed 2026-08-11; core components built same day.
Now in the tree: `src/components/pro/ProBadge.svelte`,
`src/components/pro/ProLockedPreview.svelte`, `src/components/ProGate.svelte`
(rewritten to branch on all four states), the `.badge-pro` class
(`src/styles/components.css`), and the `ProStatus` model (`src/lib/pro.svelte.ts`,
`src-tauri/src/pro/mod.rs`). First real consumer wired: `DiagnosePanel.svelte`
gates the flagship `compose.autofix` capability (affordance variant — no honest
preview sample exists yet, so no blur). Preview variant awaits its first feature
with a representative sample.

---

## The golden rule (non-negotiable)

> **Blur only teases Pro-exclusive preview content. It never obscures a result
> the free product already produces.**

This is the line between advertising and a dark pattern. Two same-looking blur
effects, two opposite meanings:

- **Allowed:** blur a representative sample of what a Pro feature *would* output.
  The free user never had this content, so nothing is taken from them — you are
  showing them what Pro does. Honest.
- **Forbidden:** compute a result the free product can produce, then blur it and
  demand payment to reveal it. That paywalls free work and violates plan
  **Nguyên tắc 1** ("không paywall thứ Colima CLI đã làm miễn phí").

A technical fact makes this easy to honor: Pro features run in the **sidecar**,
which a free user does not have. So there is no real Pro output on a free
machine to blur — the teaser is **always a designed, representative preview**
baked into the free app, never the user's live data run through Pro.

Corollary honesty constraint: the preview must **truthfully represent real Pro
output**. Do not fabricate impressive-looking content that Pro does not actually
produce. A blurred lie is still a lie.

---

## Never block a free action

Inherited from the locked "offer, never block" decision (Phase 6). A Pro surface
may sit *next to* free functionality, but it must never gate, cover, or nag a
control that works for free. The blur is on Pro-exclusive content only; every
free path around it stays fully usable, and every Pro affordance is dismissible.

---

## Visual language

One accent, used only for Pro, so "purple = Pro" becomes a learned signal.

| Token | Value | Use |
|---|---|---|
| `--accent-purple` | `#c084fc` | Pro badge text, locked-preview accents, CTA |
| `--accent-purple-glow` | `rgba(192,132,252,0.25)` | Locked-preview border/glow, badge background |

Do **not** use status colors (green/red/yellow) or the blue primary accent for
Pro — those already mean run-state and default action.

**Known inconsistency (honest note):** the app already uses `--accent-purple` for
a few *non-Pro* decorative things — the "Tauri v2" tech badge, the AI
memory-type badge, model badges, an "UNVERIFIED" AI marker. So today purple is
not yet a clean "Pro only" signal. The rule here applies to **new Pro surfaces**;
retroactively recoloring those pre-existing usages is a separate cleanup, not
done this round to avoid churning unrelated UI. New Pro badges use the
`.badge-pro` class (standardized on the token); earlier Pro badges hardcoded
`rgba(188,140,255,…)` and were migrated to `.badge-pro`.

The **PRO badge** builds on the existing `.badge` base class (pill, uppercase,
0.68rem, weight 700). Purple background at low alpha, purple text. Small, sits
above or leading the component title. It labels; it does not shout.

---

## The four states — a Pro surface has more than "locked vs unlocked"

Map directly to `ProStatus` (already implemented). Designing for only two states
is the trap that strands paying customers.

| `ProStatus` | Surface shows | Why |
|---|---|---|
| **Free** | PRO badge + blurred preview + "Learn more" CTA | The advertising state |
| **Active** | Real content; optional small PRO badge, **no blur** | They paid; show the thing |
| **NeedsUpdate** | "Restore your Pro component" — **NOT the locked/blur state** | A paying customer with a stale sidecar must never see the "you don't have Pro" screen (risk Critical #2) |
| **LicenseInactive** | Polite return to locked; Free intact, no data lost | Expiry is graceful, not punitive |

The **NeedsUpdate** distinction is the whole reason this is a system and not a
boolean: a two-state design (`if hasCapability … else lockedBlur`) renders the
locked teaser to someone who already paid. Always branch on the full enum.

**License ↔ sidecar merge (built 2026-08-11).** "Paid?" and "can run?" are two
separate questions from two sources: license entitlement (Polar, in core) answers
paid; sidecar capability answers can-run. `ProGate` now folds *both* the sidecar
`needs_update` state **and** "license-entitled but no sidecar/capability" into a
single **component-needed** state — a paying customer who has activated a license
but has no sidecar yet is shown "restore your Pro component", never the buy upsell.
Only a genuine non-customer (not entitled, no capability) sees the locked state.
See `isPaidButUnavailable(capability)` in `src/lib/pro.svelte.ts`.

---

## Components (to build)

Reusable so a new Pro feature is assembled, not restyled. DRY: define once.

- **`ProBadge`** — the purple PRO pill. Props: optional size. Pure label.
- **`ProLockedPreview`** — the teaser frame: a slot for the representative
  preview (blurred via `filter: blur(...)` + reduced opacity), an overlay with
  `ProBadge`, a one-line description of what Pro does, and a "Learn more" CTA that
  opens `UpgradeDialog`. The blurred content is **decorative/aria-hidden**; the
  real message is the readable overlay text, not the blur.
- **`ProGate`** (exists) — gains a `variant`: `affordance` (current inert button)
  or `preview` (renders `ProLockedPreview`). Still branches on `ProStatus` and
  still never blocks a free action.

---

## Accessibility & honesty checklist (per new Pro feature)

- [ ] Blur is **not the only signal** — the PRO badge + readable overlay text
      state it in words. A user who can't perceive blur still understands.
- [ ] Blurred preview content is `aria-hidden` / not focusable; the CTA is
      keyboard-reachable and labeled.
- [ ] The preview **truthfully** represents real Pro output. No fabricated value.
- [ ] No free control is covered, gated, or nagged by the Pro surface.
- [ ] All four `ProStatus` states handled — especially `NeedsUpdate` ≠ locked.
- [ ] Purple accent used only here; free UI keeps its normal colors.
- [ ] Expiry (`LicenseInactive`) loses no Free data and shows no punitive UI.

---

## How to apply when building a new Pro feature

1. Decide the capability id (e.g. `dockerfile.optimize`) — the sidecar declares
   it; the core never hardcodes the list.
2. Wrap the Pro surface in `ProGate` with `variant="preview"`, passing a
   representative (honest) preview sample.
3. Provide the four-state rendering via `ProStatus` — do not shortcut to two.
4. Use `ProBadge` + the purple tokens; do not invent new Pro styling.
5. Run the checklist above before calling it done.

## Unresolved

- Exact blur radius / opacity and whether the preview sample is a static image
  or a rendered mock — decide when the first previewed feature (compose auto-fix
  or Dockerfile optimizer) is built, then freeze it here.
