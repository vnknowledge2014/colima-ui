<script lang="ts">
  /**
   * What the sample did, in the order it did it.
   *
   * Grouped by kind rather than shown as one undifferentiated log, because the
   * kinds answer different questions: a process nobody expected is a conclusion
   * on its own, whereas a hundred filesystem writes under `/tmp` usually are
   * not. Timestamps stay relative to the session start — absolute wall-clock
   * time is noise when the whole session is two minutes long.
   */
  import { t } from "../../lib/i18n.svelte";
  import type { DetonationEvent, DetonationEventKind } from "../../lib/api/detonation";

  let {
    events,
    startedMs,
  }: { events: DetonationEvent[]; startedMs: number } = $props();

  const KIND_LABEL: Record<DetonationEventKind, string> = {
    lifecycle: "Session",
    process: "Process",
    filesystem: "Filesystem",
    output: "Output",
    error: "Problem",
  };

  function elapsed(tsMs: number): string {
    const secs = Math.max(0, Math.round((tsMs - startedMs) / 1000));
    const m = Math.floor(secs / 60);
    const s = secs % 60;
    return m > 0 ? `${m}m ${s}s` : `${s}s`;
  }
</script>

{#if events.length === 0}
  <p class="empty">
    {t("detonation.timeline_empty", { default: "Nothing observed yet." })}
  </p>
{:else}
  <ol class="timeline">
    <!-- Keyed by position: the log drain pushes many events sharing one
         millisecond, and two identical output lines are legitimately identical,
         so neither the timestamp nor the text is unique. The list is
         append-only, which is what makes the index stable. -->
    {#each events as event, i (i)}
      <li class="row kind-{event.kind}">
        <span class="at">{elapsed(event.tsMs)}</span>
        <span class="kind">
          {t(`detonation.kind_${event.kind}`, { default: KIND_LABEL[event.kind] })}
        </span>
        <span class="detail">{event.detail}</span>
      </li>
    {/each}
  </ol>
{/if}

<style>
  .empty {
    color: var(--text-muted);
    font-size: var(--text-xs);
    margin: 0;
    padding: 12px 0;
  }

  .timeline {
    list-style: none;
    margin: 0;
    padding: 0;
    max-height: 420px;
    overflow-y: auto;
    font-size: var(--text-xs);
  }

  .row {
    display: grid;
    grid-template-columns: 56px 88px 1fr;
    gap: 10px;
    align-items: baseline;
    padding: 5px 4px;
    border-bottom: 1px solid var(--border-subtle, rgba(128, 128, 128, 0.15));
  }

  .row:last-child {
    border-bottom: none;
  }

  .at {
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
  }

  .kind {
    text-transform: uppercase;
    letter-spacing: 0.04em;
    font-size: 10px;
    color: var(--text-secondary);
  }

  .detail {
    /* Sample output is arbitrary text and may be long or contain no spaces. */
    word-break: break-word;
    white-space: pre-wrap;
    font-family: var(--font-mono, monospace);
  }

  /* A spawned process and a failure are the two things worth finding by eye. */
  .kind-process .kind,
  .kind-process .detail {
    color: var(--warning, #d08770);
  }

  .kind-error .kind,
  .kind-error .detail {
    color: var(--danger, #bf616a);
  }
</style>
