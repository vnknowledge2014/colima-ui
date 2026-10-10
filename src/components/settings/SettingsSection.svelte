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

<div class="card settings-section" bind:this={el}>
  <div class="settings-section-header">
    {#if icon}
      <Icon name={icon} size={18} />
    {/if}
    {title}
  </div>
  <div class="settings-section-body">
    {#if description}
      <p class="settings-section-description">{description}</p>
    {/if}
    {@render children()}
  </div>
</div>
