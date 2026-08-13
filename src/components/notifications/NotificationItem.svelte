<script lang="ts">
  /**
   * One row of the notification centre.
   *
   * Jobs and messages share this component rather than having one each: they share
   * the row's whole frame — icon, title, second line, time — and differ only in
   * what sits under the title. Two components would duplicate that frame and then
   * drift apart.
   */
  import { transferApi } from "../../lib/api/transfer";
  import { globalToast } from "../../lib/globalToast";
  import {
    markJobCancelling,
    settleJob,
    openErrorLog,
    closeNotificationPanel,
    type NotificationEntry,
  } from "../../store/notifications.svelte";
  import { reconcileTransfers } from "../../lib/transferNotifications";
  import { isAllowedAnnouncementLink, openExternal } from "../../lib/external-links";
  import { t } from "../../lib/i18n.svelte";

  interface Props {
    entry: NotificationEntry;
  }

  let { entry }: Props = $props();

  const running = $derived(entry.status === "running" || entry.status === "cancelling");

  /**
   * Percent, when a total is known at all.
   *
   * Docker reports no total for these operations, so `totalEstimate` comes from
   * image metadata and is often absent. `null` means the bar must be indeterminate
   * rather than invent a percentage.
   */
  const percent = $derived.by(() => {
    const job = entry.job;
    if (!job?.totalEstimate || job.totalEstimate <= 0) return null;
    return Math.min(100, Math.round((job.bytes / job.totalEstimate) * 100));
  });

  /**
   * Announcements are keyed by severity rather than status: nothing about them
   * is pending or failed, so a green tick would say the wrong thing and an
   * advisory would look like a completed download.
   */
  const icon = $derived(
    entry.announcement
      ? { info: "☆", warning: "⚠", critical: "⛔" }[entry.announcement.severity]
      : {
          running: "⟳",
          cancelling: "⟳",
          success: "✓",
          error: "✕",
          cancelled: "⊘",
          ended: "•",
        }[entry.status]
  );

  async function cancel() {
    const job = entry.job;
    if (!job) return;
    const wasStatus = entry.status;
    markJobCancelling(job.jobId);
    try {
      const outcome = await transferApi.cancel(job.jobId);
      // Only `cancelled` is followed by a terminal event. The other two never
      // produce one, so this row would spin at "Cancelling…" forever without
      // resolving them here.
      if (outcome !== "cancelled") {
        // `alreadyFinished` covers *any* terminal state — success, failure and
        // cancellation alike — so it says the job is over but not how it went.
        // Calling it a success would put a green tick on a failed export. The
        // backend still lists a finished transfer for about a minute, so the real
        // outcome is one request away; `unknownJob` means our view is stale, which
        // is the same request.
        await reconcileTransfers();
        // Still unresolved: the job aged out of the backend's retention before we
        // asked. It is definitely over, and `ended` is exactly "it finished, we
        // did not see how".
        if (entry.status === "cancelling") {
          settleJob(job.jobId, "ended");
        }
      }
    } catch (e) {
      // The request never landed, so nothing was cancelled. Leaving the row
      // disabled at "Cancelling…" would strand it: no event is coming, and
      // neither reconcile nor Clear will touch a job that still looks running.
      if (entry.status === "cancelling") entry.status = wasStatus;
      globalToast("error", String(e));
    }
  }

  function human(n: number): string {
    const units = ["B", "KB", "MB", "GB", "TB"];
    let value = n;
    let unit = 0;
    while (value >= 1024 && unit < units.length - 1) {
      value /= 1024;
      unit++;
    }
    return unit === 0 ? `${n} B` : `${value.toFixed(1)} ${units[unit]}`;
  }

  /**
   * The announcement's link, but only when it survives the allowlist.
   *
   * Checked before drawing the button rather than when it is clicked: a link the
   * app will not follow should not be offered in the first place. `linkUrl` is
   * the one value in this component that came off the network.
   */
  const announcementLink = $derived(
    isAllowedAnnouncementLink(entry.announcement?.linkUrl)
      ? entry.announcement?.linkUrl
      : undefined
  );

  function formatTime(ts: number): string {
    return new Date(ts).toLocaleTimeString();
  }
</script>

<li
  class="item status-{entry.status}"
  class:unread={!entry.read}
  class:announcement={!!entry.announcement}
  class:severity-critical={entry.announcement?.severity === "critical"}
  class:severity-warning={entry.announcement?.severity === "warning"}
>
  <div class="head">
    <!-- The glyph spins, not the chip: rotating the tinted square would read as
         the whole row churning rather than one operation being in flight. -->
    <span class="icon-chip" aria-hidden="true">
      <span class="glyph" class:spin={running}>{icon}</span>
    </span>
    <span class="title">
      {entry.title}{#if entry.count > 1}<span class="count">×{entry.count}</span>{/if}
    </span>
    <span class="time">{formatTime(entry.timestamp)}</span>
  </div>

  <!-- Text bindings, never `{@html}`. `title` and `detail` can come from the
       announcement feed, i.e. from the network, and this repo already has an
       `{@html} renderMarkdownHTML` pattern next door in `AiChatPanel.svelte`.
       Applying it here would turn feed content into executable HTML inside the
       webview. Svelte escapes these; keep them escaped. -->
  {#if entry.detail}
    <p class="detail">{entry.detail}</p>
  {/if}

  {#if entry.job && running}
    <div class="progress">
      <div
        class="bar"
        role="progressbar"
        aria-valuemin="0"
        aria-valuemax="100"
        aria-valuenow={percent ?? undefined}
        aria-valuetext={percent === null
          ? t("notifications.in_progress", { default: "In progress" })
          : `${percent}%`}
      >
        <!-- Indeterminate rather than pretending to a percentage it cannot know. -->
        <div
          class="fill"
          class:indeterminate={percent === null}
          style="width: {percent ?? 100}%"
        ></div>
      </div>
      <p class="meta">
        {human(entry.job.bytes)}{#if percent !== null} · {percent}%{/if}
        {#if entry.job.message} · {entry.job.message}{/if}
      </p>
    </div>
  {/if}

  <div class="actions">
    {#if entry.job && running}
      <button
        class="action danger"
        onclick={cancel}
        disabled={entry.status === "cancelling"}
      >
        {entry.status === "cancelling"
          ? t("notifications.cancelling", { default: "Cancelling…" })
          : t("notifications.cancel", { default: "Cancel" })}
      </button>
    {/if}
    {#if announcementLink}
      <!-- ↗ because this one really does open a browser. It is the same
           convention the rest of the app follows; this button was the only
           place that left the app without saying so. -->
      <button class="action" onclick={() => void openExternal(announcementLink)}>
        {t("notifications.announcement_link", { default: "Learn more" })} ↗
      </button>
    {/if}
    {#if entry.error}
      <button
        class="action"
        onclick={() => {
          // The details panel is fixed over the page this one also covers.
          closeNotificationPanel();
          openErrorLog();
        }}
      >
        {t("errors.show_details", { default: "Details" })}
      </button>
    {/if}
  </div>
</li>

<style>
  /* One colour drives the whole row: the rail, the icon chip and the progress
     fill all read `--row-accent`. Status used to be a single coloured glyph a
     few pixels wide, which is the least visible part of the row; now the state
     is legible from the edge of the panel without reading anything. */
  .item {
    --row-accent: var(--text-muted);
    position: relative;
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 10px 12px 10px 15px;
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-md);
    background: var(--bg-card);
    transition:
      border-color var(--transition-fast),
      background var(--transition-fast);
  }
  .item:hover {
    background: var(--bg-card-hover);
    border-color: var(--border-primary);
  }
  /* The rail. Inset rather than a border so the radius stays round at the
     corners, and 3px so it survives a colour the eye barely registers. */
  .item::before {
    content: "";
    position: absolute;
    /* Offset by the 1px border: an absolutely-positioned child is placed against
       the padding box, so at left:0 a sliver of border ran down the outside of
       the rail and the rail's corners sat inside the card's own radius. */
    left: -1px;
    top: -1px;
    bottom: -1px;
    width: 3px;
    border-radius: var(--radius-md) 0 0 var(--radius-md);
    background: var(--row-accent);
  }

  /* Status sets the accent; the running case is the one worth a live colour. */
  .status-running,
  .status-cancelling {
    --row-accent: var(--accent-blue);
  }
  .status-success {
    --row-accent: var(--accent-green);
  }
  .status-error {
    --row-accent: var(--accent-red);
  }
  /* `cancelled` and `ended` keep the muted default: neither is a failure, and a
     scale where every step is loud has no top — the same reasoning the severity
     tokens are written with. */

  /* Severity, not status, decides an announcement's colour: nothing about one is
     pending or failed, so a green tick would say the wrong thing. Declared after
     the status rules so it wins on equal specificity. */
  .item.announcement {
    --row-accent: var(--text-muted);
  }
  .item.announcement.severity-warning {
    --row-accent: var(--accent-yellow);
  }
  .item.announcement.severity-critical {
    --row-accent: var(--accent-red);
  }

  .item.unread {
    /* Still not a filled row — that reads as "selected" rather than "new". The
       tint is the accent at a strength you notice only in comparison with the
       row above it.

       The border mixes into --border-primary rather than --border-subtle so
       that unread stays legible when the accent is the muted default: a
       cancelled or info-announcement row mixed 7% of grey into the card is a
       shift of about five values per channel, i.e. no signal at all. Read and
       unread then differ by the border even when they cannot differ by hue. */
    background: color-mix(in srgb, var(--row-accent) 7%, var(--bg-card));
    border-color: color-mix(in srgb, var(--row-accent) 22%, var(--border-primary));
  }
  /* Declared after `.item:hover`, which shares its (0,2,0) specificity and would
     otherwise win on source order and leave unread rows without hover feedback. */
  .item.unread:hover {
    background: color-mix(in srgb, var(--row-accent) 11%, var(--bg-card-hover));
    border-color: color-mix(in srgb, var(--row-accent) 40%, var(--border-primary));
  }

  .head {
    display: flex;
    align-items: center;
    gap: 9px;
  }
  .icon-chip {
    flex: none;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 22px;
    height: 22px;
    border-radius: var(--radius-sm);
    background: color-mix(in srgb, var(--row-accent) 14%, transparent);
    color: var(--row-accent);
    font-size: var(--text-sm);
    line-height: 1;
  }
  .glyph {
    display: block;
  }
  .title {
    flex: 1;
    min-width: 0;
    font-weight: 600;
    font-size: var(--text-base);
    color: var(--text-primary);
    word-break: break-word;
  }
  /* --text-secondary, not --text-muted: muted measures 2.75:1 on a card, under
     AA, and both of these carry real information — how many times a thing
     happened, and when. Muted is kept below for the disabled control, where a
     dim label is the point and WCAG exempts it. Raising the token itself would
     be a change to every screen, so it is not made here. */
  .count {
    margin-left: 6px;
    font-size: var(--text-xs);
    color: var(--text-secondary);
    font-variant-numeric: tabular-nums;
  }
  .time {
    flex: none;
    align-self: flex-start;
    padding-top: 3px;
    font-size: var(--text-xs);
    color: var(--text-secondary);
    white-space: nowrap;
    font-variant-numeric: tabular-nums;
  }
  .detail,
  .meta {
    margin: 0;
    /* Aligned under the title rather than under the chip: the second line
       continues the sentence the title started. */
    padding-left: 31px;
    font-size: var(--text-sm);
    color: var(--text-secondary);
    word-break: break-word;
  }
  .progress {
    display: flex;
    flex-direction: column;
    gap: 5px;
    padding-left: 31px;
  }
  .bar {
    height: 5px;
    background: var(--surface-inset);
    border-radius: var(--radius-full);
    overflow: hidden;
  }
  .fill {
    height: 100%;
    border-radius: var(--radius-full);
    background: var(--row-accent);
    transition: width 200ms linear;
  }
  .fill.indeterminate {
    animation: pulse 1.2s ease-in-out infinite;
  }
  .glyph.spin {
    animation: spin 1.4s linear infinite;
  }
  @keyframes pulse {
    0%,
    100% {
      opacity: 0.35;
    }
    50% {
      opacity: 1;
    }
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .fill.indeterminate,
    .glyph.spin {
      animation: none;
    }
  }
  .actions {
    display: flex;
    gap: 6px;
    padding-left: 31px;
  }
  .actions:empty {
    display: none;
  }
  /* Padded and bordered rather than underlined text. Cancel is destructive and
     sat one careless pixel away from Details; a real button gives each its own
     target and makes the hit area match what it looks like. */
  .action {
    background: none;
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-sm);
    padding: 3px 9px;
    color: var(--text-secondary);
    cursor: pointer;
    font: inherit;
    font-size: var(--text-xs);
    font-weight: 500;
    transition:
      color var(--transition-fast),
      background var(--transition-fast),
      border-color var(--transition-fast);
  }
  .action:hover:not(:disabled) {
    background: var(--bg-card-hover);
    border-color: var(--border-primary);
    color: var(--text-primary);
  }
  .action:focus-visible {
    outline: 2px solid var(--accent-blue);
    outline-offset: 1px;
  }
  .action:disabled {
    color: var(--text-muted);
    cursor: default;
  }
  .action.danger {
    color: var(--accent-red);
    border-color: color-mix(in srgb, var(--accent-red) 28%, transparent);
  }
  .action.danger:hover:not(:disabled) {
    background: color-mix(in srgb, var(--accent-red) 12%, transparent);
    border-color: var(--accent-red);
    color: var(--accent-red);
  }
</style>
