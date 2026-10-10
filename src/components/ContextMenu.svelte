<script lang="ts">
  import { onMount, onDestroy } from "svelte";

  interface ContextMenuItem {
    label: string;
    icon?: string;
    action: () => void;
    danger?: boolean;
    disabled?: boolean;
    divider?: boolean;
  }

  let {
    x = 0,
    y = 0,
    items = [] as ContextMenuItem[],
    onClose = () => {}
  } = $props();

  let menuRef: HTMLDivElement | null = $state(null);
  let activeIndex = $state(-1);
  let actionableItems = $derived(items.filter(i => !i.divider && !i.disabled));
  let adjustedX = $state(0);
  let adjustedY = $state(0);

  $effect(() => {
    if (menuRef) {
      const rect = menuRef.getBoundingClientRect();
      let ax = x;
      let ay = y;
      if (x + rect.width > window.innerWidth - 8) ax = window.innerWidth - rect.width - 8;
      if (y + rect.height > window.innerHeight - 8) ay = window.innerHeight - rect.height - 8;
      if (ax < 8) ax = 8;
      if (ay < 8) ay = 8;
      adjustedX = ax;
      adjustedY = ay;
    }
  });

  function handleOutsideClick(e: MouseEvent) {
    if (menuRef && !menuRef.contains(e.target as Node)) {
      onClose();
    }
  }

  function handleKeyDown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      onClose();
      return;
    }
    if (e.key === "ArrowDown") {
      e.preventDefault();
      activeIndex = (activeIndex + 1) % actionableItems.length;
    }
    if (e.key === "ArrowUp") {
      e.preventDefault();
      activeIndex = (activeIndex - 1 + actionableItems.length) % actionableItems.length;
    }
    if (e.key === "Enter" && activeIndex >= 0) {
      e.preventDefault();
      actionableItems[activeIndex]?.action();
      onClose();
    }
  }

  onMount(() => {
    document.addEventListener("mousedown", handleOutsideClick, true);
    document.addEventListener("keydown", handleKeyDown);
  });

  onDestroy(() => {
    document.removeEventListener("mousedown", handleOutsideClick, true);
    document.removeEventListener("keydown", handleKeyDown);
  });
</script>

<div role="button" tabindex="0" class="ctx-overlay" style="position: fixed; inset: 0; z-index: 99999;" onkeydown={(e) => e.key === 'Enter' && e.currentTarget.click()} onclick={(e) => { if (e.target === e.currentTarget) onClose(); }}>
  <div bind:this={menuRef} class="ctx-menu" style="left: {adjustedX}px; top: {adjustedY}px; position: absolute;">
    {#each items as item, index (index)}
      {#if item.divider}
        <div class="ctx-divider"></div>
      {:else}
        <button
          class="ctx-item"
          class:ctx-danger={item.danger}
          class:ctx-active={activeIndex >= 0 && actionableItems[activeIndex] === item}
          disabled={item.disabled}
          onclick={(e) => { e.stopPropagation(); item.action(); onClose(); }}
          onmouseover={() => { activeIndex = actionableItems.indexOf(item); }}
        >
          {#if item.icon}
            <!-- Icons come from the Icons.svelte constants, never from user or
                 daemon data, so the markup is inlined rather than escaped. -->
            <span class="ctx-icon">{@html item.icon}</span>
          {/if}
          {item.label}
        </button>
      {/if}
    {/each}
  </div>
</div>
