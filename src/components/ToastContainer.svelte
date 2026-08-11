<script lang="ts">
  import { onMount } from "svelte";
  import { onToast, onToastUpdate, onToastRemove, dismissToast, type ToastMessage } from "../lib/globalToast";
  import { openErrorLog } from "../store/errorLog.svelte";
  import { t } from "../lib/i18n.svelte";

  // Lifecycle (expiry, eviction, collapsing) is owned by globalToast so it
  // works even when this component is not mounted. This component only
  // reflects what the module says is on screen.
  let toasts = $state<ToastMessage[]>([]);

  onMount(() => {
    const unToast = onToast((toast) => {
      toasts = [...toasts, toast];
    });
    // A repeat of a toast already on screen bumps its counter instead of
    // stacking another copy.
    const unUpdate = onToastUpdate((toast) => {
      toasts = toasts.map((t) => (t.id === toast.id ? { ...toast } : t));
    });
    const unRemove = onToastRemove((id) => {
      toasts = toasts.filter((t) => t.id !== id);
    });
    return () => {
      unToast();
      unUpdate();
      unRemove();
    };
  });
</script>

{#if toasts.length > 0}
  <div class="toast-container">
    {#each toasts as toast (toast.id)}
      <div class="toast-item toast-{toast.type}">
        <div class="toast-main">
          <span class="toast-icon">
            {toast.type === 'success' ? '✓' : toast.type === 'error' ? '✕' : 'ℹ'}
          </span>
          <div class="toast-body">
            <span class="toast-text">
              {toast.text}{#if toast.count > 1}<span class="toast-count">×{toast.count}</span>{/if}
            </span>
            {#if toast.hint}
              <span class="toast-hint">{toast.hint}</span>
            {/if}
          </div>
          <button
            class="toast-close"
            aria-label={t('common.close', { default: 'Close' })}
            onclick={() => dismissToast(toast.id)}
          >×</button>
        </div>
        {#if toast.error}
          <div class="toast-actions">
            <button class="toast-action" onclick={() => { openErrorLog(); dismissToast(toast.id); }}>
              {t('errors.show_details', { default: 'Details' })}
            </button>
          </div>
        {/if}
      </div>
    {/each}
  </div>
{/if}

<style>
  /* Position, colours and the entry animation come from
     src/styles/overlays.css. Only the parts this component added are here: a
     toast now stacks a hint line and an action row, so it lays out as a column
     rather than the single centred row the base rule assumes. */
  .toast-item {
    flex-direction: column;
    align-items: stretch;
    gap: 6px;
    cursor: default;
  }

  @media (prefers-reduced-motion: reduce) {
    .toast-item {
      animation: none;
    }
  }
  .toast-main {
    display: flex;
    align-items: flex-start;
    gap: 8px;
  }
  .toast-body {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
    flex: 1;
    text-align: left;
  }
  .toast-text {
    word-break: break-word;
  }
  .toast-hint {
    font-size: 0.85em;
    opacity: 0.75;
  }
  .toast-count {
    margin-left: 6px;
    font-size: 0.8em;
    opacity: 0.7;
  }
  .toast-close,
  .toast-action {
    background: none;
    border: none;
    color: inherit;
    cursor: pointer;
    font: inherit;
  }
  .toast-close {
    opacity: 0.6;
    padding: 0 2px;
    line-height: 1;
  }
  .toast-close:hover {
    opacity: 1;
  }
  .toast-actions {
    display: flex;
    gap: 8px;
    padding-left: 24px;
  }
  .toast-action {
    font-size: 0.85em;
    text-decoration: underline;
    opacity: 0.85;
  }
  .toast-action:hover {
    opacity: 1;
  }
</style>
