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

  let {
    children,
  } = $props<{
    id?: string;
    capability?: string;
    feature?: string;
    description?: string;
    variant?: "affordance" | "preview";
    preview?: Snippet;
    children: Snippet;
  }>();
</script>

{@render children()}
