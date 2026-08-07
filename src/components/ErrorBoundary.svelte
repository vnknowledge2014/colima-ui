<script lang="ts">
  import { onMount, onDestroy } from "svelte";

  let { children } = $props<{ children: any }>();

  let hasError = $state(false);
  let errorMsg = $state("");
  let errorStack = $state("");

  function handleError(event: ErrorEvent) {
    hasError = true;
    errorMsg = event.message || "Unknown error";
    errorStack = event.error?.stack || "";
    event.preventDefault(); // Stop propagation
  }

  function handleUnhandledRejection(event: PromiseRejectionEvent) {
    hasError = true;
    errorMsg = event.reason?.message || String(event.reason) || "Unhandled Promise Rejection";
    errorStack = event.reason?.stack || "";
    event.preventDefault(); // Stop propagation
  }

  onMount(() => {
    window.addEventListener("error", handleError);
    window.addEventListener("unhandledrejection", handleUnhandledRejection);
  });

  onDestroy(() => {
    window.removeEventListener("error", handleError);
    window.removeEventListener("unhandledrejection", handleUnhandledRejection);
  });

  function resetError() {
    hasError = false;
    errorMsg = "";
    errorStack = "";
    window.location.reload();
  }
</script>

{#if hasError}
  <div style="padding: 40px; display: flex; flex-direction: column; align-items: center; justify-content: center; height: 100vh; background: var(--bg-primary); color: var(--text-primary); gap: 20px;">
    <div style="font-size: 48px; color: var(--accent-red);">⚠️</div>
    <h1 style="margin: 0; font-size: var(--text-2xl);">Ứng dụng đã gặp lỗi</h1>
    <div style="background: var(--bg-elevated); padding: 20px; border-radius: var(--radius-lg); border: 1px solid var(--border-color); max-width: 800px; width: 100%; overflow: auto;">
      <h3 style="margin-top: 0; color: var(--accent-red);">{errorMsg}</h3>
      {#if errorStack}
        <pre style="margin: 0; font-family: monospace; font-size: var(--text-xs); color: var(--text-muted); white-space: pre-wrap;">{errorStack}</pre>
      {/if}
    </div>
    <button class="btn btn-primary" onclick={resetError} style="padding: 12px 24px;">Tải lại ứng dụng</button>
  </div>
{:else}
  {@render children()}
{/if}
