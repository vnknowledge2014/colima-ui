<script lang="ts">
  /**
   * Runtime events, as Falco reported them.
   *
   * # The state banner is the feature
   *
   * An empty list has two very different meanings — "nothing happened" and
   * "nothing is being watched" — and only one of them is good news. Falco can
   * be installed, running and healthy while loading zero rules, in which case it
   * detects nothing and says so nowhere. So the state of Falco is stated above
   * the list at all times, and an empty list is never presented as reassurance.
   *
   * # What this component may not claim
   *
   * Coverage belongs to the user's Falco rules, not to ColimaUI. Nothing here
   * says "you are protected", "no threats found", or anything else that implies
   * this app is doing the detecting.
   */
  import { onDestroy, onMount } from "svelte";
  import ProGate from "../ProGate.svelte";
  import { globalToast } from "../../lib/globalToast";
  import { t } from "../../lib/i18n.svelte";
  import { aiApi } from "../../lib/api";
  import { settingsState } from "../../lib/settingsStore.svelte";
  import {
    falcoApi,
    canShowEvents,
    priorityRank,
    type FalcoEvent,
    type FalcoState,
  } from "../../lib/api/falco";

  let falco = $state<FalcoState | null>(null);
  let events = $state<FalcoEvent[]>([]);
  let watching = $state(false);
  let loading = $state(true);
  let minPriority = $state("notice");
  let ruleFilter = $state("");
  let selected = $state<FalcoEvent | null>(null);
  let poll: ReturnType<typeof setInterval> | null = null;

  const shown = $derived(
    events
      .filter((e) => priorityRank(e.priority) >= priorityRank(minPriority))
      .filter((e) =>
        ruleFilter.trim() === ""
          ? true
          : e.rule.toLowerCase().includes(ruleFilter.trim().toLowerCase())
      )
  );

  /** Events hidden by the filters — shown as a count, never silently dropped. */
  const hidden = $derived(events.length - shown.length);

  async function refresh() {
    try {
      const [s, w] = await Promise.all([falcoApi.state(), falcoApi.watching()]);
      falco = s;
      watching = w;
      if (canShowEvents(s.status)) events = await falcoApi.events(500);
    } catch (e) {
      globalToast("error", String(e));
    } finally {
      loading = false;
    }
  }

  async function toggleWatch() {
    try {
      watching = await falcoApi.setWatching(!watching);
      if (watching) await refresh();
    } catch (e) {
      globalToast("error", String(e));
    }
  }

  onMount(async () => {
    await refresh();
    // Refreshes while stopped too, slowly. The banner above this list is often
    // telling the user to go and configure something; if the page only looked
    // again while reading, following those instructions would appear to have
    // done nothing until they navigated away and back.
    let tick = 0;
    poll = setInterval(() => {
      tick += 1;
      if (watching || tick % 6 === 0) void refresh();
    }, 5000);
  });

  onDestroy(() => {
    if (poll !== null) clearInterval(poll);
  });

  function when(tsMs: number): string {
    return new Date(tsMs).toLocaleTimeString();
  }

  // --- Optional AI explanation -------------------------------------------
  //
  // A button, never automatic. It is the user's key and the user's money, and
  // an explanation of every event as it arrives would be both expensive and
  // noise. The payload can be read before it is sent — "Show what would be
  // sent" builds it without contacting the provider — same order of operations
  // as compose diagnosis and image triage.
  let explaining = $state(false);
  let showPayload = $state(false);
  /**
   * Keyed by the event it belongs to.
   *
   * These were flat strings, so opening event B showed the explanation
   * generated for event A — the worst possible bug in a feature whose whole
   * job is attributing behaviour to the right container.
   */
  let explanation = $state<{ id: number; text: string } | null>(null);
  let payloadPreview = $state<{ id: number; text: string; count: number; omitted: number } | null>(
    null
  );

  const shownExplanation = $derived(
    selected && explanation?.id === selected.id ? explanation.text : ""
  );
  const shownPayload = $derived(
    selected && payloadPreview?.id === selected.id ? payloadPreview : null
  );

  /**
   * The events to explain together, centred on the clicked one.
   *
   * `slice(0, 25)` over a newest-first list took the 25 newest matches, which
   * for a busy container excluded the very event the user clicked. Sorting by
   * distance in time keeps the clicked event first and its neighbours next.
   */
  function relatedTo(event: FalcoEvent): number[] {
    const near = events
      .filter(
        (e) =>
          Math.abs(e.tsMs - event.tsMs) < 60_000 &&
          (e.containerLabel ?? null) === (event.containerLabel ?? null)
      )
      .sort((a, b) => Math.abs(a.tsMs - event.tsMs) - Math.abs(b.tsMs - event.tsMs))
      .slice(0, 25);

    const ids = near.map((e) => e.id);
    return ids.includes(event.id) ? ids : [event.id, ...ids.slice(0, 24)];
  }

  /** Build the prompt and show it, without sending anything. */
  async function buildPreview(event: FalcoEvent) {
    try {
      const built = await falcoApi.explainPayload(relatedTo(event));
      payloadPreview = {
        id: event.id,
        text: built.payload,
        count: built.eventCount,
        omitted: built.omitted,
      };
      showPayload = true;
    } catch (e) {
      globalToast("error", String(e));
    }
  }

  async function explain(event: FalcoEvent) {
    const provider = settingsState["ai_provider"];
    const model = settingsState["ai_model"];
    if (!provider || !model) {
      globalToast(
        "error",
        t("falco.no_ai", { default: "Configure an AI provider in Settings first." })
      );
      return;
    }

    explaining = true;
    explanation = null;
    try {
      const built = await falcoApi.explainPayload(relatedTo(event));
      // Recorded before the request, so "show what would be sent" is true of
      // what was actually sent rather than becoming available only afterwards.
      payloadPreview = {
        id: event.id,
        text: built.payload,
        count: built.eventCount,
        omitted: built.omitted,
      };

      // The ids are resolved server-side against its own recent window, so an
      // event old enough to have aged out resolves to nothing. Sending anyway
      // charges the user's API key to explain an empty list.
      if (built.eventCount === 0) {
        globalToast(
          "error",
          t("falco.nothing_to_explain", {
            default: "These events are no longer in the recent window.",
          })
        );
        return;
      }

      const text = await aiApi.chat(
        provider,
        model,
        settingsState["ai_api_key"] || "",
        [{ role: "user", content: built.payload }],
        settingsState["ai_endpoint"] || ""
      );
      explanation = { id: event.id, text };
    } catch (e) {
      globalToast("error", String(e));
    } finally {
      explaining = false;
    }
  }
</script>

{#if loading}
  <p class="muted">{t("falco.checking", { default: "Checking Falco…" })}</p>
{:else if falco}
  <!-- Always rendered. See the component note: an empty list is ambiguous
       without it. -->
  <div class="state state-{falco.status}" role="note">
    {#if falco.status === "missing"}
      <strong>{t("falco.missing_title", { default: "Falco is not installed" })}</strong>
      <p>
        {t("falco.missing_body", {
          default:
            "ColimaUI shows runtime events; Falco produces them. It runs inside the Linux VM, not on macOS. The knowledge base has an install guide for Colima.",
        })}
      </p>
    {:else if falco.status === "not_falco"}
      <strong>{t("falco.not_falco_title", { default: "That is a different 'falco'" })}</strong>
      <p>
        {t("falco.not_falco_body", {
          default:
            "A program named falco was found, but it is not the CNCF runtime security tool — most likely the Fastly VCL linter of the same name. Installing via Homebrew gets the wrong one.",
        })}
      </p>
    {:else if falco.status === "installed"}
      <strong>{t("falco.installed_title", { default: "Falco is installed but not running" })}</strong>
      <p>{t("falco.installed_body", { default: "Start its service inside the VM to begin producing events." })}</p>
    {:else if falco.status === "running_without_rules"}
      <strong>
        {t("falco.norules_title", { default: "Falco is running with no rules loaded" })}
      </strong>
      <p>
        {t("falco.norules_body", {
          default:
            "It is running and looks healthy, but it has no rules, so it is detecting nothing. An empty list below does not mean nothing happened.",
        })}
      </p>
    {:else if falco.status === "output_not_configured"}
      <strong>
        {t("falco.output_title", { default: "Falco is running, but not writing events to a file" })}
      </strong>
      <p>
        {t("falco.output_body", {
          default:
            "Falco is watching your system — ColimaUI just cannot read its output. Enable JSON file output in falco.yaml to see events here.",
        })}
      </p>
    {:else}
      <strong>
        {t("falco.ready_title", { default: "Reading events from Falco" })}
        {#if falco.version}<span class="version">v{falco.version}</span>{/if}
      </strong>
      <p>
        {t("falco.ready_body", {
          default: "Coverage is whatever your Falco rules cover — {count} loaded.",
        }).replace("{count}", String(falco.ruleCount))}
      </p>
    {/if}
  </div>

  <ProGate
    variant="preview"
    feature={t("falco.feature", { default: "Runtime events" })}
    description={t("falco.desc", {
      default:
        "See what Falco detects on your containers, named by compose service rather than container id, with high-priority events raised as alerts.",
    })}
  >
    {#snippet preview()}
      <div class="example">
        <div class="example-label">{t("falco.example", { default: "Example" })}</div>
        <div class="example-row">12:04:31 · CRITICAL · web/api · Terminal shell in container</div>
        <div class="example-row">12:04:33 · WARNING · web/db · Read sensitive file untrusted</div>
      </div>
    {/snippet}

    <div class="controls">
      <button class="btn" onclick={toggleWatch}>
        {watching
          ? t("falco.stop", { default: "Stop reading" })
          : t("falco.start", { default: "Start reading" })}
      </button>

      <label class="field">
        <span>{t("falco.min_priority", { default: "Minimum priority" })}</span>
        <select bind:value={minPriority}>
          <option value="debug">debug</option>
          <option value="informational">informational</option>
          <option value="notice">notice</option>
          <option value="warning">warning</option>
          <option value="error">error</option>
          <option value="critical">critical</option>
        </select>
      </label>

      <label class="field grow">
        <span>{t("falco.filter_rule", { default: "Rule contains" })}</span>
        <input bind:value={ruleFilter} placeholder="shell" />
      </label>
    </div>
  </ProGate>

  {#if canShowEvents(falco.status)}
    {#if shown.length === 0}
      <p class="muted">
        {events.length === 0
          ? t("falco.no_events", { default: "No events recorded yet." })
          : t("falco.all_filtered", { default: "Every recorded event is hidden by the filters." })}
      </p>
    {:else}
      <ul class="events">
        {#each shown as event (event.id)}
          <li>
            <button
              class="event p-{event.priority.toLowerCase()}"
              onclick={() => (selected = selected?.id === event.id ? null : event)}
              aria-expanded={selected?.id === event.id}
            >
              <span class="at">{when(event.tsMs)}</span>
              <span class="pri">{event.priority}</span>
              <span class="who">{event.containerLabel ?? t("falco.host", { default: "host" })}</span>
              <span class="rule">{event.rule}</span>
            </button>
            {#if selected?.id === event.id}
              <!-- Falco's own composed output line, which is what the rule
                   author intended a reader to see. -->
              <pre class="detail">{event.output}</pre>
              {#if event.tags.length > 0}
                <div class="tags">
                  {#each event.tags as tag (tag)}<span class="tag">{tag}</span>{/each}
                </div>
              {/if}

              <!-- Opt-in, and the payload is visible before it goes anywhere. -->
              <div class="ai-row">
                <button class="btn btn-sm" onclick={() => explain(event)} disabled={explaining}>
                  {explaining
                    ? t("falco.explaining", { default: "Asking…" })
                    : t("falco.explain", { default: "Explain this sequence" })}
                </button>
                <!-- Available before sending, not only after: the point of
                     showing the payload is that the user can read it first. -->
                <button
                  class="btn btn-sm"
                  onclick={() => (shownPayload ? (showPayload = !showPayload) : buildPreview(event))}
                >
                  {showPayload && shownPayload
                    ? t("falco.hide_payload", { default: "Hide what would be sent" })
                    : t("falco.show_payload", { default: "Show what would be sent" })}
                </button>
              </div>

              {#if showPayload && shownPayload}
                <p class="disclaimer">
                  {t("falco.payload_covers", { default: "Covers {n} events" }).replace(
                    "{n}",
                    String(shownPayload.count)
                  )}{shownPayload.omitted > 0
                    ? t("falco.payload_omitted", { default: ", {n} not included" }).replace(
                        "{n}",
                        String(shownPayload.omitted)
                      )
                    : ""}
                </p>
                <pre class="detail">{shownPayload.text}</pre>
              {/if}

              {#if shownExplanation}
                <div class="explanation">
                  <!-- Said plainly, because the model is not permitted to reach
                       a verdict and the reader should not infer one. -->
                  <p class="disclaimer">
                    {t("falco.ai_disclaimer", {
                      default:
                        "Written by a model from the events above. It describes and asks; it does not decide whether this is safe.",
                    })}
                  </p>
                  <pre class="detail">{shownExplanation}</pre>
                </div>
              {/if}
            {/if}
          </li>
        {/each}
      </ul>

      {#if hidden > 0}
        <!-- Counted rather than silently dropped: a filtered list that does not
             say how much it is hiding reads as a complete one. -->
        <p class="muted small">
          {t("falco.hidden_count", { default: "{n} more hidden by filters" }).replace(
            "{n}",
            String(hidden)
          )}
        </p>
      {/if}
    {/if}
  {/if}
{/if}

<style>
  .muted {
    color: var(--text-muted);
    font-size: var(--text-xs);
  }

  .small {
    margin-top: 8px;
  }

  .state {
    border: 1px solid var(--border, rgba(128, 128, 128, 0.3));
    border-radius: var(--radius-md, 8px);
    padding: 12px 14px;
    margin-bottom: 14px;
  }

  .state strong {
    display: block;
    font-size: var(--text-sm);
    margin-bottom: 4px;
  }

  .state p {
    margin: 0;
    font-size: var(--text-xs);
    color: var(--text-secondary);
    line-height: 1.5;
  }

  /* The states that mean "you are not covered" must not look neutral. */
  .state-running_without_rules,
  .state-not_falco {
    border-color: var(--warning, #d08770);
    background: color-mix(in srgb, var(--warning, #d08770) 8%, transparent);
  }

  .version {
    font-weight: normal;
    color: var(--text-muted);
    margin-left: 6px;
  }

  .controls {
    display: flex;
    gap: 12px;
    align-items: flex-end;
    flex-wrap: wrap;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .field.grow {
    flex: 1 1 180px;
  }

  .field span {
    font-size: var(--text-xs);
    color: var(--text-secondary);
  }

  .events {
    list-style: none;
    margin: 14px 0 0;
    padding: 0;
    max-height: 460px;
    overflow-y: auto;
  }

  .event {
    display: grid;
    grid-template-columns: 76px 78px 150px 1fr;
    gap: 10px;
    width: 100%;
    text-align: left;
    background: none;
    border: none;
    border-bottom: 1px solid var(--border-subtle, rgba(128, 128, 128, 0.15));
    padding: 6px 4px;
    font-size: var(--text-xs);
    color: inherit;
    cursor: pointer;
  }

  .event:hover {
    background: var(--surface-hover, rgba(128, 128, 128, 0.08));
  }

  .at {
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
  }

  .pri {
    text-transform: uppercase;
    font-size: 10px;
    letter-spacing: 0.04em;
  }

  .p-critical .pri,
  .p-alert .pri,
  .p-emergency .pri {
    color: var(--danger, #bf616a);
  }

  .p-warning .pri,
  .p-error .pri {
    color: var(--warning, #d08770);
  }

  .who {
    font-family: var(--font-mono, monospace);
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .detail {
    margin: 4px 0 10px;
    padding: 8px 10px;
    background: var(--surface-sunken, rgba(128, 128, 128, 0.08));
    border-radius: var(--radius-sm, 6px);
    font-size: var(--text-xs);
    white-space: pre-wrap;
    word-break: break-word;
  }

  .tags {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
    margin-bottom: 10px;
  }

  .tag {
    font-size: 10px;
    padding: 1px 6px;
    border-radius: 999px;
    border: 1px solid var(--border, rgba(128, 128, 128, 0.3));
    color: var(--text-secondary);
  }

  .example {
    padding: 16px;
    font-size: var(--text-xs);
    color: var(--text-secondary);
  }

  .example-label {
    color: var(--text-muted);
    margin-bottom: 6px;
  }

  .example-row {
    font-family: var(--font-mono, monospace);
    padding: 2px 0;
  }

  .ai-row {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
    margin-bottom: 10px;
  }

  .explanation {
    margin-bottom: 12px;
  }

  .disclaimer {
    margin: 0 0 6px;
    font-size: var(--text-xs);
    color: var(--text-muted);
    font-style: italic;
  }
</style>
