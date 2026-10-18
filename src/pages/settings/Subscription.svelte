<script lang="ts">
  import { proState, refreshEntitlement } from "../../lib/pro.svelte";
  import { accountState } from "../../lib/account.svelte";
  import { showUpgrade } from "../../store/upgrade.svelte";
  import { openExternal, portalUrl, isBillingConfigured } from "../../lib/external-links";
  import { t } from "../../lib/i18n.svelte";
  import SettingsSection from "../../components/settings/SettingsSection.svelte";

  /**
   * Settings → Subscription. Replaces the old License page.
   *
   * Read-only by design: tier, seats and renewal date are shown here, and every
   * mutation — seat assignment, invites, payment method, cancellation — happens
   * in Polar's Customer Portal. Rebuilding those flows would mean a second UI
   * over the same data, each write needing its own authorized server path.
   *
   * Sending customers to the portal also inherits failed-payment recovery and
   * self-service cancellation, the latter being legally required in some
   * jurisdictions.
   */
  let sub = $derived(proState.subscription);
  let paid = $derived(proState.paid);
  let signedIn = $derived(accountState.user !== null);
  let configured = $derived(isBillingConfigured());
  let refreshing = $state(false);

  /** Renewal/expiry in the user's locale; the raw RFC3339 helps nobody. */
  let renewsOn = $derived(
    sub?.expires_at
      ? new Date(sub.expires_at).toLocaleDateString(undefined, {
          year: "numeric",
          month: "short",
          day: "numeric",
        })
      : ""
  );

  let tierName = $derived(
    sub?.tier === "pro_teams"
      ? t('plans.teams.name', { default: 'Pro Teams' })
      : t('plans.pro.name', { default: 'Pro' })
  );

  let isTeams = $derived(sub?.tier === "pro_teams");

  async function refresh() {
    refreshing = true;
    try {
      await refreshEntitlement();
    } finally {
      refreshing = false;
    }
  }
</script>

<SettingsSection title={t('subscription.title', { default: 'Subscription' })}>

  {#if paid && sub}
    <div style="display: flex; align-items: baseline; gap: 10px; margin: 16px 0 4px;">
      <span style="font-size: var(--text-md); font-weight: 600;">{tierName}</span>
      <span class="badge badge-running">{t('subscription.active', { default: 'ACTIVE' })}</span>
    </div>

    <div style="font-size: var(--text-sm); color: var(--text-secondary);">
      {#if isTeams && sub.seats_total}
        {t('subscription.seats', {
          claimed: sub.seats_claimed ?? 0,
          total: sub.seats_total,
          default: `${sub.seats_claimed ?? 0} of ${sub.seats_total} seats claimed`,
        })}
      {/if}
      {#if renewsOn}
        <div>{t('subscription.renews', { date: renewsOn, default: `Renews ${renewsOn}` })}</div>
      {/if}
    </div>

    <!-- The offline promise, stated where the expiry date is visible, so the
         date does not read as a deadline the user has to be online for. -->
    <p style="font-size: var(--text-xs); color: var(--text-muted); line-height: 1.6; margin: 12px 0 0;">
      {t('subscription.offline_note', { default: 'Pro keeps working offline, and stays active if you sign out.' })}
    </p>

    <div style="display: flex; gap: 12px; margin-top: 16px; flex-wrap: wrap;">
      <button class="btn btn-ghost" onclick={() => openExternal(portalUrl())}>
        {#if isTeams}
          {t('subscription.manage_team', { default: 'Manage team & seats' })} ↗
        {:else}
          {t('subscription.manage', { default: 'Manage subscription' })} ↗
        {/if}
      </button>
      <button class="btn btn-ghost" disabled={refreshing} onclick={refresh}>
        {refreshing
          ? t('subscription.refreshing', { default: 'Refreshing…' })
          : t('subscription.refresh', { default: 'Refresh' })}
      </button>
    </div>
  {:else if sub?.known && !paid}
    <!-- Lapsed rather than never-subscribed: say which, and when. -->
    <p style="font-size: var(--text-sm); color: var(--text-secondary); line-height: 1.6; margin: 8px 0 16px;">
      {#if renewsOn}
        {t('subscription.lapsed_on', { date: renewsOn, default: `Your subscription ended on ${renewsOn}.` })}
      {:else}
        {t('subscription.lapsed', { default: 'Your subscription is no longer active.' })}
      {/if}
    </p>
    <div style="display: flex; gap: 12px; flex-wrap: wrap;">
      <button class="btn btn-primary" onclick={() => showUpgrade(t('subscription.title', { default: 'Subscription' }))}>
        {t('subscription.resubscribe', { default: 'See plans' })}
      </button>
      <button class="btn btn-ghost" onclick={() => openExternal(portalUrl())}>
        {t('subscription.billing', { default: 'Billing & invoices' })} ↗
      </button>
    </div>
  {:else}
    <p style="font-size: var(--text-sm); color: var(--text-secondary); line-height: 1.6; margin: 8px 0 16px;">
      {t('subscription.free_desc', { default: 'You are on the free plan — the whole app, including commercial use. Pro adds optional AI-powered fixes.' })}
    </p>

    {#if !configured}
      <p style="font-size: var(--text-sm); color: var(--text-muted); line-height: 1.6; margin: 0 0 16px; padding: 10px 12px; border: 1px solid var(--border-primary); border-radius: var(--radius-md);">
        {t('plans.not_configured', { default: 'Paid plans are not available yet. Everything in the app today stays free.' })}
      </p>
    {:else}
      <div style="display: flex; gap: 12px; flex-wrap: wrap;">
        <button class="btn btn-primary" onclick={() => showUpgrade(t('subscription.title', { default: 'Subscription' }))}>
          {t('subscription.see_plans', { default: 'See plans' })}
        </button>
        {#if !signedIn}
          <!-- Said before the click, not after: checkout carries the account id,
               so buying signed out would leave the purchase unattachable. -->
          <span style="font-size: var(--text-xs); color: var(--text-muted); align-self: center;">
            {t('subscription.signin_first', { default: 'You will be asked to sign in first.' })}
          </span>
        {/if}
      </div>
    {/if}
  {/if}
</SettingsSection>
