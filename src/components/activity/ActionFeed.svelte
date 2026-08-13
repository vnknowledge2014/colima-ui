<script lang="ts">
  /**
   * What was done to this machine, newest first.
   *
   * Read straight back from the local record, and exportable to JSON or CSV so
   * the log can leave the app without being retyped.
   */
  import { activityApi, type ActivityFeed, type ActivityKind, type FeedItem } from "../../lib/api";
  import { globalToast } from "../../lib/globalToast";
  import { t } from "../../lib/i18n.svelte";

  const isTauri = "__TAURI_INTERNALS__" in window;
  let exporting = $state(false);

  async function exportFeed(format: "json" | "csv") {
    exporting = true;
    try {
      let destDir = "";
      if (isTauri) {
        const dialog = await import("@tauri-apps/plugin-dialog");
        const chosen = await dialog.open({ directory: true, multiple: false });
        if (typeof chosen !== "string") return;
        destDir = chosen;
      } else {
        // Browser mode has no picker; the backend confines the write to
        // whatever folder is named here regardless.
        const typed = window.prompt(
          t("activity.actions.dest_prompt", { default: "Folder to save the history in" }),
        );
        if (!typed) return;
        destDir = typed;
      }
      // Seconds in the name: two exports a minute apart must not collide.
      const stamp = new Date().toISOString().replace(/[:.]/g, "-").slice(0, 19);
      const path = await activityApi.export(destDir, `colimaui-activity-${stamp}.${format}`, format);
      globalToast("success", path);
    } catch (e) {
      globalToast("error", String(e));
    } finally {
      exporting = false;
    }
  }

  let feed = $state<ActivityFeed | null>(null);
  let loading = $state(false);
  let kind = $state<ActivityKind | "">("");

  const KINDS: ActivityKind[] = ["destructive", "lifecycle", "task", "config"];

  async function load() {
    loading = true;
    try {
      feed = await activityApi.feed({ kind: kind || undefined, limit: 200 });
    } catch (e) {
      globalToast("error", String(e));
      feed = null;
    } finally {
      loading = false;
    }
  }

  // Re-reads whenever the filter changes; `kind` is read before the first await.
  $effect(() => {
    void kind;
    void load();
  });

  function when(ts: number): string {
    return new Date(ts).toLocaleString();
  }

  function outcomeColor(item: FeedItem): string {
    if (item.outcome === "failed") return "var(--accent-red)";
    if (item.outcome === "cancelled" || item.outcome === "denied") return "var(--accent-yellow)";
    return "var(--text-secondary)";
  }

  function kindLabel(k: ActivityKind): string {
    const labels: Record<ActivityKind, string> = {
      destructive: t("activity.actions.kind_destructive", { default: "Destructive" }),
      lifecycle: t("activity.actions.kind_lifecycle", { default: "Lifecycle" }),
      task: t("activity.actions.kind_task", { default: "Tasks" }),
      config: t("activity.actions.kind_config", { default: "Config" }),
    };
    return labels[k];
  }

  function duration(ms?: number): string {
    if (ms === undefined || ms < 1000) return "";
    return ms < 60_000 ? `${(ms / 1000).toFixed(1)}s` : `${Math.round(ms / 60_000)}m`;
  }
</script>

<div class="feed">
  <div class="filters" role="group" aria-label={t("activity.actions.filter", { default: "Filter by kind" })}>
    <button type="button" class="btn btn-ghost" class:active={kind === ""} onclick={() => (kind = "")}>
      {t("activity.actions.all", { default: "All" })}
    </button>
    {#each KINDS as k (k)}
      <button type="button" class="btn btn-ghost" class:active={kind === k} onclick={() => (kind = k)}>
        {kindLabel(k)}
      </button>
    {/each}

    <span class="spacer"></span>
    <button type="button" class="btn btn-ghost" disabled={exporting} onclick={() => exportFeed("csv")}>
      {t("activity.actions.export_csv", { default: "Export CSV" })}
    </button>
    <button type="button" class="btn btn-ghost" disabled={exporting} onclick={() => exportFeed("json")}>
      {t("activity.actions.export_json", { default: "Export JSON" })}
    </button>
  </div>

  {#if feed && feed.partial.length > 0}
    <!-- Named, never swallowed: a timeline missing a source without saying so
         reads as "nothing happened". -->
    <p class="partial">
      {t("activity.actions.partial", { default: "Some history could not be read:" })}
      {feed.partial.join("; ")}
    </p>
  {/if}

  {#if loading && !feed}
    <p class="hint-text">{t("activity.actions.loading", { default: "Reading history…" })}</p>
  {:else if feed && feed.items.length === 0}
    <p class="hint-text">
      {t("activity.actions.empty", {
        default: "Nothing recorded yet. Actions that change this machine show up here.",
      })}
    </p>
  {:else if feed}
    <ul class="rows">
      {#each feed.items as item (item.id)}
        <li>
          <span class="ts">{when(item.ts)}</span>
          <span class="what">
            <strong>{item.verb}</strong>
            <span class="target">{item.targetName || item.target || item.targetKind}</span>
            {#if item.actor === "app"}
              <!-- The distinction the whole `actor` column exists for: knowing
                   whether you did this or the app did. -->
              <span class="badge">{t("activity.actions.by_app", { default: "by the app" })}</span>
            {/if}
            {#if item.mergedFrom && item.mergedFrom.length > 0}
              <span class="badge" title={item.mergedFrom.join(", ")}>
                {t("activity.actions.merged", { default: "merged" })}
              </span>
            {/if}
          </span>
          <span class="detail" style="color: {outcomeColor(item)};">
            {item.outcome !== "ok" ? `${item.outcome} — ` : ""}{item.detail}
          </span>
          <span class="dur">{duration(item.durationMs)}</span>
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .feed {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .filters {
    display: flex;
    gap: 4px;
    flex-wrap: wrap;
    align-items: center;
  }
  /* Pushes the export apart from the filters: they are different kinds of
     control and sit at opposite ends when there is room. */
  .spacer {
    flex: 1;
    min-width: 8px;
  }
  .partial {
    margin: 0;
    font-size: var(--text-xs);
    color: var(--accent-yellow);
  }
  .rows {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
  }
  .rows li {
    display: grid;
    /* The time column is fixed so rows line up; the detail takes what is left
       and truncates rather than pushing the page sideways. */
    grid-template-columns: 160px minmax(180px, auto) 1fr 50px;
    gap: 10px;
    align-items: baseline;
    padding: 6px 0;
    border-bottom: 1px solid var(--border-subtle);
    font-size: var(--text-sm);
  }
  .rows li:last-child {
    border-bottom: none;
  }
  .ts {
    color: var(--text-muted);
    font-size: var(--text-xs);
    font-family: var(--font-mono);
    white-space: nowrap;
  }
  .target {
    color: var(--text-secondary);
    font-family: var(--font-mono);
    font-size: var(--text-xs);
  }
  .detail {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: var(--text-xs);
  }
  .dur {
    color: var(--text-muted);
    font-size: var(--text-xs);
    text-align: right;
  }
  .badge {
    margin-left: 6px;
    padding: 1px 6px;
    border-radius: 999px;
    font-size: var(--text-xs);
    background: var(--bg-primary);
    border: 1px solid var(--border-subtle);
    color: var(--text-muted);
  }
  @media (max-width: 720px) {
    .rows li {
      grid-template-columns: 1fr;
      gap: 2px;
    }
    .detail {
      white-space: normal;
    }
  }
</style>
