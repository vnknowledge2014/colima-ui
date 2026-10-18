<script lang="ts">
  import { upgradeState, closeUpgrade, goToPricing, goToCheckout } from "../store/upgrade.svelte";
  import { isBillingConfigured } from "../lib/external-links";
  import { t } from "../lib/i18n.svelte";
  import ProBadge from "./pro/ProBadge.svelte";

  /**
   * The plan picker. Offers, never blocks — every free feature keeps working if
   * this is dismissed, and dismissing is always one click away.
   *
   * Free is listed alongside the paid tiers on purpose: the published promise is
   * that Free is fully usable including commercial use, and a picker that hides
   * it would quietly contradict that.
   */
  let open = $derived(upgradeState.open);
  let feature = $derived(upgradeState.options.feature);
  let description = $derived(upgradeState.options.description);
  let configured = $derived(isBillingConfigured());

  const TIERS = [
    {
      id: "pro" as const,
      name: "plans.pro.name",
      nameDefault: "Pro",
      price: "plans.pro.price",
      priceDefault: "$6 / month",
      blurb: "plans.pro.blurb",
      blurbDefault: "Everything in Free, plus optional AI-powered fixes. One seat.",
    },
    {
      id: "pro_teams" as const,
      name: "plans.teams.name",
      nameDefault: "Pro Teams",
      price: "plans.teams.price",
      priceDefault: "$6 / seat / month",
      blurb: "plans.teams.blurb",
      blurbDefault: "The same Pro features for your team, from 2 seats. Manage seats in the billing portal.",
    },
  ];
</script>

{#if open}
  <div
    style="position: fixed; inset: 0; z-index: 10000; background: rgba(0, 0, 0, 0.6); backdrop-filter: blur(4px); display: flex; align-items: center; justify-content: center;"
    onclick={(e) => { if (e.target === e.currentTarget) closeUpgrade(); }}
    role="presentation"
  >
    <div style="background: var(--bg-primary); border-radius: var(--radius-lg); border: 1px solid var(--border-primary); box-shadow: 0 20px 60px rgba(0,0,0,0.5); padding: 24px; min-width: 420px; max-width: 520px;">
      <div style="display: flex; align-items: center; gap: 8px; margin-bottom: 8px;">
        <span style="font-size: var(--text-lg); font-weight: 600;">{feature}</span>
        <ProBadge />
      </div>

      <p style="font-size: var(--text-sm); color: var(--text-secondary); line-height: 1.6; margin: 0 0 8px;">
        {description || t('pro.gate.default_desc', { default: 'This is a ColimaUI Pro feature.' })}
      </p>

      <!-- The honest framing: free stays free, this is optional -->
      <p style="font-size: var(--text-xs); color: var(--text-muted); line-height: 1.6; margin: 0 0 20px;">
        {t('pro.gate.free_note', { default: 'Everything you use today stays free, including commercial use. Pro is optional.' })}
      </p>

      {#if configured}
        <div style="display: flex; flex-direction: column; gap: 10px; margin-bottom: 20px;">
          {#each TIERS as tier (tier.id)}
            <button
              class="plan"
              onclick={() => goToCheckout(tier.id)}
            >
              <span class="plan-head">
                <span class="plan-name">{t(tier.name, { default: tier.nameDefault })}</span>
                <span class="plan-price">{t(tier.price, { default: tier.priceDefault })}</span>
              </span>
              <span class="plan-blurb">{t(tier.blurb, { default: tier.blurbDefault })}</span>
            </button>
          {/each}
        </div>
      {:else}
        <!-- No dead buttons: while no product exists, say so and link to pricing. -->
        <p style="font-size: var(--text-sm); color: var(--text-muted); line-height: 1.6; margin: 0 0 20px; padding: 10px 12px; border: 1px solid var(--border-primary); border-radius: var(--radius-md);">
          {t('plans.not_configured', { default: 'Paid plans are not available yet. Everything in the app today stays free.' })}
        </p>
      {/if}

      <div style="display: flex; justify-content: flex-end; gap: 12px;">
        <button class="btn btn-ghost" onclick={closeUpgrade}>
          {t('pro.gate.dismiss', { default: 'Not now' })}
        </button>
        <button class="btn btn-ghost" onclick={goToPricing}>
          {t('pro.gate.learn_more', { default: 'Learn more' })} ↗
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  .plan {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 12px 14px;
    background: transparent;
    border: 1px solid var(--border-primary);
    border-radius: var(--radius-md);
    color: var(--text-primary);
    cursor: pointer;
    text-align: left;
    transition: background 0.15s, border-color 0.15s;
  }

  .plan:hover {
    background: var(--bg-hover);
    border-color: var(--color-primary, var(--border-primary));
  }

  .plan-head {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 12px;
  }

  .plan-name {
    font-size: var(--text-md);
    font-weight: 600;
  }

  .plan-price {
    font-size: var(--text-sm);
    color: var(--text-secondary);
    font-variant-numeric: tabular-nums;
  }

  .plan-blurb {
    font-size: var(--text-xs);
    color: var(--text-muted);
    line-height: 1.5;
  }
</style>
