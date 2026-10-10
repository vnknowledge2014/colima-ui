<script lang="ts">
  /**
   * Renders a unified text diff. Presentation only — no fetching, no applying.
   *
   * Distinct from `DiffView.svelte`, which is a field-level before/after table
   * for structured config changes. A patch to a file is a different shape: the
   * unit is a line, and the surrounding context is what makes it reviewable.
   *
   * Shared by the compose and security patch panels so a diff looks and behaves
   * the same wherever the user meets one.
   */
  let { diff }: { diff: string } = $props();

  // The `---`/`+++` header repeats the file name the panel already shows.
  const lines = $derived(
    (diff || "").split("\n").filter((line) => !line.startsWith("--- ") && !line.startsWith("+++ ")),
  );

  function lineColor(line: string): string {
    if (line.startsWith("+")) return "var(--accent-green)";
    if (line.startsWith("-")) return "var(--accent-red)";
    return "var(--text-secondary)";
  }
</script>

<pre style="font-size: var(--text-xs); font-family: var(--font-mono); margin: 0; line-height: 1.6; overflow-x: auto; max-height: 320px;">{#each lines as line, i (i)}<span style="color: {lineColor(line)};">{line}
</span>{/each}</pre>
