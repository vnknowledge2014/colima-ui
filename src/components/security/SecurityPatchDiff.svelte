<script lang="ts">
  /**
   * Concrete Dockerfile edits for the hardening rules that have one.
   *
   * The Dockerfile has to be named by the user: an image does not record which
   * file built it, and guessing would offer to edit the wrong file. Only the
   * path travels — the backend reads it.
   *
   * Most patches here are review-only on purpose. Adding a `USER` that the base
   * image does not define, or a health check against a port that does not speak
   * HTTP, turns a working build into a broken one. Those are shown as exact
   * diffs with the risk named, and the apply button is not offered for them.
   */
  import { securityApi, composeApi, type SecurityPatch, type ComposeFixRecord } from "../../lib/api";
  import { globalToast } from "../../lib/globalToast";
  import { t } from "../../lib/i18n.svelte";
  import UnifiedDiffView from "../UnifiedDiffView.svelte";

  let dockerfilePath = $state("");
  let patches = $state<SecurityPatch[] | null>(null);
  let loading = $state(false);
  let applied = $state<ComposeFixRecord | null>(null);
  let error = $state("");

  async function propose() {
    if (!dockerfilePath.trim()) return;
    loading = true;
    error = "";
    applied = null;
    try {
      patches = await securityApi.autofixPropose(dockerfilePath.trim());
    } catch (e) {
      error = String(e);
      patches = null;
    } finally {
      loading = false;
    }
  }

  async function apply(entry: SecurityPatch) {
    try {
      // The compose apply path, reused rather than duplicated: one place holds
      // backup, undo and the pre-write check.
      applied = await composeApi.autofixApply(dockerfilePath.trim(), entry.patch);
      globalToast("success", t('security.autofix.applied', { default: 'Dockerfile patched. You can undo it.' }));
      patches = null;
    } catch (e) {
      globalToast("error", String(e));
    }
  }

  async function undo() {
    if (!applied) return;
    try {
      await composeApi.autofixUndo(applied.fix_id);
      applied = null;
      globalToast("success", t('security.autofix.undone', { default: 'Dockerfile restored.' }));
    } catch (e) {
      globalToast("error", String(e));
    }
  }
</script>

<div style="display: flex; flex-direction: column; gap: 10px;">
  <div style="display: flex; gap: 8px; align-items: center;">
    <input
      class="input"
      style="flex: 1; font-family: var(--font-mono); font-size: var(--text-xs);"
      placeholder={t('security.autofix.path_placeholder', { default: '/path/to/Dockerfile' })}
      bind:value={dockerfilePath}
    />
    <button class="btn btn-sm btn-primary" onclick={propose} disabled={loading || !dockerfilePath.trim()}>
      {loading
        ? t('security.autofix.reading', { default: 'Reading…' })
        : t('security.autofix.propose', { default: 'Propose edits' })}
    </button>
  </div>

  {#if applied}
    <div style="display: flex; align-items: center; justify-content: space-between; gap: 8px;">
      <span style="font-size: var(--text-sm); color: var(--text-secondary);">{applied.explanation}</span>
      <button class="btn btn-sm" onclick={undo}>{t('security.autofix.undo', { default: 'Undo' })}</button>
    </div>
  {/if}

  {#if patches && patches.length === 0}
    <div style="font-size: var(--text-sm); color: var(--text-secondary);">
      {t('security.autofix.none', { default: 'Nothing in this Dockerfile can be repaired mechanically. The ranked list above explains what to change by hand.' })}
    </div>
  {/if}

  {#each patches ?? [] as entry (entry.rule_id)}
    <div class="card" style="padding: 10px; display: flex; flex-direction: column; gap: 6px;">
      <div style="display: flex; align-items: baseline; gap: 8px; flex-wrap: wrap;">
        <span style="font-family: var(--font-mono); font-size: var(--text-xs); color: var(--text-muted);">
          {entry.rule_id}
        </span>
        <span style="font-size: var(--text-sm);">{entry.patch.explanation}</span>
      </div>

      {#if entry.risk_note}
        <!-- Named, not buried: this is why the apply button is absent. -->
        <div style="font-size: var(--text-xs); color: var(--accent-yellow);">{entry.risk_note}</div>
      {/if}

      <UnifiedDiffView diff={entry.patch.unified_diff} />

      {#if entry.auto_applicable}
        <div>
          <button class="btn btn-sm btn-primary" onclick={() => apply(entry)}>
            {t('security.autofix.apply', { default: 'Apply' })}
          </button>
        </div>
      {:else}
        <div style="font-size: var(--text-xs); color: var(--text-muted);">
          {t('security.autofix.review_only', { default: 'Copy this into your Dockerfile once the note above is settled.' })}
        </div>
      {/if}
    </div>
  {/each}

  {#if error}
    <div style="font-size: var(--text-xs); color: var(--accent-red);">{error}</div>
  {/if}
</div>
