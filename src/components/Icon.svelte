<script lang="ts">
  import * as svgs from "./Icons.svelte";

  const ICON_ALIASES: Record<string, string> = {
    "rotate-ccw": "Refresh",
  };

  let { name, size = 14, color = "currentColor", style = "", class: className = "" }: {
    name: string;
    size?: number;
    color?: string;
    style?: string;
    class?: string;
  } = $props();

  const renderedSvg = $derived.by(() => {
    const resolved = ICON_ALIASES[name] ?? name;
    const raw = (svgs as unknown as Record<string, string>)[resolved];
    if (!raw) return "";
    return raw.replace(/<svg\b([^>]*)>/, (_match, attrs: string) => {
      const cleaned = attrs.replace(/\s+(?:width|height)="[^"]*"/g, "");
      return `<svg width="100%" height="100%"${cleaned}>`;
    });
  });
</script>

<span class="icon-wrapper {className}" style="display: inline-flex; align-items: center; justify-content: center; width: {size}px; height: {size}px; color: {color}; {style}">
  {@html renderedSvg}
</span>
