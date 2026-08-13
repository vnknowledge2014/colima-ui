<script lang="ts">
  /**
   * Run one image in a disposable isolated instance and watch it.
   *
   * # The banner is not decoration
   *
   * A Lima VM on macOS is a real isolation boundary and not a malware-analysis
   * one. Someone who believes otherwise may detonate a live sample on their own
   * laptop on the strength of this screen. So the limitation is a permanent
   * banner above the controls — not a tooltip, not a footnote, not a line in the
   * docs — and the wording is "observe behaviour in an isolated environment".
   * Anything implying real malware is safe here is out of bounds.
   *
   * # Why the stop button is outside the gate
   *
   * `ProGate` covers starting a session. It deliberately does not cover
   * stopping one: an entitlement that lapses mid-session must not leave a VM
   * running that the user can see and cannot reach. Same rule the scan watcher
   * follows for its own switch.
   */
  import { onDestroy, onMount } from "svelte";
  import ProGate from "../ProGate.svelte";
  import DetonationTimeline from "./DetonationTimeline.svelte";
  import { globalToast } from "../../lib/globalToast";
  import { t } from "../../lib/i18n.svelte";
  import {
    detonationApi,
    isTerminal,
    type DetonationSession,
  } from "../../lib/api/detonation";

  let { images = [] }: { images?: string[] } = $props();

  let image = $state("");
  let timeoutSecs = $state(120);
  let starting = $state(false);
  let session = $state<DetonationSession | null>(null);
  let poll: ReturnType<typeof setInterval> | null = null;

  const running = $derived(session !== null && !isTerminal(session.status));

  function stopPolling() {
    if (poll !== null) {
      clearInterval(poll);
      poll = null;
    }
  }

  /**
   * The session snapshot is authoritative and carries the whole timeline, so a
   * missed tick cannot leave the view out of date — it just refreshes later.
   */
  function startPolling(id: string) {
    stopPolling();
    poll = setInterval(async () => {
      try {
        const next = await detonationApi.get(id);
        if (!next) return;
        session = next;
        if (isTerminal(next.status)) stopPolling();
      } catch {
        // A failed poll is not a failed session. The next tick retries; the
        // backend tears the instance down on its own deadline regardless.
      }
    }, 2000);
  }

  async function start() {
    if (!image.trim()) return;
    starting = true;
    try {
      const id = await detonationApi.start({ image: image.trim(), timeoutSecs });
      session = {
        id,
        image: image.trim(),
        status: "preparing",
        startedMs: Date.now(),
        timeline: [],
      };
      startPolling(id);
    } catch (e) {
      globalToast("error", String(e));
    } finally {
      starting = false;
    }
  }

  async function cancel() {
    if (!session) return;
    try {
      await detonationApi.cancel(session.id);
    } catch (e) {
      globalToast("error", String(e));
    }
  }

  /**
   * Adopt a session that is still running from a previous mount.
   *
   * Session state lives in the backend, not in this component. Without this,
   * switching to another tab and back would leave a real VM running with no
   * Stop button anywhere in the UI — the exact "visible and unreachable" state
   * the ungated stop control exists to prevent.
   */
  onMount(async () => {
    try {
      const all = await detonationApi.list();
      // The await above can outlast a click on Run. Adopting then would replace
      // the session that click created and cancel its polling, leaving a live VM
      // with no Stop button — the failure this adoption exists to prevent.
      if (session !== null) return;

      const live = all.find((s) => !isTerminal(s.status));
      if (live) {
        session = live;
        startPolling(live.id);
      } else if (all.length > 0) {
        session = all[0];
      }
    } catch {
      // Nothing to adopt, or the backend is not reachable yet. Starting a new
      // session still works, and the backend refuses a second concurrent one.
    }
  });

  onDestroy(stopPolling);
</script>

<!-- Above the controls, always rendered, never dismissible. -->
<div class="limits" role="note">
  <strong>
    {t("detonation.limits_title", {
      default: "This observes behaviour in an isolated environment",
    })}
  </strong>
  <p>
    {t("detonation.limits_body", {
      default:
        "The image runs in a separate, disposable Colima instance with no network access and no access to your files. That is a real boundary, but it is not malware-analysis isolation — virtual machine escapes exist. Do not run samples here that you could not afford to have escape.",
    })}
  </p>
</div>

<ProGate
  variant="preview"
  feature={t("detonation.feature", { default: "Detonation" })}
  description={t("detonation.desc", {
    default:
      "Run an image in a disposable isolated instance and see the processes it spawns, the files it changes, and what it writes.",
  })}
>
  {#snippet preview()}
    <div class="example">
      <div class="example-label">{t("detonation.example", { default: "Example" })}</div>
      <div class="example-row">0s · SESSION · Creating isolated instance</div>
      <div class="example-row warn">6s · PROCESS · /bin/sh -c curl http://…</div>
      <div class="example-row">8s · FILESYSTEM · A /tmp/.x</div>
    </div>
  {/snippet}

  <div class="controls">
    <label class="field">
      <span>{t("detonation.image_label", { default: "Image" })}</span>
      <input
        list="detonation-images"
        bind:value={image}
        placeholder="alpine:latest"
        disabled={running}
      />
      <datalist id="detonation-images">
        {#each images as candidate (candidate)}
          <option value={candidate}></option>
        {/each}
      </datalist>
    </label>

    <label class="field narrow">
      <span>{t("detonation.timeout_label", { default: "Time limit (s)" })}</span>
      <input type="number" min="10" max="900" bind:value={timeoutSecs} disabled={running} />
    </label>

    <button class="btn primary" onclick={start} disabled={running || starting || !image.trim()}>
      {starting
        ? t("detonation.starting", { default: "Starting…" })
        : t("detonation.start", { default: "Run and observe" })}
    </button>
  </div>
</ProGate>

{#if session}
  <section class="session">
    <header>
      <div>
        <span class="status status-{session.status}">
          {t(`detonation.status_${session.status}`, { default: session.status })}
        </span>
        <span class="image">{session.image}</span>
        {#if session.exitCode !== null && session.exitCode !== undefined}
          <span class="exit">
            {t("detonation.exit_code", { default: "exit" })} {session.exitCode}
          </span>
        {/if}
      </div>
      <!-- Outside ProGate on purpose: see the component note. -->
      {#if running}
        <button class="btn" onclick={cancel}>
          {t("detonation.stop", { default: "Stop and destroy" })}
        </button>
      {/if}
    </header>

    {#if session.error}
      <p class="error">{session.error}</p>
    {/if}

    <DetonationTimeline events={session.timeline} startedMs={session.startedMs} />
  </section>
{/if}

<style>
  .limits {
    border: 1px solid var(--warning, #d08770);
    border-radius: var(--radius-md, 8px);
    padding: 12px 14px;
    margin-bottom: 16px;
    background: color-mix(in srgb, var(--warning, #d08770) 8%, transparent);
  }

  .limits strong {
    display: block;
    font-size: var(--text-sm);
    margin-bottom: 4px;
  }

  .limits p {
    margin: 0;
    font-size: var(--text-xs);
    color: var(--text-secondary);
    line-height: 1.5;
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
    flex: 1 1 240px;
  }

  .field.narrow {
    flex: 0 0 130px;
  }

  .field span {
    font-size: var(--text-xs);
    color: var(--text-secondary);
  }

  .session {
    margin-top: 18px;
  }

  .session header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 12px;
    margin-bottom: 10px;
    flex-wrap: wrap;
  }

  .status {
    text-transform: uppercase;
    font-size: 10px;
    letter-spacing: 0.05em;
    padding: 2px 7px;
    border-radius: 999px;
    border: 1px solid var(--border, rgba(128, 128, 128, 0.3));
  }

  .status-running,
  .status-preparing {
    color: var(--accent, #5e81ac);
    border-color: currentColor;
  }

  .status-failed,
  .status-timed_out {
    color: var(--danger, #bf616a);
    border-color: currentColor;
  }

  .image {
    font-family: var(--font-mono, monospace);
    font-size: var(--text-xs);
    margin-left: 8px;
  }

  .exit {
    font-size: var(--text-xs);
    color: var(--text-muted);
    margin-left: 8px;
  }

  .error {
    color: var(--danger, #bf616a);
    font-size: var(--text-xs);
    margin: 0 0 10px;
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

  .example-row.warn {
    color: var(--warning, #d08770);
  }
</style>
