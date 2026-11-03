<script lang="ts">
  // Environment page: renders the resolution record from commands/resolution.rs
  // (env duality table + path provenance rows). Debug surface, deliberately
  // read-only — display what the app resolved and the rule that picked it.
  // Presentation rules:
  //  - shell vs app env diverging is EXPECTED (launchd is minimal by design),
  //    so it is never styled as an error
  //  - what matters is which value the app picked → the resolution record
  //  - paths are long: tables are fixed-width with wrapping cells
  import { onMount } from "svelte";
  import { t } from "../lib/i18n.svelte";
  import { environmentApi, type EnvironmentReport } from "../lib/api";

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
  <div class="content-header-actions">
    <button type="button" class="btn btn-ghost" onclick={refresh} disabled={loading}>
      ↻ {t("environment.refresh", { default: "Re-scan" })}
    </button>
  </div>
</div>

<div class="content-body">
  <p class="env-lead">
    {t("environment.subtitle", {
      default:
        "What this app believes about your setup, and the rule that made it believe that.",
    })}
  </p>

  {#if loading && !report}
    <p class="env-status">{t("environment.loading", { default: "Scanning…" })}</p>
  {:else if error}
    <p class="env-status" style="color: var(--accent-red);" role="alert">{error}</p>
  {:else if report}
    {#if report.shellSnapshotError}
      <p class="env-status" style="color: var(--accent-yellow);">
        ⓘ {t("environment.snapshotError", { default: "Shell env snapshot unavailable" })}: {report.shellSnapshotError}
      </p>
    {/if}

    <div class="env-section">
      <h2 id="env-duality-heading" class="env-heading">
        {t("environment.dualityTitle", { default: "Environment: Shell env vs This app" })}
      </h2>
      <p class="env-subtitle">
        {t("environment.dualityExplain", {
          default:
            "A GUI process inherits launchd's env, not your shell's (snapshot: one non-login shell startup via $SHELL — for zsh that's .zshenv; bash: $BASH_ENV/.bashrc; etc.). The two columns will legitimately differ — what matters is the resolution record below.",
        })}
      </p>
      <div tabindex="0" role="region" aria-labelledby="env-duality-heading" class="card env-table-wrap">
        <table class="data-table env-table">
          <colgroup>
            <col style="width: 20%" />
            <col style="width: 40%" />
            <col style="width: 40%" />
          </colgroup>
          <thead>
            <tr>
              <th scope="col">{t("environment.variable", { default: "Variable" })}</th>
              <th scope="col">{t("environment.shellEnv", { default: "Shell env" })}</th>
              <th scope="col">{t("environment.appEnv", { default: "This app" })}</th>
            </tr>
          </thead>
          <tbody>
            {#each report.envDuality as v (v.key)}
              <tr title={v.diverges ? t("environment.diverges", { default: "differs between Shell env (.zshenv) and This app" }) : undefined}>
                <th scope="row" class="mono row-key">{v.key}</th>
                <td class="mono" style={v.shell == null ? "opacity: 0.55;" : ""}>{v.shell ?? "—"}</td>
                <td class="mono" style={v.runtime == null ? "opacity: 0.55;" : ""}>{v.runtime ?? "—"}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    </div>

    <div class="env-section">
      <h2 id="env-resolution-heading" class="env-heading">
        {t("environment.resolutionTitle", { default: "Resolution record" })}
      </h2>
      <p class="env-subtitle">
        {t("environment.resolutionExplain", {
          default:
            "The values this app is actually using, and the rule that picked each one. Red means a configured path no longer exists.",
        })}
      </p>
      <div tabindex="0" role="region" aria-labelledby="env-resolution-heading" class="card env-table-wrap">
        <table class="data-table env-table">
          <colgroup>
            <col style="width: 18%" />
            <col style="width: 14%" />
            <col style="width: 34%" />
            <col style="width: 34%" />
          </colgroup>
          <thead>
            <tr>
              <th scope="col">{t("environment.what", { default: "What" })}</th>
              <th scope="col">{t("environment.rule", { default: "Rule" })}</th>
              <th scope="col">{t("environment.resolvedTo", { default: "Resolved to" })}</th>
              <th scope="col">{t("environment.detail", { default: "Detail" })}</th>
            </tr>
          </thead>
          <tbody>
            {#each report.rows as row (row.key)}
              <tr>
                <th scope="row" class="mono row-key">{row.key}</th>
                <td>{row.rule}</td>
                <td class="mono" style={healthStyle(row.health)}>
                  {row.chosen}
                  {#if row.health}
                    <span style="opacity: 0.65;">({row.health})</span>
                  {/if}
                </td>
                <td>{row.detail ?? ""}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    </div>
  {/if}
</div>

<style>
  .env-lead {
    font-size: var(--text-sm);
    color: var(--text-secondary);
    margin: 0 0 20px;
    line-height: 1.5;
  }

  .env-status {
    font-size: var(--text-sm);
    color: var(--text-secondary);
    margin: 0 0 16px;
  }

  .env-section {
    margin-bottom: 24px;
  }

  .env-heading {
    font-size: var(--text-base);
    font-weight: 600;
    color: var(--text-primary);
    margin: 0 0 6px;
  }

  .env-subtitle {
    font-size: var(--text-xs);
    color: var(--text-secondary);
    margin: 0 0 12px;
    line-height: 1.55;
  }

  .env-table-wrap {
    padding: 0;
    overflow: auto;
    max-inline-size: 100%;
  }

  .env-table {
    table-layout: fixed;
    width: 100%;
  }

  .env-table th,
  .env-table td {
    text-align: left;
    vertical-align: top;
    white-space: normal !important;
    overflow-wrap: anywhere;
    word-break: break-word;
  }

  .env-table tbody th.row-key {
    padding: 11px 16px;
    font-size: var(--text-xs);
    font-weight: 600;
    color: var(--text-primary);
    text-transform: none;
    letter-spacing: normal;
    background: transparent;
    border-bottom: 1px solid var(--border-subtle);
  }

  .env-table tbody tr:last-child th.row-key,
  .env-table tbody tr:last-child td {
    border-bottom: none;
  }

  .mono {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    word-break: break-all;
  }
</style>
