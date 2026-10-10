<script lang="ts">
  import { confirmState } from "../store/confirm.svelte";

  let open = $derived(confirmState.open);
  let options = $derived(confirmState.options);

  let title = $derived(options.title || "Confirm");
  let message = $derived(options.message);
  let confirmText = $derived(options.confirmText || "Confirm");
  let cancelText = $derived(options.cancelText || "Cancel");
  let variant = $derived(options.variant || "danger");

  const variantColors: Record<string, { bg: string; color: string; border: string }> = {
    danger: { bg: "rgba(248, 81, 73, 0.15)", color: "var(--accent-red)", border: "rgba(248, 81, 73, 0.4)" },
    warning: { bg: "rgba(210, 153, 34, 0.15)", color: "var(--accent-yellow)", border: "rgba(210, 153, 34, 0.4)" },
    info: { bg: "rgba(88, 166, 255, 0.15)", color: "var(--accent-blue)", border: "rgba(88, 166, 255, 0.4)" },
  };

  let v = $derived(variantColors[variant] || variantColors.danger);

  function handleConfirm() {
    confirmState.resolve?.(true);
    confirmState.open = false;
    confirmState.resolve = null;
  }

  function handleCancel() {
    confirmState.resolve?.(false);
    confirmState.open = false;
    confirmState.resolve = null;
  }
</script>

{#if open}
  <div
    style="position: fixed; inset: 0; z-index: 10000; background: rgba(0, 0, 0, 0.6); backdrop-filter: blur(4px); display: flex; align-items: center; justify-content: center;"
    onclick={(e) => { if (e.target === e.currentTarget) handleCancel(); }}
  >
    <div
      style="background: var(--bg-primary); border-radius: var(--radius-lg); border: 1px solid var(--border-primary); box-shadow: 0 20px 60px rgba(0,0,0,0.5); padding: 24px; min-width: 380px; max-width: 480px; animation: fadeInScale 0.15s ease-out;"
    >
      <div style="display: flex; align-items: center; gap: 10px; margin-bottom: 16px;">
        <span style="font-size: 20px; width: 36px; height: 36px; border-radius: var(--radius-md); background: {v.bg}; display: flex; align-items: center; justify-content: center; color: {v.color};">
          {#if variant === "danger"}
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"/><line x1="12" y1="9" x2="12" y2="13"/><line x1="12" y1="17" x2="12.01" y2="17"/></svg>
          {:else if variant === "warning"}
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polygon points="13 2 3 14 12 14 11 22 21 10 12 10 13 2"/></svg>
          {:else}
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"/><line x1="12" y1="16" x2="12" y2="12"/><line x1="12" y1="8" x2="12.01" y2="8"/></svg>
          {/if}
        </span>
        <h3 style="margin: 0; font-size: var(--text-base); font-weight: 600; color: var(--text-primary);">
          {title}
        </h3>
      </div>

      <p style="margin: 0 0 24px 0; font-size: var(--text-sm); color: var(--text-secondary); line-height: 1.6; white-space: pre-line;">
        {message}
      </p>

      <div style="display: flex; justify-content: flex-end; gap: 8px;">
        <button class="btn btn-ghost" onclick={handleCancel} style="font-size: var(--text-sm); padding: 8px 16px;">
          {cancelText}
        </button>
        <button
          onclick={handleConfirm}
          style="font-size: var(--text-sm); padding: 8px 20px; border-radius: var(--radius-md); border: 1px solid {v.border}; background: {v.bg}; color: {v.color}; font-weight: 600; cursor: pointer; transition: all 0.15s ease;"
          onmouseover={(e) => { e.currentTarget.style.background = v.color; e.currentTarget.style.color = "#fff"; }}
          onmouseout={(e) => { e.currentTarget.style.background = v.bg; e.currentTarget.style.color = v.color; }}
        >
          {confirmText}
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  @keyframes fadeInScale {
    from { opacity: 0; transform: scale(0.95); }
    to { opacity: 1; transform: scale(1); }
  }
</style>
