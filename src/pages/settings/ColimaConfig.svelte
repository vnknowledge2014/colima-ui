<script lang="ts">
  import { onMount, untrack } from "svelte";
  import {
    colimaConfigApi,
    systemApi,
    RUNTIMES,
    VM_TYPES,
    MOUNT_TYPES,
    type ConfigChanges,
    type ApplyResult,
    type ValidationIssue,
    type HostSpecs,
  } from "../../lib/api";
  import { t } from "../../lib/i18n.svelte";
  import { globalToast } from "../../lib/globalToast";
  import { reportError } from "../../lib/errorReporter";
  import { normalizeError, errorMessage } from "../../lib/errors";
  import { dashboardState, uiState } from "../../store.svelte";
  import DiffView from "../../components/DiffView.svelte";

  /**
   * Editor for `~/.colima/<profile>/colima.yaml`.
   *
   * The file is the single source of truth for a profile's resources —
   * `start_instance` only passes CLI flags when no config exists yet — so what
   * is saved here is what the VM boots with on its next start.
   *
   * The form only ever sends the fields the user actually changed. Sending the
   * whole form back would make an untouched control overwrite a value that
   * colima or the user had set elsewhere.
   */

  type FormState = {
    cpu: number;
    memory: number;
    disk: number;
    runtime: string;
    vmType: string;
    mountType: string;
    dns: string;
    networkAddress: boolean;
    kubernetes: boolean;
  };

  /**
   * Profiles come from the polled instance list rather than a one-shot fetch at
   * mount.
   *
   * `dataPoller` already keeps `colimaInstances` current from both the Tauri
   * event stream and the HTTP fallback, so reading it here means the panel
   * picks up an instance the moment one is created — previously you had to
   * leave Settings and come back, because `onMount` had run exactly once.
   */
  const profiles = $derived(dashboardState.colimaInstances.map((i) => i.name));

  let profile = $state("");
  let loading = $state(true);
  let saving = $state(false);
  let loadError = $state<string | null>(null);

  let host = $state<HostSpecs | null>(null);
  let mtime = $state(0);
  /** The values as loaded, used to compute what actually changed. */
  let original = $state<FormState | null>(null);
  let form = $state<FormState | null>(null);
  let preview = $state<ApplyResult | null>(null);

  const errors = $derived(preview?.issues.filter((i) => i.severity === "error") ?? []);
  const warnings = $derived(preview?.issues.filter((i) => i.severity === "warning") ?? []);
  const dirty = $derived((preview?.changes.length ?? 0) > 0);

  /** Presets scale to the host rather than to fixed numbers — half a 32 GiB
      machine is a very different "balanced" from half an 8 GiB one. */
  const presets = $derived(
    host
      ? [
          {
            id: "light",
            label: t("colima_config.preset_light", { default: "Light" }),
            cpu: Math.max(1, Math.floor(host.cpu_cores / 4)),
            memory: Math.max(2, Math.floor(host.memory_gib / 4)),
          },
          {
            id: "balanced",
            label: t("colima_config.preset_balanced", { default: "Balanced" }),
            cpu: Math.max(2, Math.floor(host.cpu_cores / 2)),
            memory: Math.max(4, Math.floor(host.memory_gib / 2)),
          },
          {
            id: "performance",
            label: t("colima_config.preset_performance", { default: "High performance" }),
            // Never every core: the VM would contend with the host scheduler.
            cpu: Math.max(4, host.cpu_cores - 2),
            memory: Math.max(8, host.memory_gib - 4),
          },
        ]
      : []
  );

  function toForm(values: Record<string, unknown>): FormState {
    const dns = values["network.dns"];
    return {
      cpu: Number(values.cpu ?? 2),
      memory: Number(values.memory ?? 2),
      disk: Number(values.disk ?? 60),
      runtime: String(values.runtime ?? "docker"),
      vmType: String(values.vmType ?? "qemu"),
      mountType: String(values.mountType ?? "sshfs"),
      dns: Array.isArray(dns) ? dns.join(", ") : "",
      networkAddress: values["network.address"] === true,
      kubernetes: values["kubernetes.enabled"] === true,
    };
  }

  function parseDns(text: string): string[] {
    return text
      .split(/[\s,]+/)
      .map((s) => s.trim())
      .filter(Boolean);
  }

  /** Only the fields that differ from what was loaded. */
  function changedFields(): ConfigChanges {
    if (!form || !original) return {};
    const changes: ConfigChanges = {};
    if (form.cpu !== original.cpu) changes.cpu = form.cpu;
    if (form.memory !== original.memory) changes.memory = form.memory;
    if (form.disk !== original.disk) changes.disk = form.disk;
    if (form.runtime !== original.runtime) changes.runtime = form.runtime;
    if (form.vmType !== original.vmType) changes.vmType = form.vmType;
    if (form.mountType !== original.mountType) changes.mountType = form.mountType;
    if (form.dns !== original.dns) changes.dns = parseDns(form.dns);
    if (form.networkAddress !== original.networkAddress)
      changes.networkAddress = form.networkAddress;
    if (form.kubernetes !== original.kubernetes) changes.kubernetes = form.kubernetes;
    return changes;
  }

  async function load(target: string) {
    loading = true;
    loadError = null;
    preview = null;
    try {
      const snapshot = await colimaConfigApi.get(target);
      mtime = snapshot.mtime;
      original = toForm(snapshot.values);
      form = { ...original };
    } catch (err) {
      // A profile that has never been started has no colima.yaml. That is a
      // normal state, not a fault, so it is shown inline rather than raised as
      // a toast and logged as a session error.
      loadError = errorMessage(normalizeError(err));
      original = null;
      form = null;
    } finally {
      loading = false;
    }
  }

  async function refreshPreview() {
    if (!form) return;
    const changes = changedFields();
    if (Object.keys(changes).length === 0) {
      preview = null;
      return;
    }
    try {
      preview = await colimaConfigApi.preview(profile, changes);
    } catch (err) {
      reportError(err, {
        action: t("colima_config.preview_action", { default: "Preview Colima config" }),
      });
    }
  }

  async function apply() {
    if (!form) return;
    saving = true;
    try {
      const result = await colimaConfigApi.apply(profile, changedFields(), mtime);
      preview = result;

      if (!result.backupPath) {
        // The write was refused. `issues` explains why and is already rendered.
        globalToast(
          "error",
          t("colima_config.not_saved", { default: "Config not saved — fix the errors above." })
        );
        return;
      }

      mtime = result.mtime;
      original = { ...form };
      preview = null;
      globalToast(
        "success",
        t("colima_config.saved", {
          default: "Config saved. Restart the instance to apply.",
        })
      );
    } catch (err) {
      reportError(err, {
        action: t("colima_config.apply_action", { default: "Save Colima config" }),
      });
    } finally {
      saving = false;
    }
  }

  function applyPreset(cpu: number, memory: number) {
    if (!form) return;
    form.cpu = cpu;
    form.memory = memory;
    refreshPreview();
  }

  /**
   * Localize one validation issue.
   *
   * The English `message` is the fallback, and Rust's `params` carry the
   * numbers so a translation can say "32 requested, host has 8" rather than a
   * vague "too many CPUs".
   */
  function issueText(issue: ValidationIssue): string {
    return t(`colima_config.issue.${issue.code}`, {
      ...issue.params,
      default: issue.message,
    });
  }

  function revert() {
    if (!original) return;
    form = { ...original };
    preview = null;
  }

  onMount(async () => {
    // Host specs are independent of Colima — they drive the presets and the
    // over-commit warnings, so they load even when no instance exists.
    try {
      host = await systemApi.hostSpecs();
    } catch {
      // Presets are hidden without host specs; the rest of the form still works.
    }
  });

  // Keep the selection valid as the polled list changes: adopt the first
  // profile when nothing is selected, and drop the selection if the instance
  // it pointed at was deleted.
  $effect(() => {
    const list = profiles;
    const current = untrack(() => profile);
    if (list.length === 0) {
      if (current !== "") profile = "";
    } else if (!list.includes(current)) {
      profile = list[0];
    }
  });

  /**
   * Load whenever the selected profile changes.
   *
   * Keyed on the profile name and guarded by `lastLoadedProfile` because this
   * effect writes `loading`, `form` and `original` via `load()`; reading any of
   * them here would make it retrigger itself. Only `profile` is tracked.
   */
  let lastLoadedProfile: string | null = null;
  $effect(() => {
    const target = profile;
    if (target === lastLoadedProfile) return;
    lastLoadedProfile = target;

    if (!target) {
      // No instance exists yet. There is no config file to read, so asking for
      // one would return NotFound and render as a failure — which is what this
      // panel used to do on a machine where Colima had never been started.
      // The template holds the loading state until `instancesLoaded` flips, so
      // clearing it here does not expose the empty state prematurely.
      loading = false;
      loadError = null;
      original = null;
      form = null;
      preview = null;
      return;
    }
    load(target);
  });
</script>

<div class="card" style="margin-bottom: 24px;">
  <h3 style="font-size: var(--text-lg); font-weight: 600; margin-bottom: 8px;">
    {t("colima_config.title", { default: "Colima Configuration" })}
  </h3>
  <p style="font-size: var(--text-sm); color: var(--text-secondary); margin-bottom: 20px;">
    {t("colima_config.description", {
      default:
        "Edit colima.yaml directly. Settings you have added by hand are preserved; changes take effect on the next restart.",
    })}
  </p>

  {#if profiles.length > 1}
    <label class="cfg-row">
      <span class="cfg-label">{t("colima_config.profile", { default: "Profile" })}</span>
      <select
        class="input"
        value={profile}
        onchange={(e) => {
          profile = (e.currentTarget as HTMLSelectElement).value;
          load(profile);
        }}
      >
        {#each profiles as p}
          <option value={p}>{p}</option>
        {/each}
      </select>
    </label>
  {/if}

  {#if loading || !dashboardState.instancesLoaded}
    <p class="cfg-muted">{t("common.loading", { default: "Loading…" })}</p>
  {:else if profiles.length === 0}
    <!-- Not an error: colima.yaml is written by the first `colima start`, so
         having none is the expected state before then. -->
    <div class="cfg-empty">
      <p class="cfg-muted">
        {t("colima_config.no_instance", {
          default:
            "No Colima instance yet. The config file is created the first time an instance starts.",
        })}
      </p>
      <button class="btn btn-ghost" onclick={() => (uiState.currentPage = "instances")}>
        {t("colima_config.go_to_instances", { default: "Go to Instances" })}
      </button>
    </div>
  {:else if loadError || !form}
    <p class="cfg-error">{loadError}</p>
  {:else}
    {#if presets.length > 0}
      <div class="cfg-presets">
        <span class="cfg-label">{t("colima_config.presets", { default: "Presets" })}</span>
        <div class="cfg-preset-buttons">
          {#each presets as preset (preset.id)}
            <button class="btn btn-ghost" onclick={() => applyPreset(preset.cpu, preset.memory)}>
              {preset.label}
              <span class="cfg-preset-detail">{preset.cpu} CPU · {preset.memory} GiB</span>
            </button>
          {/each}
        </div>
      </div>
    {/if}

    <label class="cfg-row">
      <span class="cfg-label">
        {t("colima_config.cpu", { default: "CPUs" })}
        {#if host}<span class="cfg-hint">/ {host.cpu_cores}</span>{/if}
      </span>
      <input class="input" type="number" min="1" bind:value={form.cpu} onchange={refreshPreview} />
    </label>

    <label class="cfg-row">
      <span class="cfg-label">
        {t("colima_config.memory", { default: "Memory (GiB)" })}
        {#if host}<span class="cfg-hint">/ {host.memory_gib}</span>{/if}
      </span>
      <input class="input" type="number" min="1" bind:value={form.memory} onchange={refreshPreview} />
    </label>

    <label class="cfg-row">
      <span class="cfg-label">
        {t("colima_config.disk", { default: "Disk (GiB)" })}
        <span class="cfg-hint">{t("colima_config.disk_grow_only", { default: "grow only" })}</span>
      </span>
      <input class="input" type="number" min="1" bind:value={form.disk} onchange={refreshPreview} />
    </label>

    <label class="cfg-row">
      <span class="cfg-label">{t("colima_config.runtime", { default: "Runtime" })}</span>
      <select class="input" bind:value={form.runtime} onchange={refreshPreview}>
        {#each RUNTIMES as value}<option {value}>{value}</option>{/each}
      </select>
    </label>

    <label class="cfg-row">
      <span class="cfg-label">{t("colima_config.vm_type", { default: "VM type" })}</span>
      <select class="input" bind:value={form.vmType} onchange={refreshPreview}>
        {#each VM_TYPES as value}<option {value}>{value}</option>{/each}
      </select>
    </label>

    <label class="cfg-row">
      <span class="cfg-label">{t("colima_config.mount_type", { default: "Mount type" })}</span>
      <select class="input" bind:value={form.mountType} onchange={refreshPreview}>
        {#each MOUNT_TYPES as value}<option {value}>{value}</option>{/each}
      </select>
    </label>

    <label class="cfg-row">
      <span class="cfg-label">{t("colima_config.dns", { default: "DNS servers" })}</span>
      <input
        class="input"
        type="text"
        placeholder="1.1.1.1, 8.8.8.8"
        bind:value={form.dns}
        onchange={refreshPreview}
      />
    </label>

    <label class="cfg-check">
      <input
        type="checkbox"
        class="checkbox"
        bind:checked={form.networkAddress}
        onchange={refreshPreview}
      />
      <span>{t("colima_config.network_address", { default: "Assign a reachable IP address to the VM" })}</span>
    </label>

    <label class="cfg-check">
      <input
        type="checkbox"
        class="checkbox"
        bind:checked={form.kubernetes}
        onchange={refreshPreview}
      />
      <span>{t("colima_config.kubernetes", { default: "Enable Kubernetes" })}</span>
    </label>

    {#if errors.length > 0 || warnings.length > 0}
      <ul class="cfg-issues">
        {#each errors as issue}
          <li class="cfg-issue cfg-issue-error">
            <code>{issue.field}</code>
            {issueText(issue)}
          </li>
        {/each}
        {#each warnings as issue}
          <li class="cfg-issue cfg-issue-warning">
            <code>{issue.field}</code>
            {issueText(issue)}
          </li>
        {/each}
      </ul>
    {/if}

    {#if dirty}
      <div class="cfg-diff">
        <span class="cfg-label">{t("colima_config.pending", { default: "Pending changes" })}</span>
        <DiffView changes={preview?.changes ?? []} />
      </div>
    {/if}

    <div class="cfg-actions">
      <button
        class="btn btn-primary"
        disabled={!dirty || saving || errors.length > 0}
        onclick={apply}
      >
        {saving
          ? t("common.saving", { default: "Saving…" })
          : t("colima_config.apply", { default: "Apply changes" })}
      </button>
      <button class="btn btn-ghost" disabled={!dirty || saving} onclick={revert}>
        {t("common.revert", { default: "Revert" })}
      </button>
    </div>
  {/if}
</div>

<style>
  .cfg-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    padding: 10px 0;
    border-bottom: 1px solid var(--border-subtle);
  }
  .cfg-row .input {
    max-width: 220px;
  }
  .cfg-label {
    font-size: var(--text-sm);
    font-weight: 500;
  }
  .cfg-hint {
    font-weight: 400;
    color: var(--text-muted);
    font-size: var(--text-xs);
    margin-left: 4px;
  }
  .cfg-muted {
    font-size: var(--text-sm);
    color: var(--text-secondary);
  }
  .cfg-error {
    font-size: var(--text-sm);
    color: var(--accent-red);
  }
  .cfg-empty {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    flex-wrap: wrap;
  }
  .cfg-empty .cfg-muted {
    margin: 0;
  }
  .cfg-check {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px 0;
    font-size: var(--text-sm);
    cursor: pointer;
    border-bottom: 1px solid var(--border-subtle);
  }
  .cfg-presets {
    padding: 10px 0 16px;
  }
  .cfg-preset-buttons {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
    margin-top: 8px;
  }
  .cfg-preset-detail {
    display: block;
    font-size: var(--text-xs);
    color: var(--text-muted);
    font-weight: 400;
  }
  .cfg-issues {
    list-style: none;
    margin: 16px 0 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .cfg-issue {
    font-size: var(--text-sm);
    padding: 8px 10px;
    border-radius: 6px;
    border-left: 3px solid transparent;
  }
  .cfg-issue code {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    margin-right: 8px;
  }
  .cfg-issue-error {
    background: var(--bg-primary);
    border-left-color: var(--accent-red);
  }
  .cfg-issue-warning {
    background: var(--bg-primary);
    border-left-color: var(--accent-yellow);
  }
  .cfg-diff {
    margin-top: 16px;
  }
  .cfg-diff .cfg-label {
    display: block;
    margin-bottom: 8px;
  }
  .cfg-actions {
    display: flex;
    gap: 8px;
    margin-top: 20px;
  }
</style>
