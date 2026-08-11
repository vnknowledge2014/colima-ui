<script lang="ts">
  import { onMount } from "svelte";
  import { Terminal as XTerm } from "@xterm/xterm";
  // Required, not cosmetic. xterm.js positions its row divs, hides the
  // character-measuring element, and sizes the cursor entirely from this
  // stylesheet. Without it the measure element renders as visible garbage and
  // the cursor draws as an oversized block.
  import "@xterm/xterm/css/xterm.css";
  import { FitAddon } from "@xterm/addon-fit";
  import { WebLinksAddon } from "@xterm/addon-web-links";
  import { t } from "../lib/i18n.svelte";
  import {
    exitHint,
    openTerminal,
    type SessionKind,
    type TerminalHandle,
  } from "../lib/terminal-transport";

  let { sessionId, kind, active, termTheme } = $props<{
    sessionId: string;
    kind: SessionKind;
    active: boolean;
    termTheme: any;
  }>();

  let termRef = $state<HTMLDivElement | null>(null);
  let xterm: XTerm | null = null;
  let fit: FitAddon | null = null;
  let handle: TerminalHandle | null = null;
  let exitPollId: ReturnType<typeof setInterval> | null = null;
  let mounted = true;
  let connected = $state(false);
  let error = $state<string | null>(null);

  /** Last size pushed to the pty, so identical resizes are not re-sent. */
  let lastRows = 0;
  let lastCols = 0;
  let resizeTimer: ReturnType<typeof setTimeout> | null = null;

  /**
   * Tell the pty its new grid.
   *
   * Debounced because a drag fires continuously, and guarded against 0 —
   * `fit()` reports zero while the element is hidden, and a 0-row pty makes
   * some shells abort.
   */
  function pushResize() {
    if (!xterm || !handle) return;
    const { rows, cols } = xterm;
    if (!rows || !cols) return;
    if (rows === lastRows && cols === lastCols) return;

    lastRows = rows;
    lastCols = cols;
    if (resizeTimer) clearTimeout(resizeTimer);
    resizeTimer = setTimeout(() => {
      handle?.resize(rows, cols).catch(() => {});
    }, 50);
  }

  function refit() {
    if (!fit || !active) return;
    try {
      fit.fit();
      pushResize();
    } catch {
      /* element not laid out yet */
    }
  }

  onMount(() => {
    mounted = true;
    if (!termRef) return;

    xterm = new XTerm({
      cursorBlink: true,
      fontFamily: "'JetBrains Mono', 'Fira Code', monospace",
      fontSize: 13,
      lineHeight: 1.4,
      // Bounded: an unbounded scrollback plus a chatty build log is a slow leak.
      scrollback: 5000,
      theme: termTheme,
    });

    fit = new FitAddon();
    xterm.loadAddon(fit);
    xterm.loadAddon(new WebLinksAddon());
    xterm.open(termRef);
    fit.fit();

    const resizeObserver = new ResizeObserver(() => refit());
    resizeObserver.observe(termRef);

    const connect = async () => {
      // No "connecting" banner on the happy path — the shell prompt is the
      // signal that the session is live, and a banner just pushes it down.
      // Failures still announce themselves below.
      try {
        // The session id is the tab's, with no timestamp mixed in. It used to
        // carry `Date.now()`, so every remount opened a *new* pty and abandoned
        // the old one; keying it to the tab means a remount reattaches.
        handle = await openTerminal(sessionId, kind, (text) => {
          // Written through untouched. The old code rewrote `\n` as `\r\n` to
          // compensate for the `script(1)` wrapper, which corrupts any program
          // that positions the cursor itself.
          xterm?.write(text);
        });

        if (!mounted) {
          await handle.close();
          return;
        }

        connected = true;
        // Send the real grid now that the pty exists — it starts at 80x24.
        lastRows = 0;
        lastCols = 0;
        refit();

        xterm!.onData((input) => {
          handle?.write(input).catch(() => {});
        });

        // Output is pushed, so nothing polls for it. This only asks whether the
        // shell has died, so the UI stops showing a live prompt for a dead pty.
        exitPollId = setInterval(async () => {
          if (!mounted || !handle) return;
          try {
            const code = await handle.pollExit();
            if (code !== null && code !== undefined) {
              xterm?.writeln(
                `\r\n\x1b[33m● ${t("terminal.session_ended", { default: "Session ended (exit code: {code})", code })}\x1b[0m`,
              );
              // Some exit codes are meaningless on their own — a shell-less
              // image just reports 127 and an OCI error. Say what happened.
              const hint = exitHint(kind, code);
              if (hint) xterm?.writeln(`\x1b[36m  ${hint}\x1b[0m`);
              connected = false;
              if (exitPollId) clearInterval(exitPollId);
              exitPollId = null;
            }
          } catch {
            /* session already gone */
          }
        }, 1000);
      } catch (e) {
        if (!mounted) return;
        xterm!.writeln(
          `\r\n\x1b[31m● ${t("terminal.failed_to_connect", { default: "Failed to connect: {error}", error: String(e) })}\x1b[0m`,
        );
        xterm!.writeln(
          `\x1b[33m  ${t("terminal.ensure_running", { default: "Make sure the instance is running." })}\x1b[0m`,
        );
        error = String(e);
      }
    };

    connect();

    return () => {
      mounted = false;
      resizeObserver.disconnect();
      if (exitPollId) clearInterval(exitPollId);
      if (resizeTimer) clearTimeout(resizeTimer);
      handle?.close().catch(() => {});
      handle = null;
      xterm?.dispose();
      xterm = null;
    };
  });

  // Refit when the tab becomes visible: it was measured at zero while hidden.
  $effect(() => {
    if (active) setTimeout(refit, 50);
  });
</script>

<div style="position: relative; height: 100%; display: {active ? 'block' : 'none'};">
  {#if error && !connected}
    <div class="terminal-error-badge">
      {t("terminal.connection_failed", { default: "Connection failed" })}
    </div>
  {/if}
  <div bind:this={termRef} style="height: 100%; padding: 4px;"></div>
</div>

<style>
  .terminal-error-badge {
    position: absolute;
    top: 12px;
    right: 12px;
    z-index: 10;
    padding: 6px 12px;
    border-radius: var(--radius-md);
    background: rgba(248, 81, 73, 0.15);
    border: 1px solid var(--accent-red);
    color: var(--accent-red);
    font-size: var(--text-xs);
  }
</style>
