<script lang="ts">
  import { t } from "../lib/i18n.svelte";
  import type { FieldChange } from "../lib/api/colimaConfig";

  /**
   * Field-level before/after table shown before a change is written.
   *
   * Deliberately generic over `FieldChange` rather than tied to colima.yaml,
   * so any structured before/after change can use it.
   *
   * Not for file patches: those are line-level and live in
   * `UnifiedDiffView.svelte`. A field table cannot show a hunk in context.
   */
  let {
    changes,
    emptyLabel = t("diff.no_changes", { default: "No changes" }),
  }: {
    changes: FieldChange[];
    emptyLabel?: string;
  } = $props();

  const anyRestart = $derived(changes.some((c) => c.requiresRestart));

  /** An absent key reads better as a dash than as the string "null". */
  function show(value: string | null): string {
    return value ?? "—";
  }
</script>

{#if changes.length === 0}
  <p class="diff-empty">{emptyLabel}</p>
{:else}
  <div class="diff-scroll">
    <table class="diff-table">
      <thead>
        <tr>
          <th>{t("diff.field", { default: "Field" })}</th>
          <th>{t("diff.from", { default: "Current" })}</th>
          <th>{t("diff.to", { default: "New" })}</th>
        </tr>
      </thead>
      <tbody>
        {#each changes as change (change.field)}
          <tr>
            <td class="diff-field">
              <code>{change.field}</code>
              {#if change.requiresRestart}
                <span class="diff-restart-badge">
                  {t("diff.restart", { default: "restart" })}
                </span>
              {/if}
            </td>
            <td class="diff-from">{show(change.from)}</td>
            <td class="diff-to">{show(change.to)}</td>
          </tr>
        {/each}
      </tbody>
    </table>
  </div>

  {#if anyRestart}
    <p class="diff-note">
      {t("diff.restart_note", {
        default:
          "Fields marked restart take effect the next time the instance starts.",
      })}
    </p>
  {/if}
{/if}

<style>
  .diff-empty {
    margin: 0;
    font-size: var(--text-sm);
    color: var(--text-secondary);
  }
  /* The table is the widest thing on a narrow settings column, so it scrolls
     inside its own box instead of pushing the page sideways. */
  .diff-scroll {
    overflow-x: auto;
    border: 1px solid var(--border-subtle);
    border-radius: 6px;
  }
  .diff-table {
    width: 100%;
    border-collapse: collapse;
    font-size: var(--text-sm);
  }
  .diff-table th {
    text-align: left;
    padding: 8px 12px;
    font-weight: 600;
    color: var(--text-secondary);
    border-bottom: 1px solid var(--border-subtle);
    white-space: nowrap;
  }
  .diff-table td {
    padding: 8px 12px;
    border-bottom: 1px solid var(--border-subtle);
    vertical-align: top;
  }
  .diff-table tr:last-child td {
    border-bottom: none;
  }
  .diff-field code {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
  }
  .diff-restart-badge {
    margin-left: 8px;
    padding: 1px 6px;
    border-radius: 999px;
    font-size: var(--text-xs);
    background: var(--bg-primary);
    border: 1px solid var(--border-subtle);
    color: var(--text-muted);
  }
  .diff-from {
    color: var(--text-muted);
    text-decoration: line-through;
    font-family: var(--font-mono);
    font-size: var(--text-xs);
  }
  .diff-to {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    font-weight: 600;
  }
  .diff-note {
    margin: 8px 0 0;
    font-size: var(--text-xs);
    color: var(--text-secondary);
  }
</style>
