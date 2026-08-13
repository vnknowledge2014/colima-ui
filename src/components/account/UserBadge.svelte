<script lang="ts">
  import { accountState, accountStatus } from "../../lib/account.svelte";
  import { showSignIn } from "../../store/account-dialog.svelte";
  import { uiState } from "../../store.svelte";
  import { proState } from "../../lib/pro.svelte";
  import { t } from "../../lib/i18n.svelte";

  /**
   * Identity in the sidebar footer: who is signed in, or a way to sign in.
   *
   * The tier badge (PRO / TEAMS) shows the signed-in account's Polar seat
   * subscription. It is **display only** — it never unlocks or gates anything
   * and never implies that signing in is what makes someone Pro. A subscriber
   * keeps Pro while signed out (the entitlement is cached locally), so the badge
   * simply reflects the entitlement state that the Subscription page explains
   * in full. Nothing renders while the cached entitlement is still loading, so
   * the row cannot flash a wrong tier.
   *
   * Clicking always goes to Settings → Account when signed in; signed out it
   * opens the sign-in panel. Never a no-op.
   *
   * The collapsed rail is handled in CSS off the sidebar's own `.collapsed`
   * class rather than by reading `uiState` here, so the badge stays in step with
   * the neighbouring footer rows without a second source of truth.
   */
  let user = $derived(accountState.user);
  // Before the first session lookup resolves, render nothing rather than
  // flashing "Sign in" at someone who is in fact signed in.
  let ready = $derived(accountState.loaded);
  // In browser mode or an unconfigured build there is nothing to sign in to, so
  // the row is omitted entirely instead of offering a dead action.
  let offerSignIn = $derived(accountStatus() === "ready");

  /** "pro" | "pro_teams" | null — the tier of the signed-in account, when paid. */
  let tier = $derived(
    user && proState.paid ? (proState.subscription?.tier ?? "pro") : null
  );

  let avatarFailed = $state(false);

  /** Initials fallback — a broken <img> is never acceptable in a chrome row. */
  let initials = $derived(
    (user?.name ?? "")
      .split(/\s+/)
      .filter(Boolean)
      .slice(0, 2)
      .map((w) => w[0]?.toUpperCase() ?? "")
      .join("") || "?"
  );

  function open() {
    if (user) {
      uiState.currentPage = "settings";
    } else {
      showSignIn();
    }
  }
</script>

{#if ready && (user || offerSignIn)}
  <button
    class="account-badge"
    onclick={open}
    title={user ? `${user.name}${user.email ? ` — ${user.email}` : ""}` : t('account.badge.signin', { default: 'Sign in' })}
    aria-label={user ? user.name : t('account.badge.signin', { default: 'Sign in' })}
  >
    {#if user}
      {#if user.avatar && !avatarFailed}
        <img
          class="account-avatar"
          src={user.avatar}
          alt=""
          onerror={() => (avatarFailed = true)}
        />
      {:else}
        <span class="account-avatar account-avatar-initials" aria-hidden="true">{initials}</span>
      {/if}
      <span class="account-name">{user.name}</span>
      {#if tier}
        <span class="badge account-tier {tier === 'pro_teams' ? 'badge-teams' : 'badge-pro'}" aria-label={tier === 'pro_teams' ? 'Pro Teams' : 'Pro'}>
          {tier === 'pro_teams' ? t('pro.gate.badge_teams', { default: 'TEAMS' }) : t('pro.gate.badge', { default: 'PRO' })}
        </span>
      {/if}
    {:else}
      <span class="account-avatar account-avatar-initials" aria-hidden="true">
        <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2" />
          <circle cx="12" cy="7" r="4" />
        </svg>
      </span>
      <span class="account-name">{t('account.badge.signin', { default: 'Sign in' })}</span>
    {/if}
  </button>
{/if}

<style>
  .account-badge {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 6px 8px;
    margin-bottom: 6px;
    background: transparent;
    border: 1px solid transparent;
    border-radius: var(--radius-md);
    color: var(--text-secondary);
    cursor: pointer;
    text-align: left;
    transition: background 0.15s, border-color 0.15s;
  }

  .account-badge:hover {
    background: var(--bg-hover);
    border-color: var(--border-primary);
    color: var(--text-primary);
  }

  .account-avatar {
    flex-shrink: 0;
    width: 22px;
    height: 22px;
    border-radius: 50%;
    object-fit: cover;
  }

  .account-avatar-initials {
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(--bg-tertiary);
    border: 1px solid var(--border-primary);
    color: var(--text-secondary);
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 0.02em;
  }

  .account-name {
    overflow: hidden;
    font-size: var(--text-xs);
    font-weight: 500;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  /* Tier pill pinned to the right edge so a truncated name never collides with
     it. Smaller than the global .badge to sit comfortably in the footer row. */
  .account-tier {
    margin-left: auto;
    font-size: 0.58rem;
    padding: 1px 6px;
    flex-shrink: 0;
  }

  /* Collapsed rail: the avatar alone, centred like the other footer icons. */
  :global(.sidebar.collapsed) .account-badge {
    justify-content: center;
    padding: 6px 0;
  }

  :global(.sidebar.collapsed) .account-name,
  :global(.sidebar.collapsed) .account-tier {
    display: none;
  }
</style>
