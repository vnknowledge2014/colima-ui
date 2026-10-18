<script lang="ts">
  /**
   * Fixes applied to compose files, newest first, each still undoable.
   *
   * This exists because undo is worth more than the fix itself. A user who
   * applies a patch, restarts the app and only then finds the stack broken has
   * no way back from the panel that applied it — the backup is on disk for
   * thirty days, so the way back should be too.
   */
  import { composeApi, type ComposeFixRecord } from "../../lib/api";
  import { globalToast } from "../../lib/globalToast";
  import { t } from "../../lib/i18n.svelte";

  let records = $state<ComposeFixRecord[]>([]);
  let loaded = $state(false);

  async function load() {
    try {
      records = (await composeApi.autofixHistory()) || [];
    } catch {
      // History is a convenience. Failing to read it must not push an error at
      // someone who was diagnosing a different problem.
      records = [];
    }
    loaded = true;
  }

  async function undo(record: ComposeFixRecord) {
    try {
      await composeApi.autofixUndo(record.fix_id);
      globalToast("success", t('compose.autofix.undone', { default: 'Compose file restored.' }));
      await load();
    } catch (e) {
      globalToast("error", String(e));
    }
  }

  $effect(() => {
    if (!loaded) void load();
  });

  function shortPath(path: string): string {
    const parts = path.split("/");
    return parts.slice(-2).join("/");
  }
</script>

{#if records.length > 0}
  <div style="display: flex; flex-direction: column; gap: 6px; margin-top: 10px;">
    <div style="font-size: var(--text-xs); color: var(--text-muted);">
      {t('compose.autofix.history_title', { default: 'Applied fixes' })}
    </div>
    {#each records as record (record.fix_id)}
      <div style="display: flex; align-items: center; justify-content: space-between; gap: 8px; font-size: var(--text-xs);">
        <span style="color: var(--text-secondary); overflow: hidden; text-overflow: ellipsis; white-space: nowrap;">
          <span style="font-family: var(--font-mono);">{shortPath(record.file_path)}</span>
          — {record.explanation}
        </span>
        {#if record.undone}
          <span style="color: var(--text-muted); flex-shrink: 0;">
            {t('compose.autofix.history_undone', { default: 'undone' })}
          </span>
        {:else}
          <button class="btn btn-sm" style="flex-shrink: 0;" onclick={() => undo(record)}>
            {t('compose.autofix.undo', { default: 'Undo' })}
          </button>
        {/if}
      </div>
    {/each}
  </div>
{/if}
