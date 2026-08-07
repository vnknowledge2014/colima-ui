<script lang="ts">
  import * as svgs from "./Icons.svelte";
  
  let { name, size = 14, color = "currentColor", style = "", class: className = "" } = $props<{
    name: keyof typeof svgs;
    size?: number;
    color?: string;
    style?: string;
    class?: string;
  }>();
</script>

<span class="icon-wrapper {className}" style="display: inline-flex; align-items: center; justify-content: center; width: {size}px; height: {size}px; color: {color}; {style}">
  <!-- We assume (svgs as any)[name] contains the raw SVG string. We can wrap it in an SVG tag if it's just paths, but here it's full SVGs.
       The SVG will scale down if it has viewBox and no explicit width/height or if width/height are 100%. -->
  {@html (svgs as any)[name] ? (svgs as any)[name].replace('<svg ', '<svg width="100%" height="100%" ') : ''}
</span>
