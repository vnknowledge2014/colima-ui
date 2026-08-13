<script lang="ts">
  /**
   * The notification centre: what is happening, and what just happened.
   *
   * A panel beside the page rather than a page of its own. Notifications are
   * glanced at and dismissed; making them a destination would mean navigating away
   * from the work they are about — and the sidebar already carries a comment
   * warning that its nav list is at the limit of the viewport.
   */
  import {
    notificationState,
    closeNotificationPanel,
    clearFinished,
    type NotificationEntry,
  } from "../../store/notifications.svelte";
  import NotificationItem from "./NotificationItem.svelte";
  import { Inbox } from "../Icons.svelte";
  import { t } from "../../lib/i18n.svelte";

  /**
   * Running transfers first, then newest.
   *
   * The store keeps insertion order so a collapsed repeat does not jump around;
   * this view sorts because the one thing here the user can still act on should not
   * be scrolled off by chatter that arrived after it.
   */
  const isRunning = (e: NotificationEntry) =>
    e.status === "running" || e.status === "cancelling";

  const ordered = $derived.by(() => {
    // Finished entries by time, not by where they happen to sit in the store:
    // the store preserves insertion order and moves a collapsed repeat to the
    // front, so reading it directly showed rows whose timestamps went backwards.
    const finished = notificationState.entries
      .filter((e) => !isRunning(e))
      .sort((a, b) => b.timestamp - a.timestamp);
    return [...notificationState.entries.filter(isRunning), ...finished];
  });

  /**
   * Announced when a transfer reaches a terminal state.
   *
   * Deliberately **not** `aria-live` on the list itself: progress mutates `bytes`
   * about five times a second, and a live region around it queues one announcement
   * per tick. "Polite" defers an announcement, it does not merge it.
   */
  const latestOutcome = $derived.by(() => {
    const settled = notificationState.entries.find(
      (e) => e.kind === "job" && !isRunning(e)
    );
    return settled ? `${settled.title}: ${settled.status}` : "";
  });

  const hasFinished = $derived(notificationState.entries.some((e) => !isRunning(e)));

  let panel = $state<HTMLElement | null>(null);
  let closeButton = $state<HTMLButtonElement | null>(null);
  /** Where focus came from, so closing puts it back. */
  let opener: HTMLElement | null = null;

  $effect(() => {
    if (notificationState.panelOpen) {
      // Trapping focus without first moving it in means the first Tab escapes into
      // the page behind the backdrop — the trap only engages once focus is already
      // on the panel's first or last control.
      opener = document.activeElement as HTMLElement | null;
      closeButton?.focus();
    } else if (opener) {
      // Returning focus to the bell rather than the document body: otherwise the
      // next Tab restarts from the top of the page.
      opener.focus();
      opener = null;
    }
  });

  /**
   * Keep Tab inside the panel while it is open.
   *
   * It sits over the page it would otherwise let you tab into, which for a screen
   * reader means the focus order silently walks behind an opaque overlay.
   */
  function trapFocus(e: KeyboardEvent) {
    if (e.key !== "Tab" || !panel) return;
    const focusable = panel.querySelectorAll<HTMLElement>(
      'button:not([disabled]), [href], input, select, textarea, [tabindex]:not([tabindex="-1"])'
    );
    if (focusable.length === 0) return;
    const first = focusable[0];
    const last = focusable[focusable.length - 1];
    if (e.shiftKey && document.activeElement === first) {
      e.preventDefault();
      last.focus();
    } else if (!e.shiftKey && document.activeElement === last) {
      e.preventDefault();
      first.focus();
    }
  }

  function onKeydown(e: KeyboardEvent) {
    if (!notificationState.panelOpen) return;
    if (e.key === "Escape") closeNotificationPanel();
    else trapFocus(e);
  }
</script>

<!-- Bound at the window: the backdrop is never focused, so a handler there would
     only fire after a click, which already closes the panel. Must be top level;
     Svelte rejects `<svelte:window>` inside a block. -->
<svelte:window onkeydown={onKeydown} />

{#if notificationState.panelOpen}
  <div
    class="backdrop"
    aria-hidden="true"
    onclick={closeNotificationPanel}
  ></div>

  <!-- `dialog` + `aria-modal`, not a bare landmark: focus is trapped and a
       backdrop covers the page, so a screen reader's virtual cursor must be
       confined too. An `<aside>` would let it wander behind the overlay. -->
  <div
    class="panel"
    role="dialog"
    aria-modal="true"
    bind:this={panel}
    aria-label={t("notifications.title", { default: "Notifications" })}
  >
    <header class="header">
      <h2>
        {t("notifications.title", { default: "Notifications" })}
        <!-- Total, not unread: opening the panel marks everything read, so an
             unread chip here would be a number that is always zero. -->
        {#if ordered.length > 0}
          <span class="count-chip">{ordered.length}</span>
        {/if}
      </h2>
      <div class="header-actions">
        {#if hasFinished}
          <!-- Only the finished ones: a running transfer is the one thing here
               that is still actionable, and clearing it would strand it. -->
          <button class="text-action" onclick={clearFinished}>
            {t("notifications.clear", { default: "Clear" })}
          </button>
        {/if}
        <button
          class="icon-action"
          bind:this={closeButton}
          aria-label={t("common.close", { default: "Close" })}
          onclick={closeNotificationPanel}
        >×</button>
      </div>
    </header>

    {#if ordered.length === 0}
      <!-- An empty inbox rather than a sentence: the panel is a place, and a place
           with nothing in it should look empty rather than announce it. -->
      <div class="empty">
        <span class="empty-icon" aria-hidden="true">{@html Inbox}</span>
        <p class="empty-title">
          {t("notifications.empty", { default: "Inbox zero" })}
        </p>
        <p class="empty-hint">
          {t("notifications.empty_hint", {
            default: "Transfers and alerts show up here.",
          })}
        </p>
      </div>
    {:else}
      <ul class="list">
        {#each ordered as entry (entry.id)}
          <NotificationItem {entry} />
        {/each}
      </ul>
    {/if}

    <!-- Only terminal outcomes reach this, so it stays quiet while transfers run. -->
    <p class="sr-only" aria-live="polite">{latestOutcome}</p>
  </div>
{/if}

<style>
  /* Every colour here is a token. The previous rules carried literal fallbacks
     (`var(--border-color, #30363d)`, `#3b82f6`, `#161b22`) that were copied from
     an older palette and no longer matched the tokens they shadowed — so any
     theme change moved the rest of the app and left this panel behind. A missing
     token is caught by `pnpm check:tokens`; a stale fallback is caught by nobody. */
  .backdrop {
    position: fixed;
    inset: 0;
    /* Derived from the shared overlay token rather than a one-off rgba, but
       deliberately lighter than it: a translucent panel that blurs a 72%-black
       page is not glass, it is a flat dark rectangle. A side drawer also does
       not need a centred modal's dimming — the page stays visible beside it,
       which is the point of putting the panel there instead of on its own route. */
    background: color-mix(in srgb, var(--bg-overlay) 55%, transparent);
    backdrop-filter: blur(2px);
    z-index: 900;
    animation: backdrop-in var(--transition-normal);
  }
  /* Glass, matching the toasts: both are transient layers floating over the
     page, and the app already reads as one surface language. */
  .panel {
    position: fixed;
    top: 0;
    right: 0;
    bottom: 0;
    width: min(420px, 100vw);
    background: var(--glass-bg);
    backdrop-filter: var(--glass-blur);
    border-left: 1px solid var(--glass-border);
    box-shadow: var(--shadow-xl);
    display: flex;
    flex-direction: column;
    z-index: 901;
    overflow: hidden;
    animation: panel-in var(--transition-normal);
  }
  .header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 14px 16px;
    border-bottom: 1px solid var(--glass-border);
    /* No background of its own: the list below owns its scroll box, so nothing
       ever passes behind this bar. Painting --glass-bg here would stack 0.7 on
       0.7 and show as a darker two-tone strip for no reason. */
    flex: none;
  }
  .header h2 {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: var(--text-base);
    font-weight: 600;
    letter-spacing: 0.01em;
    color: var(--text-primary);
    margin: 0;
  }
  .count-chip {
    padding: 1px 7px;
    border-radius: var(--radius-full);
    background: color-mix(in srgb, var(--accent-blue) 14%, transparent);
    border: 1px solid color-mix(in srgb, var(--accent-blue) 28%, transparent);
    color: var(--accent-blue);
    font-size: var(--text-xs);
    font-weight: 600;
    font-variant-numeric: tabular-nums;
  }
  .header-actions {
    display: flex;
    align-items: center;
    gap: 4px;
  }
  /* Both header controls were bare text with no padding: a ~10px pointer target
     and no visible focus. They are the only way out of a modal that traps focus,
     so they get real hit areas. */
  .text-action,
  .icon-action {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    background: none;
    border: 1px solid transparent;
    border-radius: var(--radius-sm);
    color: var(--text-secondary);
    cursor: pointer;
    font: inherit;
    transition:
      color var(--transition-fast),
      background var(--transition-fast),
      border-color var(--transition-fast);
  }
  .text-action {
    padding: 5px 10px;
    font-size: var(--text-sm);
  }
  .icon-action {
    width: 28px;
    height: 28px;
    font-size: 1.1rem;
    line-height: 1;
  }
  .text-action:hover,
  .icon-action:hover {
    background: var(--bg-card-hover);
    border-color: var(--border-primary);
    color: var(--text-primary);
  }
  .text-action:focus-visible,
  .icon-action:focus-visible {
    outline: 2px solid var(--accent-blue);
    outline-offset: 1px;
    color: var(--text-primary);
  }
  .empty {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 4px;
    padding: 32px 24px;
    text-align: center;
    color: var(--text-secondary);
  }
  .empty-icon {
    display: block;
    width: 48px;
    height: 48px;
    margin-bottom: 8px;
    /* Faint: it is a placeholder, not a status the user has to act on. */
    opacity: 0.35;
  }
  /* The glyph is authored at 18px; let it fill the box it is given. */
  .empty-icon :global(svg) {
    width: 100%;
    height: 100%;
  }
  .empty-title {
    margin: 0;
    font-weight: 600;
    color: var(--text-primary);
  }
  .empty-hint {
    margin: 0;
    font-size: var(--text-sm);
  }
  .list {
    list-style: none;
    margin: 0;
    padding: 8px;
    display: flex;
    flex-direction: column;
    gap: 6px;
    overflow-y: auto;
    /* Rows are cards now, so the list needs the scroll box rather than sharing
       the panel's: otherwise the gap between cards collapses at the edges. */
    flex: 1;
    min-height: 0;
    /* Without this, flicking past the last notification scrolls the page behind
       the modal — which the backdrop exists to say is not interactive. */
    overscroll-behavior: contain;
  }
  @keyframes backdrop-in {
    from {
      opacity: 0;
    }
  }
  @keyframes panel-in {
    from {
      transform: translateX(16px);
      opacity: 0;
    }
  }
  /* The panel is how a screen reader user gets to a running transfer; motion is
     decoration on top of that, so it is the only part that goes. */
  @media (prefers-reduced-motion: reduce) {
    .backdrop,
    .panel {
      animation: none;
    }
  }
  .sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    margin: -1px;
    padding: 0;
    overflow: hidden;
    clip: rect(0, 0, 0, 0);
    white-space: nowrap;
    border: 0;
  }
</style>
