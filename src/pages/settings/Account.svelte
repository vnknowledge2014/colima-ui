<script lang="ts">
  import { accountState, accountStatus, signOut } from "../../lib/account.svelte";
  import { showSignIn } from "../../store/account-dialog.svelte";
  import { openExternal } from "../../lib/external-links";
  import { proState } from "../../lib/pro.svelte";
  import { globalToast } from "../../lib/globalToast";
  import { t } from "../../lib/i18n.svelte";
  import SettingsSection from "../../components/settings/SettingsSection.svelte";

  /**
   * Settings → Account: the read-only profile.
   *
   * It sits next to `Subscription.svelte`, and the two answer different
   * questions on purpose — Subscription is "have you paid", Account is "who are
   * you". The tier badge rendered here is a read-only copy of the subscription
   * state for convenience; Subscription remains the authority on the details.
   *
   * Read-only by design: name, avatar and email come from the OAuth provider and
   * are changed there. That means no profile table in Supabase, and therefore no
   * row-level-security surface to get wrong.
   */
  let user = $derived(accountState.user);
  let status = $derived(accountStatus());
  let busy = $state(false);
  let avatarFailed = $state(false);

  /** "pro" | "pro_teams" | null — the tier of the signed-in account, when paid. */
  let tier = $derived(
    user && proState.paid ? (proState.subscription?.tier ?? "pro") : null
  );

  /** Where a user goes to change the identity this app displays. */
  const PROVIDER_ACCOUNT_URLS: Record<string, string> = {
    google: "https://myaccount.google.com/personal-info",
    github: "https://github.com/settings/profile",
  };

  let initials = $derived(
    (user?.name ?? "")
      .split(/\s+/)
      .filter(Boolean)
      .slice(0, 2)
      .map((w) => w[0]?.toUpperCase() ?? "")
      .join("") || "?"
  );

  let providerLabel = $derived(
    user?.provider ? user.provider.charAt(0).toUpperCase() + user.provider.slice(1) : ""
  );

  async function doSignOut() {
    busy = true;
    try {
      await signOut();
      globalToast("success", t('account.signed_out', { default: 'Signed out.' }));
    } finally {
      busy = false;
    }
  }
</script>

<SettingsSection title={t('account.title', { default: 'Account' })}>

  {#if user}
    <div style="display: flex; align-items: center; gap: 14px; margin: 16px 0;">
      {#if user.avatar && !avatarFailed}
        <img
          src={user.avatar}
          alt=""
          style="width: 48px; height: 48px; border-radius: 50%; object-fit: cover; flex-shrink: 0;"
          onerror={() => (avatarFailed = true)}
        />
      {:else}
        <span
          aria-hidden="true"
          style="width: 48px; height: 48px; border-radius: 50%; flex-shrink: 0; display: flex; align-items: center; justify-content: center; background: var(--bg-tertiary); border: 1px solid var(--border-primary); color: var(--text-secondary); font-size: var(--text-sm); font-weight: 600;"
        >{initials}</span>
      {/if}

      <div style="min-width: 0;">
        <div style="display: flex; align-items: center; gap: 8px;">
          <span style="font-size: var(--text-md); font-weight: 600; color: var(--text-primary);">{user.name}</span>
          {#if tier}
            <span class="badge {tier === 'pro_teams' ? 'badge-teams' : 'badge-pro'}" aria-label={tier === 'pro_teams' ? 'Pro Teams' : 'Pro'}>
              {tier === 'pro_teams' ? t('pro.gate.badge_teams', { default: 'TEAMS' }) : t('pro.gate.badge', { default: 'PRO' })}
            </span>
          {/if}
        </div>
        {#if user.email}
          <div style="font-size: var(--text-sm); color: var(--text-secondary); overflow: hidden; text-overflow: ellipsis;">{user.email}</div>
        {/if}
        {#if providerLabel}
          <div style="font-size: var(--text-xs); color: var(--text-muted); margin-top: 2px;">
            {t('account.signed_in_with', { provider: providerLabel, default: `Signed in with ${providerLabel}` })}
          </div>
        {/if}
      </div>
    </div>

    <!-- Read-only is a design decision, not a missing feature — say so. -->
    <p style="font-size: var(--text-xs); color: var(--text-muted); line-height: 1.6; margin: 0 0 8px;">
      {t('account.readonly_note', { default: 'Your name and picture come from your sign-in provider. Change them there and they update here.' })}
    </p>

    {#if accountState.ephemeral}
      <p style="font-size: var(--text-xs); color: var(--text-muted); line-height: 1.6; margin: 0 0 8px;">
        {t('account.ephemeral_note', { default: 'This session cannot be saved securely on this system, so you will need to sign in again next launch.' })}
      </p>
    {/if}

    <div style="display: flex; gap: 12px; margin-top: 16px;">
      <button class="btn btn-ghost" disabled={busy} onclick={doSignOut}>
        {t('account.sign_out', { default: 'Sign out' })}
      </button>
      {#if PROVIDER_ACCOUNT_URLS[user.provider]}
        <button class="btn btn-ghost" onclick={() => openExternal(PROVIDER_ACCOUNT_URLS[user.provider])}>
          {t('account.manage', { default: 'Manage account' })} ↗
        </button>
      {/if}
    </div>
  {:else if status === "ready"}
    <p style="font-size: var(--text-sm); color: var(--text-secondary); line-height: 1.6; margin: 8px 0 16px;">
      {t('account.signed_out_desc', { default: 'You are not signed in. An account only puts your name on this app — every feature works without one.' })}
    </p>
    <button class="btn btn-primary" onclick={showSignIn}>
      {t('account.sign_in', { default: 'Sign in' })}
    </button>
  {:else}
    <!-- Honest about why, rather than a button that cannot work. -->
    <p style="font-size: var(--text-sm); color: var(--text-secondary); line-height: 1.6; margin: 8px 0 0;">
      {#if status === "browser-mode"}
        {t('account.unavailable.browser', { default: 'Sign-in is only available in the ColimaUI desktop app — it needs the system browser and your keychain.' })}
      {:else}
        {t('account.unavailable.unconfigured', { default: 'Accounts are not configured in this build yet.' })}
      {/if}
    </p>
  {/if}
</SettingsSection>
