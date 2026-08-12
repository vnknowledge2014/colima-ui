<script lang="ts">
  import type { Snippet } from "svelte";
  import Icon from "../Icon.svelte";

  // Shared Settings layout: a titled header bar visually separated from its
  // content, so every section on the page reads as one system instead of each
  // inventing its own heading style.
  let { title, icon, description, el = $bindable(), children } = $props<{
    title: string;
    icon?: string;
    description?: string;
    /** Optional: expose the root card element (e.g. for scroll-into-view). */
    el?: HTMLDivElement | null;
    children: Snippet;
  }>();
</script>

<div class="card" bind:this={el} style="margin-bottom: 24px; padding: 0;">
  <div style="padding: 16px 20px; border-bottom: 1px solid var(--border-primary); font-weight: 600; font-size: var(--text-lg); display: flex; align-items: center; gap: 8px;">
    {#if icon}
      <Icon name={icon} size={18} />
    {/if}
    {title}
  </div>
  <div style="padding: 24px 20px;">
    {#if description}
      <p style="font-size: var(--text-sm); color: var(--text-secondary); margin: 0 0 20px;">{description}</p>
    {/if}
    {@render children()}
  </div>
</div>
