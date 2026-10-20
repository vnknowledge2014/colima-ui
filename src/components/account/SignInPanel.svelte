<script lang="ts">
  import { accountDialogState, closeSignIn } from "../../store/account-dialog.svelte";
  import {
    accountState,
    accountStatus,
    isSignedIn,
    signIn,
    signInWithCredentials,
    signUpWithCredentials,
    cancelPendingSignIn,
  } from "../../lib/account.svelte";
  import type { OAuthProvider } from "../../lib/account-oauth";
  import { OAUTH_ENABLED } from "../../lib/surrealdb";
  import { t } from "../../lib/i18n.svelte";

  /**
   * The sign-in & sign-up dialog backed by SurrealDB (`DEFINE ACCESS account`).
   *
   * Supports direct Email & Password authentication against SurrealDB, plus
   * optional OAuth deep-link providers when `VITE_OAUTH_ENABLED=true`.
   */
  let open = $derived(accountDialogState.open);
  let status = $derived(accountStatus());
  let busy = $derived(accountState.busy);
  let mode = $state<"signin" | "signup">("signin");
  let name = $state("");
  let email = $state("");
  let password = $state("");
  let localError = $state("");
  let displayError = $derived(localError || accountState.error);

  $effect(() => {
    if (isSignedIn()) closeSignIn();
  });

  async function submitCredentials(e: Event) {
    e.preventDefault();
    localError = "";
    try {
      if (mode === "signup") {
        await signUpWithCredentials(email, password, name);
      } else {
        await signInWithCredentials(email, password);
      }
      password = "";
    } catch (err) {
      localError = err instanceof Error ? err.message : String(err);
    }
  }

  async function choose(provider: OAuthProvider) {
    localError = "";
    try {
      await signIn(provider);
    } catch (e) {
      localError = e instanceof Error ? e.message : String(e);
    }
  }

  function dismiss() {
    cancelPendingSignIn();
    localError = "";
    password = "";
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

      <p style="font-size: var(--text-sm); color: var(--text-secondary); line-height: 1.6; margin: 0 0 16px;">
        {t('account.signin.subtitle', { default: 'Signing in is optional — it only puts your name on this app. Your first sign-in creates the account.' })}
      </p>

      {#if status === "ready"}
        <div style="display: flex; gap: 6px; margin-bottom: 14px; padding: 3px; background: var(--bg-secondary); border-radius: var(--radius-md); border: 1px solid var(--border-primary);">
          <button
            type="button"
            class="btn {mode === 'signin' ? 'btn-primary' : 'btn-ghost'}"
            style="flex: 1; justify-content: center; padding: 6px 10px; font-size: var(--text-xs);"
            disabled={busy}
            onclick={() => { mode = "signin"; localError = ""; }}
          >
            {t('account.signin.mode_signin', { default: 'Sign in' })}
          </button>
          <button
            type="button"
            class="btn {mode === 'signup' ? 'btn-primary' : 'btn-ghost'}"
            style="flex: 1; justify-content: center; padding: 6px 10px; font-size: var(--text-xs);"
            disabled={busy}
            onclick={() => { mode = "signup"; localError = ""; }}
          >
            {t('account.signin.mode_signup', { default: 'Create account' })}
          </button>
        </div>

        <form onsubmit={submitCredentials} style="display: flex; flex-direction: column; gap: 10px; margin-bottom: 16px;">
          {#if mode === "signup"}
            <div style="display: flex; flex-direction: column; gap: 4px;">
              <label for="surreal-account-name" style="font-size: var(--text-xs); color: var(--text-secondary);">
                {t('account.signin.name_label', { default: 'Name' })}
              </label>
              <input
                id="surreal-account-name"
                name="name"
                type="text"
                class="input"
                autocomplete="name"
                placeholder={t('account.signin.name_placeholder', { default: 'Your name' })}
                bind:value={name}
                disabled={busy}
              />
            </div>
          {/if}

          <div style="display: flex; flex-direction: column; gap: 4px;">
            <label for="surreal-account-email" style="font-size: var(--text-xs); color: var(--text-secondary);">
              {t('account.signin.email_label', { default: 'Email' })}
            </label>
            <input
              id="surreal-account-email"
              name="email"
              type="email"
              class="input"
              autocomplete="email"
              required
              placeholder={t('account.signin.email_placeholder', { default: 'you@example.com' })}
              bind:value={email}
              disabled={busy}
            />
          </div>

          <div style="display: flex; flex-direction: column; gap: 4px;">
            <label for="surreal-account-password" style="font-size: var(--text-xs); color: var(--text-secondary);">
              {t('account.signin.password_label', { default: 'Password' })}
            </label>
            <input
              id="surreal-account-password"
              name="password"
              type="password"
              class="input"
              autocomplete={mode === "signup" ? "new-password" : "current-password"}
              required
              placeholder={t('account.signin.password_placeholder', { default: '••••••••' })}
              bind:value={password}
              disabled={busy}
            />
          </div>

          <button type="submit" class="btn btn-primary" style="margin-top: 4px; justify-content: center;" disabled={busy}>
            {#if mode === "signup"}
              {t('account.signin.submit_signup', { default: 'Create SurrealDB account' })}
            {:else}
              {t('account.signin.submit_signin', { default: 'Sign in with SurrealDB' })}
            {/if}
          </button>
        </form>

        {#if OAUTH_ENABLED}
          <div style="display: flex; flex-direction: column; gap: 10px; margin-bottom: 16px;">
            <button type="button" class="btn btn-secondary" disabled={busy} onclick={() => choose("google")}>
              {t('account.signin.google', { default: 'Continue with Google' })}
            </button>
            <button type="button" class="btn btn-secondary" disabled={busy} onclick={() => choose("github")}>
              {t('account.signin.github', { default: 'Continue with GitHub' })}
            </button>
          </div>
        {/if}

        {#if busy}
          <p style="font-size: var(--text-xs); color: var(--text-muted); margin: 0 0 12px;">
            {t('account.signin.waiting', { default: 'Finish signing in in your browser, then come back here.' })}
          </p>
        {/if}
      {:else}
        <p style="font-size: var(--text-sm); color: var(--text-muted); line-height: 1.6; margin: 0 0 16px; padding: 10px 12px; border: 1px solid var(--border-primary); border-radius: var(--radius-md);">
          {#if status === "browser-mode"}
            {t('account.unavailable.browser', { default: 'Sign-in is only available in the ColimaUI desktop app — it needs the system browser and your keychain.' })}
          {:else}
            {t('account.unavailable.unconfigured', { default: 'Accounts are not configured in this build yet.' })}
          {/if}
        </p>
      {/if}

      {#if displayError}
        <p style="font-size: var(--text-xs); color: var(--color-error); margin: 0 0 12px;">{displayError}</p>
      {/if}

      <div style="display: flex; justify-content: flex-end;">
        <button type="button" class="btn btn-ghost" onclick={dismiss}>
          {t('account.signin.skip', { default: 'Skip' })}
        </button>
      </div>
    </div>
  </div>
{/if}
