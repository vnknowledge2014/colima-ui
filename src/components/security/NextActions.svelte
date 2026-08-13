<script lang="ts">
  /**
   * What to do next, ranked by what it is worth.
   *
   * Each row leads to the image it is about: a list of advice you cannot act on
   * from where you are reading it is a list nobody acts on.
   */
  import { t } from "../../lib/i18n.svelte";
  import type { Level } from "../../lib/api/security";
  import type { NextActionList } from "./security-actions";

  let {
    actions,
    level,
    onSelect,
  }: {
    actions: NextActionList;
    level: Level;
    onSelect: (imageRef: string) => void;
  } = $props();
</script>

<section class="card actions">
  <h2>{t("security.next_actions", { default: "Next actions" })}</h2>

  {#if actions.items.length === 0}
    <p class="empty">
      {t("security.next_actions_empty", { default: "Nothing left to fix at this strictness" })}
      · {level.toUpperCase()}
    </p>
  {:else}
    <ul>
      {#each actions.items as action (action.id)}
        <li>
          <button class="row" onclick={() => onSelect(action.imageRef)}>
            <span class="body">
              <span class="title">{action.title}</span>
              {#if action.detail}
                <span class="detail">{action.detail}</span>
              {/if}
              <code class="ref">{action.imageRef}</code>
            </span>
            <span class="gain" class:unmeasured={action.estimatedGain === null}>
              {#if action.estimatedGain === null}
                {t("security.gain_unknown", { default: "not measured" })}
              {:else}
                +{action.estimatedGain}
              {/if}
            </span>
          </button>
        </li>
      {/each}
    </ul>

    {#if actions.remaining > 0}
      <p class="more">
        {t("security.next_actions_more", { default: "and more, not shown" })} · {actions.remaining}
      </p>
    {/if}
  {/if}
</section>

<style>
  .actions {
    padding: 16px;
  }
  h2 {
    margin: 0 0 10px;
    font-size: var(--text-base);
  }
  ul {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .row {
    display: flex;
    width: 100%;
    align-items: flex-start;
    justify-content: space-between;
    gap: 12px;
    padding: 10px;
    background: none;
    border: none;
    border-radius: 6px;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }
  .row:hover {
    background: var(--bg-secondary);
  }
  .body {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .title {
    font-size: var(--text-sm);
  }
  .detail {
    font-size: var(--text-xs);
    color: var(--text-secondary);
  }
  .ref {
    font-family: var(--font-mono, ui-monospace, monospace);
    font-size: var(--text-xs);
    color: var(--text-muted);
    word-break: break-all;
  }
  .gain {
    flex-shrink: 0;
    font-size: var(--text-sm);
    font-weight: 600;
    font-variant-numeric: tabular-nums;
    color: var(--accent-green, #22c55e);
  }
  .gain.unmeasured {
    font-weight: 400;
    color: var(--text-muted);
  }
  .empty,
  .more {
    margin: 0;
    font-size: var(--text-sm);
    color: var(--text-secondary);
  }
  .more {
    margin-top: 8px;
    color: var(--text-muted);
  }
</style>
