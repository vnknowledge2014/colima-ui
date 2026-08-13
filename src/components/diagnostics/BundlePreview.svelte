<script lang="ts">
  /**
   * "Report a problem": collect diagnostics, show the user every byte, then let
   * them decide.
   *
   * The preview is the feature, not a formality. Content is redacted by the
   * backend at construction, but redaction is a machine's judgement about what
   * looks like a secret — the person whose machine it is gets the last word, so
   * each section is expanded in full and can be unchecked.
   *
   * Nothing here transmits. Copy puts text on the clipboard, Save writes a file,
   * and "Open GitHub" opens a prefilled issue form in the browser. All three are
   * the user's action.
   */
  import { onMount } from "svelte";
  import {
    diagnosticsApi,
    renderBundleMarkdown,
    type DiagnosticBundle,
  } from "../../lib/api/diagnostics";
  import { newIssueUrl, openExternal } from "../../lib/external-links";
  import { isRunningInTauri } from "../../lib/env";
  import { globalToast } from "../../lib/globalToast";
  import { t } from "../../lib/i18n.svelte";
  import * as Icons from "../Icons.svelte";

  interface Props {
    /** The error the user is reporting, when they opened this from one. */
    error?: string;
    /** Container whose logs are worth attaching, if the report is about one. */
    containerId?: string;
    onClose: () => void;
  }

  let { error = "", containerId = "", onClose }: Props = $props();

  const isTauri = isRunningInTauri();

  let bundle = $state<DiagnosticBundle | null>(null);
  let loading = $state(true);
  let failure = $state("");
  let included = $state<Set<string>>(new Set());
  let expanded = $state<Set<string>>(new Set());
  let saving = $state(false);
  let savedPath = $state("");

  onMount(async () => {
    try {
      const result = await diagnosticsApi.bundle(error || undefined, containerId || undefined);
      bundle = result;
      included = new Set(result.sections.filter((s) => s.includedByDefault).map((s) => s.id));
    } catch (e) {
      failure = String(e);
    } finally {
      loading = false;
    }
  });

  function toggle(id: string) {
    const next = new Set(included);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    included = next;
  }

  function toggleExpanded(id: string) {
    const next = new Set(expanded);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    expanded = next;
  }

  const markdown = $derived(bundle ? renderBundleMarkdown(bundle, [...included]) : "");

  async function copy() {
    try {
      await navigator.clipboard.writeText(markdown);
      globalToast("success", t("diagnostics.copied", { default: "Copied to clipboard" }));
    } catch (e) {
      globalToast("error", String(e));
    }
  }

  async function save() {
    if (!bundle) return;
    saving = true;
    try {
      let destDir = "";
      if (isTauri) {
        const dialog = await import("@tauri-apps/plugin-dialog");
        const chosen = await dialog.open({ directory: true, multiple: false });
        if (typeof chosen !== "string") return;
        destDir = chosen;
      } else {
        // Browser mode has no picker; the backend still confines the write to
        // whatever folder is named here.
        const typed = window.prompt(
          t("diagnostics.dest_prompt", { default: "Folder to save the report in" })
        );
        if (!typed) return;
        destDir = typed;
      }
      // Seconds in the name: filing two reports a minute apart should not have
      // the second silently refuse because the first took the name.
      const stamp = new Date().toISOString().replace(/[:.]/g, "-").slice(0, 19);
      savedPath = await diagnosticsApi.save(
        bundle,
        [...included],
        destDir,
        `colimaui-diagnostics-${stamp}.md`
      );
      globalToast("success", savedPath);
    } catch (e) {
      globalToast("error", String(e));
    } finally {
      saving = false;
    }
  }

  async function openIssue() {
    const title = bundle?.signature
      ? `[bug] ${bundle.signature.slice(0, 120)}`
      : "[bug] ";
    const body = [
      "### What happened",
      "",
      "",
      "### Diagnostics",
      "",
      t("diagnostics.paste_hint", {
        default: "Paste the report you copied from the app here, or attach the saved file.",
      }),
      "",
    ].join("\n");
    await openExternal(newIssueUrl(title, body));
  }
</script>

<div class="modal-overlay" role="presentation" onclick={onClose}>
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div class="modal wide" role="dialog" aria-modal="true" tabindex="-1" onclick={(e) => e.stopPropagation()}>
    <div class="modal-header">
      <span class="modal-title">{t("diagnostics.title", { default: "Report a problem" })}</span>
    </div>

    <div class="body">
      <p class="lead">
        {t("diagnostics.lead", {
          default:
            "Nothing is sent automatically. Review everything below, uncheck anything you would rather not share, then copy or save it.",
        })}
      </p>

      {#if loading}
        <p class="hint">{t("diagnostics.collecting", { default: "Collecting…" })}</p>
      {:else if failure}
        <p class="failure">{failure}</p>
      {:else if bundle}
        {#if bundle.signature}
          <p class="hint">
            {t("diagnostics.signature", { default: "Signature" })}:
            <code>{bundle.signature}</code>
          </p>
        {/if}
        {#if bundle.truncatedBytes > 0}
          <p class="hint">
            {t("diagnostics.truncated", {
              default: "{bytes} bytes of older log lines were trimmed.",
              bytes: bundle.truncatedBytes,
            })}
          </p>
        {/if}

        <ul class="sections">
          {#each bundle.sections as section (section.id)}
            <li>
              <div class="row">
                <label>
                  <input
                    type="checkbox"
                    checked={included.has(section.id)}
                    onchange={() => toggle(section.id)}
                  />
                  <span>{section.title}</span>
                </label>
                <button class="btn btn-ghost" onclick={() => toggleExpanded(section.id)}>
                  {expanded.has(section.id)
                    ? t("diagnostics.hide", { default: "Hide" })
                    : t("diagnostics.show", { default: "Show" })}
                </button>
              </div>
              {#if expanded.has(section.id)}
                <!-- Full content, not a sample: a preview that elides is not a
                     preview the user can make a decision from. -->
                <pre>{section.content}</pre>
              {/if}
            </li>
          {/each}
        </ul>
      {/if}
    </div>

    <div class="modal-footer">
      <button class="btn btn-ghost" onclick={onClose}>
        {t("common.close", { default: "Close" })}
      </button>
      <button class="btn btn-ghost" disabled={!bundle || saving} onclick={save}>
        {saving
          ? t("diagnostics.saving", { default: "Saving…" })
          : t("diagnostics.save", { default: "Save .md" })}
      </button>
      <button class="btn btn-ghost" disabled={!bundle} onclick={openIssue}>
        {t("diagnostics.open_issue", { default: "Open GitHub issue" })}
      </button>
      <button class="btn btn-primary" disabled={!bundle} onclick={copy}>
        {@html Icons.Check}
        {t("diagnostics.copy", { default: "Copy report" })}
      </button>
    </div>
  </div>
</div>

<style>
  .modal.wide {
    width: min(720px, 92vw);
  }

  .body {
    padding: 16px;
    display: flex;
    flex-direction: column;
    gap: 12px;
    max-height: 60vh;
    overflow-y: auto;
  }

  .lead {
    margin: 0;
    font-size: var(--text-xs);
    color: var(--text-secondary);
    line-height: 1.5;
  }

  .hint {
    margin: 0;
    font-size: var(--text-xs);
    color: var(--text-muted);
    word-break: break-all;
  }

  .failure {
    margin: 0;
    font-size: var(--text-xs);
    color: var(--color-danger, #ef4444);
  }

  .sections {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }

  .row label {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: var(--text-xs);
    color: var(--text-secondary);
    cursor: pointer;
  }

  pre {
    margin: 4px 0 0;
    padding: 8px;
    background: var(--bg-primary);
    border: 1px solid var(--border-primary);
    border-radius: 6px;
    font-size: 11px;
    max-height: 240px;
    overflow: auto;
    white-space: pre-wrap;
    word-break: break-word;
    color: var(--text-secondary);
  }

  code {
    font-size: 11px;
  }
</style>
