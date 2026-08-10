<script lang="ts">
  import { onMount } from "svelte";
  import { onToast, type ToastMessage } from "../lib/globalToast";

  interface ActiveToast extends ToastMessage {
    progress: number; // 0–100
    intervalId?: ReturnType<typeof setInterval>;
  }

  let toasts = $state<ActiveToast[]>([]);

  const DURATION = 5000;
  const TICK = 50;

  onMount(() => {
    const unToast = onToast((toast) => {
      const active: ActiveToast = { ...toast, progress: 100 };

      active.intervalId = setInterval(() => {
        const t = toasts.find((t) => t.id === active.id);
        if (!t) return;
        t.progress -= (TICK / DURATION) * 100;
        if (t.progress <= 0) {
          clearInterval(active.intervalId);
          dismiss(active.id);
        }
      }, TICK);

      toasts = [...toasts, active];
    });
    return () => {
      toasts.forEach((t) => t.intervalId && clearInterval(t.intervalId));
      unToast();
    };
  });

  function dismiss(id: number) {
    const t = toasts.find((t) => t.id === id);
    if (t?.intervalId) clearInterval(t.intervalId);
    toasts = toasts.filter((t) => t.id !== id);
  }

  const config = {
    success: {
      icon: `<svg width="18" height="18" viewBox="0 0 20 20" fill="none"><circle cx="10" cy="10" r="9" fill="rgba(52,211,153,0.18)" stroke="rgba(52,211,153,0.6)" stroke-width="1.5"/><path d="M6.5 10l2.5 2.5 4.5-5" stroke="#34d399" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"/></svg>`,
      label: "Success",
      barColor: "var(--accent-green)",
      glowColor: "rgba(52, 211, 153, 0.12)",
      borderColor: "rgba(52, 211, 153, 0.28)",
      bgColor: "rgba(10, 24, 16, 0.92)",
      textColor: "var(--text-primary)",
      labelColor: "var(--accent-green)",
    },
    error: {
      icon: `<svg width="18" height="18" viewBox="0 0 20 20" fill="none"><circle cx="10" cy="10" r="9" fill="rgba(248,113,113,0.15)" stroke="rgba(248,113,113,0.55)" stroke-width="1.5"/><path d="M7 7l6 6M13 7l-6 6" stroke="#f87171" stroke-width="1.75" stroke-linecap="round"/></svg>`,
      label: "Error",
      barColor: "var(--accent-red)",
      glowColor: "rgba(248, 113, 113, 0.1)",
      borderColor: "rgba(248, 113, 113, 0.28)",
      bgColor: "rgba(24, 8, 8, 0.92)",
      textColor: "var(--text-primary)",
      labelColor: "var(--accent-red)",
    },
    info: {
      icon: `<svg width="18" height="18" viewBox="0 0 20 20" fill="none"><circle cx="10" cy="10" r="9" fill="rgba(96,165,250,0.15)" stroke="rgba(96,165,250,0.55)" stroke-width="1.5"/><path d="M10 9v5M10 6.5v.5" stroke="#60a5fa" stroke-width="1.75" stroke-linecap="round"/></svg>`,
      label: "Info",
      barColor: "var(--accent-blue)",
      glowColor: "rgba(96, 165, 250, 0.1)",
      borderColor: "rgba(96, 165, 250, 0.28)",
      bgColor: "rgba(8, 14, 28, 0.92)",
      textColor: "var(--text-primary)",
      labelColor: "var(--accent-blue)",
    },
  } as const;
</script>

{#if toasts.length > 0}
  <div class="toast-container" role="status" aria-live="polite">
    {#each toasts as toast (toast.id)}
      {@const c = config[toast.type as keyof typeof config] ?? config.info}
      <div
        class="toast-card"
        style="--toast-bg:{c.bgColor}; --toast-border:{c.borderColor}; --toast-glow:{c.glowColor}; --toast-bar:{c.barColor}; --toast-progress:{toast.progress}%;"
        role="alert"
      >
        <!-- Icon -->
        <span class="toast-icon" aria-hidden="true">{@html c.icon}</span>

        <!-- Content -->
        <div class="toast-body">
          <span class="toast-label" style="color:{c.labelColor}">{c.label}</span>
          <p class="toast-message">{toast.text}</p>
        </div>

        <!-- Dismiss -->
        <button
          class="toast-dismiss"
          onclick={() => dismiss(toast.id)}
          aria-label="Dismiss"
          title="Dismiss"
        >
          <svg width="12" height="12" viewBox="0 0 12 12" fill="none">
            <path d="M2 2l8 8M10 2l-8 8" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>
          </svg>
        </button>

        <!-- Progress bar -->
        <div class="toast-progress" aria-hidden="true">
          <div class="toast-progress-bar"></div>
        </div>
      </div>
    {/each}
  </div>
{/if}

<style>
  .toast-container {
    position: fixed;
    top: 20px;
    right: 20px;
    z-index: 99999;
    display: flex;
    flex-direction: column;
    gap: 10px;
    width: 340px;
    pointer-events: auto;
  }

  .toast-card {
    position: relative;
    overflow: hidden;
    display: flex;
    align-items: flex-start;
    gap: 12px;
    padding: 14px 14px 20px 14px;
    border-radius: 14px;
    background: var(--toast-bg);
    border: 1px solid var(--toast-border);
    box-shadow:
      0 0 0 1px rgba(255,255,255,0.04),
      0 8px 32px rgba(0,0,0,0.6),
      0 2px 8px rgba(0,0,0,0.4),
      inset 0 1px 0 rgba(255,255,255,0.06);
    backdrop-filter: blur(20px);
    -webkit-backdrop-filter: blur(20px);
    animation: toast-in 0.32s cubic-bezier(0.34, 1.4, 0.64, 1) forwards;
  }

  /* Subtle glow halo */
  .toast-card::before {
    content: '';
    position: absolute;
    inset: 0;
    border-radius: inherit;
    background: var(--toast-glow);
    pointer-events: none;
  }

  .toast-icon {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    margin-top: 1px;
  }

  .toast-body {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 3px;
  }

  .toast-label {
    font-size: 0.7rem;
    font-weight: 700;
    letter-spacing: 0.07em;
    text-transform: uppercase;
    line-height: 1;
    font-family: var(--font-sans);
  }

  .toast-message {
    margin: 0;
    font-size: var(--text-sm);
    font-weight: 400;
    color: var(--text-secondary);
    line-height: 1.45;
    font-family: var(--font-sans);
    word-break: break-word;
  }

  .toast-dismiss {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 22px;
    height: 22px;
    border-radius: 6px;
    background: transparent;
    border: none;
    cursor: pointer;
    color: var(--text-muted);
    padding: 0;
    font-family: inherit;
    transition: background 0.15s, color 0.15s;
    margin-top: 1px;
  }

  .toast-dismiss:hover {
    background: rgba(255, 255, 255, 0.08);
    color: var(--text-primary);
  }

  /* Progress bar */
  .toast-progress {
    position: absolute;
    bottom: 0;
    left: 0;
    right: 0;
    height: 3px;
    background: rgba(255, 255, 255, 0.05);
    border-radius: 0 0 14px 14px;
    overflow: hidden;
  }

  .toast-progress-bar {
    height: 100%;
    width: var(--toast-progress);
    background: var(--toast-bar);
    border-radius: inherit;
    transition: width 0.05s linear;
    opacity: 0.75;
  }

  @keyframes toast-in {
    from {
      opacity: 0;
      transform: translateX(100%) scale(0.9);
    }
    to {
      opacity: 1;
      transform: translateX(0) scale(1);
    }
  }
</style>
