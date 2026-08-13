<script lang="ts">
  import type { Snippet } from "svelte";
  import ProBadge from "./ProBadge.svelte";
  import { showUpgrade } from "../../store/upgrade.svelte";
  import { t } from "../../lib/i18n.svelte";

  let {
    feature,
    description,
    preview,
  } = $props<{
    /** Human name of the Pro feature, shown in the overlay and upgrade dialog. */
    feature: string;
    description?: string;
    /**
     * A REPRESENTATIVE, honest sample of what this Pro feature outputs — baked
     * into the free app. Never the user's live result, and never fabricated
     * value. It is blurred purely as a teaser; the readable message is the
     * overlay, not the blur. See the Pro UI design system report.
     */
    preview: Snippet;
  }>();
</script>

<div class="pro-locked-preview">
  <!-- The blurred sample is decorative: hidden from assistive tech and not
       focusable, so blur is never the only signal. -->
  <div class="pro-locked-preview__sample" aria-hidden="true" inert>
    {@render preview()}
  </div>

  <div class="pro-locked-preview__overlay">
    <ProBadge />
    <div class="pro-locked-preview__feature">{feature}</div>
    {#if description}
      <div class="pro-locked-preview__desc">{description}</div>
    {/if}
    <!-- No ↗ here. Throughout this app that arrow means "this leaves the app",
         and it is honoured everywhere else it appears (SetupWizard, Account,
         Subscription, and the pricing button in UpgradeDialog, which really
         does open a browser). This button opens the upgrade dialog in place, so
         the arrow was promising a trip the click never takes. -->
    <button class="btn btn-primary" onclick={() => showUpgrade({ feature, description })}>
      {t('pro.gate.learn_more', { default: 'Learn more' })}
    </button>
  </div>
</div>

<style>
  .pro-locked-preview {
    position: relative;
    border-radius: var(--radius-lg);
    overflow: hidden;
    border: 1px solid color-mix(in srgb, var(--accent-purple) 25%, transparent);
  }
  .pro-locked-preview__sample {
    filter: blur(6px);
    opacity: 0.5;
    pointer-events: none;
    user-select: none;
  }
  .pro-locked-preview__overlay {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 8px;
    text-align: center;
    padding: 24px;
    overflow-y: auto;
    background: color-mix(in srgb, var(--bg-primary) 55%, transparent);
  }
  .pro-locked-preview__feature {
    font-weight: 600;
  }
  .pro-locked-preview__desc {
    font-size: var(--text-sm);
    color: var(--text-secondary);
    max-width: 34ch;
    display: -webkit-box;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    overflow: hidden;
  }
</style>
