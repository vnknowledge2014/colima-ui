<script lang="ts">
  import { onMount } from "svelte";
  import { Terminal as XTerm } from "@xterm/xterm";
  import { FitAddon } from "@xterm/addon-fit";
  import { WebLinksAddon } from "@xterm/addon-web-links";
  import { t } from "../lib/i18n.svelte";

  let { 
    sessionId, 
    profile, 
    vmType = "colima", 
    active, 
    termTheme, 
    authHeaders, 
    API_BASE 
  } = $props<{
    sessionId: string;
    profile: string;
    vmType?: "colima" | "lima";
    active: boolean;
    termTheme: any;
    authHeaders: () => Promise<Record<string, string>>;
    API_BASE: string;
  }>();

  let termRef = $state<HTMLDivElement | null>(null);
  let xterm: XTerm | null = null;
  let fit: FitAddon | null = null;
  let pollingId: ReturnType<typeof setInterval> | null = null;
  let mounted = true;
  let connected = $state(false);
  let error = $state<string | null>(null);
  
  // We need to keep track of this specific session instance
  let actualSessionId = "";

  onMount(() => {
    actualSessionId = `${sessionId}-${Date.now()}`;
    mounted = true;
    if (!termRef) return;

    xterm = new XTerm({
      cursorBlink: true,
      fontFamily: "'JetBrains Mono', 'Fira Code', monospace",
      fontSize: 13,
      lineHeight: 1.4,
      theme: termTheme,
    });

    fit = new FitAddon();
    xterm.loadAddon(fit);
    xterm.loadAddon(new WebLinksAddon());

    xterm.open(termRef);
    fit.fit();

    const resizeObserver = new ResizeObserver(() => {
      if (fit) {
        try { fit.fit(); } catch (_) { /* ignore */ }
      }
    });
    resizeObserver.observe(termRef);

    const connect = async () => {
      xterm!.writeln(`\x1b[36m● ${t('terminal.connecting_to', { default: 'Connecting to ' })} ${profile} (${t('terminal.browser_mode', { default: 'browser mode' })})...\x1b[0m\r\n`);

      try {
        const hdrs = await authHeaders();
        await fetch(`${API_BASE}/api/terminal/close`, {
          method: "POST",
          headers: hdrs,
          body: JSON.stringify({ session_id: actualSessionId }),
        }).catch(() => {});

        const res = await fetch(`${API_BASE}/api/terminal/create`, {
          method: "POST",
          headers: hdrs,
          body: JSON.stringify({ session_id: actualSessionId, profile, vm_type: vmType }),
        });
        const data = await res.json();

        if (!mounted) return;

        if (!data.success) {
          throw new Error(data.error || "Failed to create session");
        }

        connected = true;

        xterm!.onData(async (input) => {
          try {
            const h = await authHeaders();
            await fetch(`${API_BASE}/api/terminal/write`, {
              method: "POST",
              headers: h,
              body: JSON.stringify({ session_id: actualSessionId, data: input }),
            });
          } catch (_) { /* ignore write errors */ }
        });

        pollingId = setInterval(async () => {
          if (!mounted) return;
          try {
            const h = await authHeaders();
            const r = await fetch(`${API_BASE}/api/terminal/read?session_id=${encodeURIComponent(actualSessionId)}`, { headers: h });
            const d = await r.json();
            if (d.success && d.data) {
              const normalized = d.data.replace(/\r?\n/g, "\r\n");
              xterm!.write(normalized);
            }
          } catch (_) { /* ignore read errors */ }
        }, 100);

      } catch (e) {
        if (!mounted) return;
        xterm!.writeln(`\r\n\x1b[31m● ${t('terminal.failed_to_connect', { default: 'Failed to connect: ' })} ${e}\x1b[0m`);
        xterm!.writeln(`\x1b[33m  ${t('terminal.ensure_running', { default: 'Make sure the instance is running.' })}\x1b[0m`);
        error = String(e);
      }
    };

    connect();

    return () => {
      mounted = false;
      resizeObserver.disconnect();
      if (pollingId) {
        clearInterval(pollingId);
        pollingId = null;
      }
      authHeaders().then((h: any) => {
        fetch(`${API_BASE}/api/terminal/close`, {
          method: "POST",
          headers: h,
          body: JSON.stringify({ session_id: actualSessionId }),
        }).catch(() => {});
      });
      if (xterm) {
        xterm.dispose();
        xterm = null;
      }
    };
  });

  $effect(() => {
    if (active && fit) {
      setTimeout(() => {
        try { fit?.fit(); } catch (_) { /* ignore */ }
      }, 100);
    }
  });
</script>

<div style="position: relative; height: 100%; display: {active ? 'block' : 'none'};">
  {#if error && !connected}
    <div style="position: absolute; top: 12px; right: 12px; z-index: 10; padding: 6px 12px; border-radius: var(--radius-md); background: rgba(248, 81, 73, 0.15); border: 1px solid var(--accent-red); color: var(--accent-red); font-size: var(--text-xs);">
      {t('terminal.connection_failed', { default: 'Connection failed' })}
    </div>
  {/if}
  <div bind:this={termRef} style="height: 100%; padding: 4px;"></div>
</div>
