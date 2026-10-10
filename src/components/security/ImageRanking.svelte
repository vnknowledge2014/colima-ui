<script lang="ts">
  /**
   * The images worth looking at first: the worst few scores, then everything
   * still unmeasured.
   *
   * Unscanned images stay in the list rather than being filtered out of it:
   * what has not been measured is part of the picture, and hiding it would make
   * a half-measured machine look fully measured.
   */
  import { t } from "../../lib/i18n.svelte";
  import { overviewRows, type ImageRow } from "./security-posture";

  let {
    rows,
    onSelect,
    onScan,
    onSeeAll,
    scanDisabled = false,
  }: {
    rows: ImageRow[];
    onSelect: (imageRef: string) => void;
    onScan: (imageRef: string) => void;
    onSeeAll: () => void;
    scanDisabled?: boolean;
  } = $props();

  const shown = $derived(overviewRows(rows));

  function statusLabel(status: ImageRow["status"]): string {
    if (status === "scanning") return t("security.status_scanning", { default: "scanning…" });
    if (status === "failed") return t("security.status_failed", { default: "scan failed" });
    return t("security.status_unscanned", { default: "not scanned" });
  }
</script>

<section class="card ranking">
  <h2>{t("security.images_heading", { default: "Images" })}</h2>
  <ul>
    {#each shown.rows as row (row.key)}
      <li>
        <button class="row" onclick={() => onSelect(row.ref)}>
          <code class="ref">{row.ref}</code>
          {#if row.score === null}
            <span class="status">{statusLabel(row.status)}</span>
          {:else}
            <span class="bar" aria-hidden="true">
              <span class="fill" style="width: {row.score}%"></span>
            </span>
            <span class="score">{row.score}</span>
          {/if}
        </button>
        {#if row.status === "unscanned" || row.status === "failed"}
          <button class="btn btn-ghost" disabled={scanDisabled} onclick={() => onScan(row.ref)}>
            {t("security.scan", { default: "Scan" })}
          </button>
        {/if}
      </li>
    {/each}
  </ul>

  {#if shown.hidden > 0}
    <!-- Says the number rather than implying the list is everything: an
         overview that quietly truncates reads as a complete inventory. -->
    <button class="see-all" onclick={onSeeAll}>
      {t("security.and_more_images", {
        default: "{count} more scored images",
        count: shown.hidden,
      })}
    </button>
  {/if}
</section>

<style>
  .ranking {
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
  li {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .row {
    display: flex;
    flex: 1;
    align-items: center;
    gap: 12px;
    min-width: 0;
    padding: 8px 10px;
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
  .ref {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: var(--font-mono, ui-monospace, monospace);
    font-size: var(--text-sm);
  }
  .bar {
    flex-shrink: 0;
    width: 80px;
    height: 6px;
    border-radius: 3px;
    background: var(--bg-app);
    overflow: hidden;
  }
  .fill {
    display: block;
    height: 100%;
    background: var(--accent-blue);
  }
  .score {
    flex-shrink: 0;
    width: 2.5ch;
    text-align: right;
    font-size: var(--text-sm);
    font-weight: 600;
    font-variant-numeric: tabular-nums;
  }
  .status {
    flex-shrink: 0;
    font-size: var(--text-xs);
    color: var(--text-muted);
  }
  .see-all {
    display: block;
    width: 100%;
    margin-top: 4px;
    padding: 8px 10px;
    background: none;
    border: none;
    border-radius: 6px;
    color: var(--text-muted);
    font: inherit;
    font-size: var(--text-xs);
    text-align: left;
    cursor: pointer;
  }
  .see-all:hover {
    background: var(--bg-secondary);
    color: var(--text-secondary);
  }
</style>
