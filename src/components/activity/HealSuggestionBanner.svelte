<script lang="ts">
  /**
   * What self-healing noticed and is waiting on you to decide.
   *
   * ## Only what is still worth acting on
   *
   * A suggestion is advice about a moment, and a day-old one about a container
   * that has since been fixed is noise. Entries older than a day are dropped,
   * and only the latest suggestion per rule and container is kept — a
   * crash-looping container produces one line, not forty.
   *
   * ## Nothing to dismiss
   *
   * There is no dismiss button, because a dismissal would have to be stored,
   * and a table of "advice the user waved away" earns its keep only once there
   * is something to do with it. Suggestions age out on their own instead.
   *
   * ## It does not offer to act
   *
   * The banner reports and links to the settings that govern it. Putting a
   * "do it now" button here would be a second path to the same action that the
   * executor guards with a quota and a kill switch.
   */
  import { onMount } from "svelte";
  import { selfHealApi, type HealLogEntry } from "../../lib/api/self-heal";
  import { t } from "../../lib/i18n.svelte";

  let { onOpenSettings }: { onOpenSettings?: () => void } = $props();

  const DAY_MS = 24 * 60 * 60 * 1000;

  let pending = $state<HealLogEntry[]>([]);

  onMount(load);

  async function load() {
    try {
      const log = await selfHealApi.recentLog(100);
      const cutoff = Date.now() - DAY_MS;
      const latest = new Map<string, HealLogEntry>();
      for (const e of log) {
        if (e.outcome !== "suggested" || e.ts < cutoff) continue;
        // `recentLog` is newest first, so the first hit for a key is the one
        // to keep.
        const key = `${e.ruleId}:${e.containerId}`;
        if (!latest.has(key)) latest.set(key, e);
      }
      pending = [...latest.values()];
    } catch {
      // A banner that cannot load has nothing to say. Failing loudly here would
      // put an error over the page the user actually came to read.
      pending = [];
    }
  }
</script>

{#if pending.length > 0}
  <aside class="banner" aria-label={t("self_heal.suggestions", { default: "Self-healing suggestions" })}>
    <div class="head">
      <strong>
        {t("self_heal.suggestions_heading", {
          default: "Self-healing has {count} suggestions",
          count: pending.length,
        })}
      </strong>
      {#if onOpenSettings}
        <button class="btn btn-ghost" onclick={onOpenSettings}>
          {t("self_heal.open_settings", { default: "Rules" })}
        </button>
      {/if}
    </div>
    <ul>
      {#each pending as entry (entry.id)}
        <li>
          {#if entry.containerName}
            <code>{entry.containerName}</code>
          {/if}
          <span>{entry.detail}</span>
        </li>
      {/each}
    </ul>
  </aside>
{/if}

<style>
  .banner {
    padding: 10px 12px;
    margin-bottom: 12px;
    border: 1px solid var(--border-primary);
    border-left: 3px solid var(--accent-blue);
    border-radius: var(--radius-md);
    background: var(--bg-card);
  }
  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    font-size: var(--text-sm);
  }
  ul {
    margin: 6px 0 0;
    padding: 0;
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  li {
    display: flex;
    align-items: baseline;
    gap: 8px;
    font-size: var(--text-xs);
    color: var(--text-muted);
  }
  code {
    flex-shrink: 0;
    font-family: var(--font-mono, ui-monospace, monospace);
    color: var(--text-secondary);
  }
</style>
