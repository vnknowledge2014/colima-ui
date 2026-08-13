<!--
  Shows Pro-only UI, or an offer to learn about Pro.

  # Never put a feature's off switch inside its own gate

  A locked gate renders the upsell *instead of* its children. So a control placed
  inside it disappears the moment entitlement lapses — and for a feature that
  keeps doing something in the background, the control that disappears is the one
  that stops it. The user loses the ability to intervene at exactly the moment
  they most need it, in a state they did not choose.

  Render the stop switch outside the gate, always. Gate the configuration, not
  the brake.

  # Gating UI is not gating behaviour

  This hides things. It does not stop anything already running: background work
  must check entitlement where it acts, and subscribe to
  `onEntitlementChange` so it hears about expiry. Entitlement is re-read on a
  timer and expires on a wall clock, so a decision made once at start-up is a
  decision made against an answer that has since changed.
-->
<script lang="ts">
  import type { Snippet } from "svelte";
  import { hasCapability, isPaid, isPaidButUnavailable } from "../lib/pro.svelte";
  import { showUpgrade } from "../store/upgrade.svelte";
  import { telemetryApi } from "../lib/api";
  import { t } from "../lib/i18n.svelte";
  import ProBadge from "./pro/ProBadge.svelte";
  import ProLockedPreview from "./pro/ProLockedPreview.svelte";

  // Feature ids → the snake_case `GatedCapability` enum the telemetry command
  // accepts. Only ids listed here are recorded; the Rust enum is closed, so an
  // unlisted id would be dropped server-side anyway.
  const GATED_ENUM: Record<string, string> = {
    "compose.autofix": "compose_autofix",
    "compose.diagnose": "compose_diagnose",
    "dockerfile.optimize": "dockerfile_optimize",
    "metrics.history": "metrics_history",
    "metrics.alerts": "metrics_alerts",
    "self.healing": "self_healing",
    "security.triage": "security_triage",
    "security.autofix": "security_autofix",
    "security.history": "security_history",
    "activity.export": "activity_export",
  };

  let {
    id,
    capability,
    feature,
    description,
    variant = "affordance",
    preview,
    children,
  } = $props<{
    /** Feature id for the telemetry funnel, e.g. "self.healing". */
    id?: string;
    /**
     * Sidecar capability to require, e.g. "compose.autofix".
     *
     * Omit it — which is the normal case — and the gate asks the only question
     * that has an answer today: has this user paid? A capability id requires a
     * Pro sidecar to declare it, and no sidecar is bundled (`pro/mod.rs`), so
     * gating a shipped feature on one locks out every paying customer. Pass it
     * only for a feature that genuinely cannot run without the sidecar.
     */
    capability?: string;
    /** Human name shown when gated. */
    feature: string;
    description?: string;
    /** "affordance" = small PRO button; "preview" = blurred teaser frame. */
    variant?: "affordance" | "preview";
    /** Representative sample for the preview variant (honest, never live data). */
    preview?: Snippet;
    /** Rendered when the feature is available. */
    children: Snippet;
  }>();

  // Two gating questions, and the right one depends on what the feature needs.
  //
  // Without a capability: entitlement alone, which is a boolean — matching what
  // `docs/pricing-rationale.md` promises and what `subscription/mod.rs` calls
  // the only field a gate may read.
  //
  // With one: branch on the FULL status, never collapsed to a boolean. A paying
  // customer whose sidecar is stale or absent must be told to restore it, not
  // to buy what they already own.
  const view = $derived(
    capability
      ? hasCapability(capability)
        ? "unlocked"
        : isPaidButUnavailable(capability)
          ? "needs_update"
          : "locked"
      : isPaid()
        ? "unlocked"
        : "locked",
  );

  // Funnel: record when a genuine non-customer is shown the Pro upsell.
  //
  // Watched rather than sampled at mount. Entitlement resolves asynchronously,
  // so a mount-time snapshot records a paying customer as a gate-hit whenever
  // the gate renders first — and never records the ordinary case where a free
  // user reaches a gate that only appears after some work completes.
  //
  // Fires at most once per instance: the interesting fact is "this user met this
  // gate", not how many times the state was recomputed.
  const telemetryId = $derived(GATED_ENUM[id ?? capability ?? ""]);
  let funnelRecorded = false;

  $effect(() => {
    if (funnelRecorded || view !== "locked" || !telemetryId) return;
    funnelRecorded = true;
    telemetryApi.recordProGateReached(telemetryId);
  });
</script>

{#if view === "unlocked"}
  {@render children()}
{:else if view === "needs_update"}
  <!-- A customer who paid: never the locked teaser. Prompt to restore, not to buy. -->
  <div style="display: inline-flex; align-items: center; gap: 6px; font-size: var(--text-sm); color: var(--text-secondary);">
    <ProBadge />
    {t('pro.gate.needs_update', { default: 'Pro component needs updating' })}
  </div>
{:else if variant === "preview" && preview}
  <ProLockedPreview {feature} {description} {preview} />
{:else}
  <!-- Offer, never block: an inert affordance to learn about Pro. Gates nothing. -->
  <button
    class="btn btn-ghost"
    style="display: inline-flex; align-items: center; gap: 6px;"
    onclick={() => showUpgrade({ feature, description })}
  >
    <ProBadge />
    {feature}
  </button>
{/if}
