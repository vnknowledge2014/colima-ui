<script lang="ts">
  /**
   * The vulnerabilities a scan found, filtered by severity.
   *
   * Sorted by severity, then by whether a fix exists: a critical nobody has
   * patched yet and a critical with a one-line upgrade are the same number and
   * completely different work, so the fixed version is a column rather than a
   * detail behind a click.
   */
  import type { Finding, Severity } from "../../lib/api/security";
  import { t } from "../../lib/i18n.svelte";

  interface Props {
    findings: Finding[];
  }

  let { findings }: Props = $props();

  const ORDER: Severity[] = ["critical", "high", "medium", "low", "unknown"];

  /** Translated once here rather than at four call sites. */
  function severityLabel(sev: Severity): string {
    return t(`security.severity_${sev}`, { default: sev });
  }

  /** A severity the backend has not normalised must not become NaN or sort first. */
  function rank(sev: Severity): number {
    const at = ORDER.indexOf(sev);
    return at === -1 ? ORDER.length : at;
  }

  let minSeverity = $state<Severity>("low");

  const counts = $derived.by(() => {
    const c: Record<Severity, number> = { critical: 0, high: 0, medium: 0, low: 0, unknown: 0 };
    for (const f of findings) c[f.severity] = (c[f.severity] ?? 0) + 1;
    return c;
  });

  const shown = $derived.by(() => {
    const cutoff = rank(minSeverity);
    return findings
      .filter((f) => rank(f.severity) <= cutoff)
      .slice()
      .sort((a, b) => {
        const bySeverity = rank(a.severity) - rank(b.severity);
        if (bySeverity !== 0) return bySeverity;
        // Fixable first: it is the half of the list somebody can act on today.
        const aFix = a.fixedVersion ? 0 : 1;
        const bFix = b.fixedVersion ? 0 : 1;
        return aFix - bFix || a.id.localeCompare(b.id);
      });
  });

  const fixable = $derived(findings.filter((f) => f.fixedVersion).length);
</script>

<div class="findings">
  <div class="toolbar">
    <div class="counts">
      {#each ORDER as sev (sev)}
        {#if counts[sev] > 0}
          <span class="chip sev-{sev}">{counts[sev]} {severityLabel(sev)}</span>
        {/if}
      {/each}
      {#if findings.length > 0}
        <span class="chip">
          {t("security.fixable", {
            default: "{count} with a published fix",
            count: fixable,
          })}
        </span>
      {/if}
    </div>

    <label class="filter">
      {t("security.min_severity", { default: "Show down to" })}
      <select class="input select" bind:value={minSeverity}>
        {#each ORDER as sev (sev)}
          <option value={sev}>{severityLabel(sev)}</option>
        {/each}
      </select>
    </label>
  </div>

  {#if findings.length === 0}
    <p class="none">
      {t("security.no_findings", {
        default:
          "No known vulnerabilities in this image, for the database date shown above.",
      })}
    </p>
  {:else if shown.length === 0}
    <p class="none">{t("security.none_at_severity", { default: "Nothing at this severity." })}</p>
  {:else}
    <div class="table-wrap">
      <table>
        <thead>
          <tr>
            <th>{t("security.col_severity", { default: "Severity" })}</th>
            <th>{t("security.col_id", { default: "Advisory" })}</th>
            <th>{t("security.col_package", { default: "Package" })}</th>
            <th>{t("security.col_installed", { default: "Installed" })}</th>
            <th>{t("security.col_fixed", { default: "Fixed in" })}</th>
          </tr>
        </thead>
        <tbody>
          {#each shown as f (f.id + f.package + f.installedVersion)}
            <tr>
              <td><span class="chip sev-{f.severity}">{severityLabel(f.severity)}</span></td>
              <td class="mono">{f.id}{#if f.cvss}<span class="cvss">CVSS {f.cvss}</span>{/if}</td>
              <td class="mono">{f.package}</td>
              <td class="mono">{f.installedVersion}</td>
              <td class="mono">
                {#if f.fixedVersion}
                  {f.fixedVersion}
                {:else}
                  <span class="no-fix">{t("security.no_fix", { default: "no fix yet" })}</span>
                {/if}
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
</div>

<style>
  .findings {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .toolbar {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 12px;
    flex-wrap: wrap;
  }
  .counts {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
  }
  .chip {
    font-size: var(--text-xs);
    padding: 2px 8px;
    border-radius: 10px;
    background: var(--bg-secondary);
    border: 1px solid var(--border-primary);
    color: var(--text-secondary);
    white-space: nowrap;
  }
  .sev-critical {
    color: var(--sev-critical);
    border-color: var(--sev-critical);
  }
  .sev-high {
    color: var(--sev-high);
    border-color: var(--sev-high);
  }
  .sev-medium {
    color: var(--sev-medium);
  }
  .filter {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: var(--text-sm);
    color: var(--text-secondary);
  }
  /* Keep the shared input padding so this sits at the same height as every
     other control; only the full-width default needs undoing inline. */
  .filter :global(select) {
    width: auto;
  }
  .table-wrap {
    overflow-x: auto;
  }
  table {
    width: 100%;
    border-collapse: collapse;
    font-size: var(--text-sm);
  }
  th {
    text-align: left;
    font-weight: 500;
    color: var(--text-muted);
    font-size: var(--text-xs);
    padding: 6px 10px;
    border-bottom: 1px solid var(--border-primary);
    white-space: nowrap;
  }
  td {
    padding: 6px 10px;
    border-bottom: 1px solid var(--border-subtle, #30363d);
    vertical-align: top;
  }
  .mono {
    font-family: var(--font-mono, ui-monospace, monospace);
    word-break: break-all;
  }
  .cvss {
    margin-left: 8px;
    font-size: var(--text-xs);
    color: var(--text-muted);
  }
  .no-fix {
    color: var(--text-muted);
  }
  .none {
    margin: 0;
    font-size: var(--text-sm);
    color: var(--text-secondary);
  }
</style>
