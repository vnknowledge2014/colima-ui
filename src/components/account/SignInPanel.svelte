<script lang="ts">
  import { accountDialogState, closeSignIn } from "../../store/account-dialog.svelte";
  import { accountState, accountStatus, isSignedIn, signIn, cancelPendingSignIn } from "../../lib/account.svelte";
  import type { OAuthProvider } from "../../lib/account-oauth";
  import { t } from "../../lib/i18n.svelte";

  /**
   * The sign-in offer. There is no signup form: the first OAuth login *is* the
   * signup, so the panel has exactly two buttons and a way out.
   *
   * It never blocks. Nothing in the app waits on it, "Skip" simply closes it,
   * and every free feature works identically signed out.
   */
  let open = $derived(accountDialogState.open);
  let status = $derived(accountStatus());
  let busy = $derived(accountState.busy);
  let error = $state("");

  // A completed sign-in closes the offer: the badge now shows the user, so the
  // panel has nothing left to say. Guards the pre-sign-in state too — if a
  // session already exists when the panel mounts, it must not flash.
  $effect(() => {
    if (isSignedIn()) closeSignIn();
  });

  async function choose(provider: OAuthProvider) {
    error = "";
    try {
      await signIn(provider);
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  function dismiss() {
    // A pending round-trip is abandoned rather than left spinning behind a
    // closed panel; the browser tab is the user's to close.
    cancelPendingSignIn();
    error = "";
    closeSignIn();
  }
</script>

{#if open}
  <div
    style="position: fixed; inset: 0; z-index: 10000; background: rgba(0, 0, 0, 0.6); backdrop-filter: blur(4px); display: flex; align-items: center; justify-content: center;"
    onclick={(e) => { if (e.target === e.currentTarget) dismiss(); }}
    role="presentation"
  >
    <div style="background: var(--bg-primary); border-radius: var(--radius-lg); border: 1px solid var(--border-primary); box-shadow: 0 20px 60px rgba(0,0,0,0.5); padding: 24px; min-width: 380px; max-width: 440px;">
      <div style="font-size: var(--text-lg); font-weight: 600; margin-bottom: 8px;">
        {t('account.signin.title', { default: 'Sign in to ColimaUI' })}
      </div>

      <p style="font-size: var(--text-sm); color: var(--text-secondary); line-height: 1.6; margin: 0 0 8px;">
        {t('account.signin.subtitle', { default: 'Signing in is optional — it only puts your name on this app. Your first sign-in creates the account.' })}
      </p>

      <!-- Said plainly, because the sidebar badge could otherwise imply it. -->
      <p style="font-size: var(--text-xs); color: var(--text-muted); line-height: 1.6; margin: 0 0 20px;">
        {t('account.signin.no_unlock', { default: 'An account unlocks nothing. Pro is your license key, and it keeps working signed out and offline.' })}
      </p>

      {#if status === "ready"}
        <div style="display: flex; flex-direction: column; gap: 10px; margin-bottom: 16px;">
          <button class="btn btn-secondary" disabled={busy} onclick={() => choose("google")}>
            {t('account.signin.google', { default: 'Continue with Google' })}
          </button>
          <button class="btn btn-secondary" disabled={busy} onclick={() => choose("github")}>
            {t('account.signin.github', { default: 'Continue with GitHub' })}
          </button>
        </div>

        {#if busy}
          <p style="font-size: var(--text-xs); color: var(--text-muted); margin: 0 0 12px;">
            {t('account.signin.waiting', { default: 'Finish signing in in your browser, then come back here.' })}
          </p>
        {/if}
      {:else}
        <!-- No dead buttons: when sign-in cannot work, say why instead. -->
        <p style="font-size: var(--text-sm); color: var(--text-muted); line-height: 1.6; margin: 0 0 16px; padding: 10px 12px; border: 1px solid var(--border-primary); border-radius: var(--radius-md);">
          {#if status === "browser-mode"}
            {t('account.unavailable.browser', { default: 'Sign-in is only available in the ColimaUI desktop app — it needs the system browser and your keychain.' })}
          {:else}
            {t('account.unavailable.unconfigured', { default: 'Accounts are not configured in this build yet.' })}
          {/if}
        </p>
      {/if}

      {#if error}
        <p style="font-size: var(--text-xs); color: var(--color-error); margin: 0 0 12px;">{error}</p>
      {/if}

      <div style="display: flex; justify-content: flex-end;">
        <button class="btn btn-ghost" onclick={dismiss}>
          {t('account.signin.skip', { default: 'Skip' })}
        </button>
      </div>
    </div>
  </div>
{/if}
