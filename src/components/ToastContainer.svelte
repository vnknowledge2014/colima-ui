<script lang="ts">
  import { onMount } from "svelte";
  import { onToast, type ToastMessage } from "../lib/globalToast";

  let toasts = $state<ToastMessage[]>([]);

  onMount(() => {
    const unToast = onToast((toast) => {
      toasts = [...toasts, toast];
      setTimeout(() => {
        toasts = toasts.filter((t) => t.id !== toast.id);
      }, 5000);
    });
    return unToast;
  });

  function dismiss(id: number) {
    toasts = toasts.filter((t) => t.id !== id);
  }

  const icons: Record<string, string> = {
    success: `<svg width="15" height="15" viewBox="0 0 16 16" fill="none"><circle cx="8" cy="8" r="7" stroke="currentColor" stroke-width="1.5"/><path d="M5 8l2 2 4-4" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/></svg>`,
    error:   `<svg width="15" height="15" viewBox="0 0 16 16" fill="none"><circle cx="8" cy="8" r="7" stroke="currentColor" stroke-width="1.5"/><path d="M5.5 5.5l5 5M10.5 5.5l-5 5" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/></svg>`,
    info:    `<svg width="15" height="15" viewBox="0 0 16 16" fill="none"><circle cx="8" cy="8" r="7" stroke="currentColor" stroke-width="1.5"/><path d="M8 7v4M8 5.25v.5" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/></svg>`,
  };
</script>

{#if toasts.length > 0}
  <div class="toast-container" role="status" aria-live="polite" aria-label="Notifications">
    {#each toasts as toast (toast.id)}
      <button
        class="toast-item toast-{toast.type}"
        onclick={() => dismiss(toast.id)}
        aria-label="Dismiss: {toast.text}"
        title="Click to dismiss"
      >
        <span class="toast-icon" aria-hidden="true">{@html icons[toast.type]}</span>
        <span class="toast-text">{toast.text}</span>
        <span class="toast-close" aria-hidden="true">×</span>
      </button>
    {/each}
  </div>
{/if}

<style>
  .toast-icon {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .toast-text {
    flex: 1;
    line-height: 1.4;
  }

  .toast-close {
    flex-shrink: 0;
    font-size: 17px;
    line-height: 1;
    opacity: 0.35;
    transition: opacity 0.15s;
    margin-left: 4px;
  }

  .toast-item:hover .toast-close {
    opacity: 0.75;
  }
</style>
