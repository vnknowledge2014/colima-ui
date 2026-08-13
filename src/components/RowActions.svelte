<script lang="ts">
  import ContextMenu from "./ContextMenu.svelte";
  import * as Icons from "./Icons.svelte";
  import { t } from "../lib/i18n.svelte";

  /** An action shown as an icon button directly on the row or card. */
  interface InlineAction {
    /** SVG markup from Icons.svelte — never user or daemon data. */
    icon: string;
    /** Used for both the tooltip and the accessible name. */
    label: string;
    onclick: () => void;
    disabled?: boolean;
    /** Maps to an accent token so colour and icon agree on the outcome. */
    tone?: "default" | "success" | "warning" | "danger";
  }

  interface MenuAction {
    label: string;
    icon?: string;
    action: () => void;
    danger?: boolean;
    disabled?: boolean;
    divider?: boolean;
  }

  let {
    inline = [] as InlineAction[],
    menu = [] as MenuAction[],
  } = $props();

  let menuPosition = $state<{ x: number; y: number } | null>(null);

  function openMenu(e: MouseEvent) {
    e.stopPropagation();
    const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
    menuPosition = { x: rect.right, y: rect.bottom + 4 };
  }
</script>

<div class="row-actions">
  {#each inline as action (action.label)}
    <button
      class="btn btn-ghost btn-icon tone-{action.tone ?? 'default'}"
      data-tooltip={action.label}
      aria-label={action.label}
      disabled={action.disabled}
      onclick={(e) => { e.stopPropagation(); action.onclick(); }}
    >{@html action.icon}</button>
  {/each}

  {#if menu.length > 0}
    <button
      class="btn btn-ghost btn-icon"
      data-tooltip={t('common.more_actions', { default: 'More actions' })}
      aria-label={t('common.more_actions', { default: 'More actions' })}
      aria-haspopup="menu"
      onclick={openMenu}
    >{@html Icons.More}</button>
  {/if}
</div>

{#if menuPosition}
  <ContextMenu
    x={menuPosition.x}
    y={menuPosition.y}
    items={menu}
    onClose={() => (menuPosition = null)}
  />
{/if}

<style>
  .row-actions {
    display: flex;
    gap: 4px;
    flex-shrink: 0;
    flex-wrap: nowrap;
  }

  .tone-success {
    color: var(--status-running);
  }

  .tone-warning {
    color: var(--accent-yellow);
  }

  .tone-danger {
    color: var(--accent-red);
  }
</style>
