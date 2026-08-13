<script lang="ts">
  /**
   * Propose, review and apply a deterministic fix for a broken compose file.
   *
   * The panel is deliberately unexcited about its own capability. Roughly a
   * third of real compose failures can be repaired without guessing at intent,
   * so "no fix available" is the ordinary outcome and is stated plainly rather
   * than hidden behind a spinner that goes nowhere.
   *
   * Two things are always visible before the apply button: the exact diff, and
   * whether the fix costs the user their comments.
   */
  import { composeApi, type ComposeAutofixProposal, type ComposePatch, type ComposeFixRecord } from "../../lib/api";
  import { globalToast } from "../../lib/globalToast";
  import { t } from "../../lib/i18n.svelte";
  import Icon from "../Icon.svelte";
  import UnifiedDiffView from "../UnifiedDiffView.svelte";

  let { composeFile } = $props<{ composeFile: string }>();

  let proposing = $state(false);
  let applying = $state(false);
  let proposal = $state<ComposeAutofixProposal | null>(null);
  let applied = $state<ComposeFixRecord | null>(null);
  let error = $state("");

  // Each patch carries the whole file, so the last one is the complete fix and
  // the earlier steps exist only to explain how it got there.
  const finalPatch = $derived<ComposePatch | null>(
    proposal && proposal.patches.length > 0 ? proposal.patches[proposal.patches.length - 1] : null,
  );

  async function propose() {
    proposing = true;
    error = "";
    applied = null;
    try {
      proposal = await composeApi.autofixPropose(composeFile);
    } catch (e) {
      error = String(e);
      proposal = null;
    } finally {
      proposing = false;
    }
  }

  async function apply() {
    if (!finalPatch) return;
    applying = true;
    try {
      applied = await composeApi.autofixApply(composeFile, finalPatch);
      globalToast("success", t('compose.autofix.applied', { default: 'Fix applied. You can undo it.' }));
      proposal = null;
    } catch (e) {
      // The backend re-runs its key check here, so a refusal at this point is
      // worth showing verbatim rather than summarising away.
      globalToast("error", String(e));
    } finally {
      applying = false;
    }
  }

  async function undo() {
    if (!applied) return;
    try {
      await composeApi.autofixUndo(applied.fix_id);
      applied = null;
      globalToast("success", t('compose.autofix.undone', { default: 'Compose file restored.' }));
    } catch (e) {
      globalToast("error", String(e));
    }
  }
</script>

<div style="display: flex; flex-direction: column; gap: 10px;">
  {#if applied}
    <div style="display: flex; align-items: center; gap: 8px; justify-content: space-between;">
      <span style="font-size: var(--text-sm); color: var(--text-secondary);">
        {applied.explanation}
      </span>
      <button class="btn btn-sm" onclick={undo}>
        <Icon name="rotate-ccw" size={14} />
        {t('compose.autofix.undo', { default: 'Undo' })}
      </button>
    </div>
  {:else if !proposal}
    <button class="btn btn-sm btn-primary" onclick={propose} disabled={proposing}>
      {proposing
        ? t('compose.autofix.proposing', { default: 'Looking for a fix…' })
        : t('compose.autofix.propose', { default: 'Propose a fix' })}
    </button>
  {:else if !finalPatch}
    <!-- The common case, stated rather than dressed up. -->
    <div style="font-size: var(--text-sm); color: var(--text-secondary);">
      {t('compose.autofix.none', { default: 'No safe automatic fix for this error. The diagnosis above explains the cause; this one needs a human edit.' })}
    </div>
  {:else}
    <div style="display: flex; align-items: center; gap: 8px; flex-wrap: wrap;">
      <span style="font-size: var(--text-sm); color: var(--text-primary);">{finalPatch.explanation}</span>
      <span style="font-size: var(--text-xs); color: var(--text-muted);">
        {t('compose.autofix.confidence', { default: 'confidence' })}: {Math.round(finalPatch.confidence * 100)}%
        · {finalPatch.source}
      </span>
    </div>

    {#if !finalPatch.comments_preserved}
      <!-- Never applied without saying so: YAML round-tripping cannot keep
           comments, and the product must not imply otherwise. -->
      <div style="font-size: var(--text-xs); color: var(--accent-yellow);">
        {t('compose.autofix.comments_lost', { default: 'This fix rewrites the file structure and will remove your comments.' })}
      </div>
    {/if}

    <UnifiedDiffView diff={finalPatch.unified_diff} />

    {#if !proposal.resolved}
      <div style="font-size: var(--text-xs); color: var(--text-muted);">
        {t('compose.autofix.partial', { default: 'This fixes one problem but the file still fails validation:' })}
        <span style="font-family: var(--font-mono);">{proposal.remaining_error}</span>
      </div>
    {/if}

    <div style="display: flex; gap: 8px;">
      <button class="btn btn-sm btn-primary" onclick={apply} disabled={applying}>
        {applying
          ? t('compose.autofix.applying', { default: 'Applying…' })
          : t('compose.autofix.apply', { default: 'Apply fix' })}
      </button>
      <button class="btn btn-sm" onclick={() => (proposal = null)}>
        {t('compose.autofix.cancel', { default: 'Cancel' })}
      </button>
    </div>
  {/if}

  {#if proposal && proposal.refused.length > 0}
    <!-- A refusal here is a defect in a fixer, not user error. Surfaced so it
         is reported rather than silently swallowed. -->
    <div style="font-size: var(--text-xs); color: var(--accent-red);">
      {t('compose.autofix.refused', { default: 'A proposed fix was blocked because it would have removed:' })}
      {proposal.refused.map((r) => r.path).join(", ")}
    </div>
  {/if}

  {#if error}
    <div style="font-size: var(--text-xs); color: var(--accent-red);">{error}</div>
  {/if}
</div>
