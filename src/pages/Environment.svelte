<script lang="ts">
  // Environment page: renders the resolution record from commands/resolution.rs
  // (env duality table + path provenance rows). Debug surface, deliberately
  // read-only — see design: display what it resolved, pin what it must.
  // Presentation rules:
  //  - shell vs app env diverging is EXPECTED (launchd is minimal by design),
  //    so it is never styled as an error
  //  - what matters is which value the app picked → the resolution record
  //  - paths are long: tables are fixed-width with wrapping cells
  import { onMount } from "svelte";
  import { t } from "../lib/i18n.svelte";
  import { environmentApi } from "../lib/api";
  import type { EnvironmentReport } from "../lib/api";

  let report: EnvironmentReport | null = $state(null);
  let loading = $state(false);
  let error = $state<string | null>(null);

  const healthStyle = (health: string | null) =>
    health === "missing" || health === "unreachable" ? "color: var(--accent-red);" : "";

  async function refresh() {
    loading = true;
    error = null;
    try {
      report = await environmentApi.getResolutions();
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  }

  onMount(() => {
    refresh();
  });
</script>

<div class="content-header" data-tauri-drag-region>
  <h1>{t("environment.title", { default: "Environment" })}</h1>
  <p class="page-subtitle">
    {t("environment.subtitle", {
      default:
        "What this app believes about your setup, and the rule that made it believe that.",
    })}
  </p>
  <div class="content-header-actions">
    <button class="btn btn-ghost" onclick={refresh} disabled={loading}>
      ↻ {t("environment.refresh", { default: "Re-scan" })}
    </button>
  </div>
</div>

<div class="content-body">
  {#if loading && !report}
    <p>{t("environment.loading", { default: "Scanning…" })}</p>
  {:else if error}
    <p style="color: var(--accent-red);">{error}</p>
  {:else if report}
    {#if report.shellSnapshotError}
      <p style="color: var(--accent-yellow)">
        ⓘ {t("environment.snapshotError", { default: "Shell env snapshot unavailable" })}: {report.shellSnapshotError}
      </p>
    {/if}

    <h3>{t("environment.dualityTitle", { default: "Environment: Shell env vs This app" })}</h3>
    <p class="page-subtitle">
      {t("environment.dualityExplain", {
        default:
          "A GUI process inherits launchd's env, not your shell's (snapshot: one non-login shell startup via $SHELL — for zsh that's .zshenv; bash: $BASH_ENV/.bashrc; etc.). The two columns will legitimately differ — what matters is the resolution record below.",
      })}
    </p>
    <table class="env-table">
      <colgroup><col style="width: 20%" /><col style="width: 40%" /><col style="width: 40%" /></colgroup>
      <thead>
        <tr>
          <th>{t("environment.variable", { default: "Variable" })}</th>
          <th>{t("environment.shellEnv", { default: "Shell env" })}</th>
          <th>{t("environment.appEnv", { default: "This app" })}</th>
        </tr>
      </thead>
      <tbody>
        {#each report.envDuality as v (v.key)}
          <tr>
            <td class="mono">{v.key}</td>
            <td class="mono" style={v.shell == null ? "opacity: 0.55;" : ""}>{v.shell ?? "—"}</td>
            <td class="mono" style={v.runtime == null ? "opacity: 0.55;" : ""}>{v.runtime ?? "—"}</td>
          </tr>
        {/each}
      </tbody>
    </table>

    <h3>{t("environment.resolutionTitle", { default: "Resolution record" })}</h3>
    <p class="page-subtitle">
      {t("environment.resolutionExplain", {
        default:
          "The values this app is actually using, and the rule that picked each one. Red means a configured path no longer exists.",
      })}
    </p>
    <table class="env-table">
      <colgroup><col style="width: 16%" /><col style="width: 12%" /><col style="width: 36%" /><col style="width: 36%" /></colgroup>
      <thead>
        <tr>
          <th>{t("environment.what", { default: "What" })}</th>
          <th>{t("environment.rule", { default: "Rule" })}</th>
          <th>{t("environment.resolvedTo", { default: "Resolved to" })}</th>
          <th>{t("environment.detail", { default: "Detail" })}</th>
        </tr>
      </thead>
      <tbody>
        {#each report.rows as row (row.key)}
          <tr>
            <td class="mono">{row.key}</td>
            <td>{row.rule}</td>
            <td class="mono" style={healthStyle(row.health)}>
              {row.chosen}
              {#if row.health}
                <span style="opacity: 0.65;">({row.health})</span>
              {/if}
            </td>
            <td class="wrap">{row.detail ?? ""}</td>
          </tr>
        {/each}
      </tbody>
    </table>
  {/if}
</div>

<style>
  .env-table {
    table-layout: fixed;
    width: 100%;
  }
  .env-table th,
  .env-table td {
    text-align: left;
    vertical-align: top;
    white-space: normal !important; /* defeat inherited nowrap from global table rules */
    overflow-wrap: anywhere;
    word-break: break-word;
  }
  .mono {
    font-family: var(--font-mono, "SF Mono", Menlo, monospace);
    font-size: var(--text-sm, 12px);
    word-break: break-all; /* PATH-ish values have no spaces: break anywhere */
  }
</style>