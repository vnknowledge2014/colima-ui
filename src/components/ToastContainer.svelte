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
</script>

{#if toasts.length > 0}
  <div class="toast-container">
    {#each toasts as toast (toast.id)}
      <button
        class="toast-item toast-{toast.type}"
        onclick={() => toasts = toasts.filter(t => t.id !== toast.id)}
      >
        {toast.type === 'success' ? '✓' : toast.type === 'error' ? '✕' : 'ℹ'} {toast.text}
      </button>
    {/each}
  </div>
{/if}
