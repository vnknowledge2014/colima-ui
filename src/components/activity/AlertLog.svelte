<script lang="ts">
  /**
   * What has actually fired.
   *
   * Kept even after its rule is deleted: the event records something that
   * happened, and removing the rule does not un-happen it. Each row carries the
   * value that tripped the threshold, because "CPU alert at 03:12" and "CPU at
   * 340% at 03:12" are different amounts of information.
   */
  import type { AlertEvent } from "../../lib/api/metrics";
  import { t } from "../../lib/i18n.svelte";

  interface Props {
    events: AlertEvent[];
  }

  let { events }: Props = $props();

  function formatValue(event: AlertEvent): string {
    if (event.metric === "mem_bytes") {
      const units = ["B", "KB", "MB", "GB", "TB"];
      let value = event.value;
      let unit = 0;
      while (value >= 1024 && unit < units.length - 1) {
        value /= 1024;
        unit++;
      }
      return `${value.toFixed(1)} ${units[unit]}`;
    }
    return `${event.value.toFixed(1)}%`;
  }

  function formatTime(ts: number): string {
    return new Date(ts).toLocaleString();
  }
</script>

{#if events.length === 0}
  <p class="none">
    {t("activity.no_alert_events", {
      default: "Nothing has fired yet. That is the intended state.",
    })}
  </p>
{:else}
  <ul class="log">
    {#each events as event (event.id)}
      <li>
        <span class="when">{formatTime(event.ts)}</span>
        <span class="what">
          <strong>{event.ruleName}</strong>
          ·
          {event.containerName || event.containerId.slice(0, 12)}
        </span>
        <span class="value">
          {formatValue(event)}
          <span class="threshold">
            {t("activity.over_threshold", { default: "over {threshold}", threshold: event.threshold })}
          </span>
        </span>
      </li>
    {/each}
  </ul>
{/if}

<style>
  .log {
    margin: 0;
    padding: 0;
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .log li {
    display: grid;
    grid-template-columns: minmax(10rem, auto) 1fr auto;
    gap: 12px;
    align-items: baseline;
    padding: 6px 0;
    border-bottom: 1px solid var(--border-subtle, #30363d);
    font-size: var(--text-sm);
  }
  .when {
    color: var(--text-muted);
    font-size: var(--text-xs);
    font-variant-numeric: tabular-nums;
  }
  .what {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .value {
    font-variant-numeric: tabular-nums;
    color: var(--accent-red, #ef4444);
    white-space: nowrap;
  }
  .threshold {
    color: var(--text-muted);
    font-size: var(--text-xs);
    margin-left: 6px;
  }
  .none {
    margin: 0;
    font-size: var(--text-sm);
    color: var(--text-secondary);
  }
</style>
