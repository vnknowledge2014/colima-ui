<script lang="ts">
  import { onMount } from "svelte";
  import { limaApi, type LimaInstance } from "../lib/api";
  import { globalToast } from "../lib/globalToast";
  import { confirm } from "../store/confirm.svelte";
  import { t } from "../lib/i18n.svelte";

  let vms = $state<LimaInstance[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let actionLoading = $state<string | null>(null);
  let selectedVM = $state<LimaInstance | null>(null);
  let shellCmd = $state("");
  let shellOutput = $state("");
  let shellCwd = $state("/");

  let showCreate = $state(false);
  let templates = $state<string[]>([]);
  let newVM = $state({ name: "", cpus: 2, memory: 2, disk: 60, template: "" });

  let timeoutId: ReturnType<typeof setTimeout> | null = null;
  let intervalId: ReturnType<typeof setInterval> | null = null;

  async function fetchVMs() {
    try {
      error = null;
      const list = await limaApi.list();
      vms = list;
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  onMount(() => {
    fetchVMs();
    intervalId = setInterval(() => {
      if (document.visibilityState === "visible") fetchVMs();
    }, 15000);

    return () => {
      if (intervalId) clearInterval(intervalId);
      if (timeoutId) clearTimeout(timeoutId);
    };
  });

  $effect(() => {
    if (showCreate && templates.length === 0) {
      limaApi
        .templates()
        .then((raw) => {
          try {
            const lines = raw
              .split("\n")
              .map((l) => l.trim())
              .filter((l) => l && !l.startsWith("-"));
            templates =
              lines.length > 0
                ? lines
                : ["default", "docker", "ubuntu", "fedora", "alpine", "debian"];
          } catch {
            templates = [
              "default",
              "docker",
              "ubuntu",
              "fedora",
              "alpine",
              "debian",
            ];
          }
        })
        .catch(() => {
          templates = [
            "default",
            "docker",
            "ubuntu",
            "fedora",
            "alpine",
            "debian",
          ];
        });
    }
  });

  async function handleCreate() {
    if (!newVM.name.trim()) return;
    const name = newVM.name.trim().toLowerCase();
    globalToast(
      "success",
      `Creating VM '${name}'... This may take a few minutes.`,
    );
    showCreate = false;
    newVM = { name: "", cpus: 2, memory: 2, disk: 60, template: "" };

    try {
      await limaApi.create({
        name,
        cpus: newVM.cpus,
        memory: newVM.memory,
        disk: newVM.disk,
        template: newVM.template || undefined,
      });
      globalToast("success", `VM '${name}' created successfully`);
      if (timeoutId) clearTimeout(timeoutId);
      timeoutId = setTimeout(fetchVMs, 2000);
    } catch (e) {
      globalToast("error", `Failed to create VM: ${e}`);
    }
  }

  async function handleAction(
    name: string,
    action: "start" | "stop" | "delete",
  ) {
    actionLoading = `${name}-${action}`;
    try {
      if (action === "start") {
        globalToast("success", `Starting VM '${name}'...`);
        limaApi
          .start(name)
          .then(() => {
            globalToast("success", `VM '${name}' started`);
            if (timeoutId) clearTimeout(timeoutId);
            timeoutId = setTimeout(fetchVMs, 1000);
          })
          .catch((e) => globalToast("error", String(e)))
          .finally(() => (actionLoading = null));
        return;
      } else if (action === "stop") {
        await limaApi.stop(name);
        globalToast("success", `VM '${name}' stopped`);
      } else {
        const ok = await confirm({
          title: "Delete VM",
          message: `Delete VM '${name}'? This cannot be undone.`,
          confirmText: "Delete",
          variant: "danger",
        });
        if (!ok) {
          actionLoading = null;
          return;
        }
        await limaApi.delete(name, true);
        globalToast("success", `VM '${name}' deleted`);
      }
      if (timeoutId) clearTimeout(timeoutId);
      timeoutId = setTimeout(fetchVMs, 1000);
    } catch (e) {
      globalToast("error", String(e));
    } finally {
      actionLoading = null;
    }
  }

  async function runShell() {
    if (!selectedVM || !shellCmd.trim()) return;
    const cmd = shellCmd.trim();

    if (cmd === "cd" || cmd === "cd ~" || cmd === "cd ~/") {
      shellOutput += `${selectedVM.name}:${shellCwd}$ ${cmd}\n`;
      shellCwd = "/";
      shellCmd = "";
      return;
    }
    if (cmd.startsWith("cd ")) {
      const target = cmd.slice(3).trim();
      try {
        const cwdCmd = `cd ${shellCwd} && cd ${target} && pwd`;
        const newPath = await limaApi.shell(selectedVM.name, cwdCmd);
        const resolved = newPath.trim();
        if (resolved) {
          shellOutput += `${selectedVM.name}:${shellCwd}$ ${cmd}\n`;
          shellCwd = resolved;
        } else {
          shellOutput += `${selectedVM.name}:${shellCwd}$ ${cmd}\ncd: no such directory: ${target}\n`;
        }
      } catch (e) {
        shellOutput += `${selectedVM.name}:${shellCwd}$ ${cmd}\n${e}\n`;
      }
      shellCmd = "";
      return;
    }

    const fullCmd = `cd ${shellCwd} && ${cmd}`;
    try {
      const output = await limaApi.shell(selectedVM.name, fullCmd);
      shellOutput += `${selectedVM.name}:${shellCwd}$ ${cmd}\n${output}\n`;
      shellCmd = "";
    } catch (e) {
      shellOutput += `${selectedVM.name}:${shellCwd}$ ${cmd}\nError: ${e}\n`;
      shellCmd = "";
    }
  }

  function statusColor(status: string) {
    if (status === "Running") return "var(--accent-green)";
    if (status === "Stopped") return "var(--accent-red)";
    return "var(--text-muted)";
  }
</script>

{#if loading}
  <div class="content-header" data-tauri-drag-region><h1>{t('linux_vms.loading_title', { default: 'Linux VMs' })}</h1></div>
  <div class="loading-screen">
    <div class="spinner"></div>
    <span>{t('linux_vms.loading', { default: 'Loading VMs...' })}</span>
  </div>
{:else}
  <div class="content-header" data-tauri-drag-region>
    <h1>
      {t('linux_vms.title', { default: 'Linux VMs (Lima)' })}
      <span
        style="font-size: var(--text-sm); color: var(--text-muted); font-weight: 400; margin-left: 12px;"
      >
        {vms.length} {t('linux_vms.vm_count', { default: 'VMs' })}
      </span>
    </h1>
    <div class="content-header-actions" style="display: flex; gap: 8px;">
      <button class="btn btn-ghost" onclick={fetchVMs} aria-label="{t('linux_vms.refresh', { default: 'Refresh VMs' })}" title="{t('linux_vms.refresh', { default: 'Refresh VMs' })}">
        <svg
          width="14"
          height="14"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
        >
          <path
            d="M21.5 2v6h-6M2.5 22v-6h6M2 11.5a10 10 0 0 1 18.8-4.3M22 12.5a10 10 0 0 1-18.8 4.3"
          />
        </svg>
      </button>
      <button class="btn btn-primary" onclick={() => (showCreate = true)} style="display: flex; align-items: center; gap: 6px;">
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg> {t('linux_vms.new_vm', { default: 'New VM' })}
      </button>
    </div>
  </div>

  <div class="content-body">
    {#if error}
      <div
        class="card"
        style="border-color: var(--accent-yellow); margin-bottom: 16px;"
      >
        <p
          style="color: var(--accent-yellow); font-size: var(--text-sm); display: flex; align-items: center; gap: 6px;"
        >
          <svg
            width="14"
            height="14"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            ><path
              d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"
            /><line x1="12" y1="9" x2="12" y2="13" /><line
              x1="12"
              y1="17"
              x2="12.01"
              y2="17"
            /></svg
          >
          {error}
        </p>
      </div>
    {/if}

    {#if vms.length > 0}
      <div style="display: flex; flex-direction: column; gap: 8px;">
        {#each vms as vm (vm.name)}
          {@const isLoading = actionLoading?.startsWith(vm.name)}
          {@const isRunning = vm.status === "Running"}
          <div
            onclick={() => {
              selectedVM = vm;
              shellOutput = "";
              shellCwd = "/";
            }}
            style="padding: 16px; background: var(--bg-secondary); border-radius: 12px; border: 1px solid var(--border-primary); cursor: pointer; opacity: {isLoading
              ? 0.6
              : 1}; transition: all 200ms;"
          >
            <div
              style="display: flex; justify-content: space-between; align-items: center;"
            >
              <div>
                <div style="display: flex; align-items: center; gap: 8px;">
                  <svg
                    width="18"
                    height="18"
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke={isRunning
                      ? "var(--accent-green)"
                      : "var(--text-muted)"}
                    stroke-width="2"
                  >
                    <rect x="2" y="3" width="20" height="14" rx="2" /><line
                      x1="8"
                      y1="21"
                      x2="16"
                      y2="21"
                    /><line x1="12" y1="17" x2="12" y2="21" />
                  </svg>
                  <span style="font-weight: 600; font-size: var(--text-md);"
                    >{vm.name}</span
                  >
                  <span
                    style="color: {statusColor(
                      vm.status,
                    )}; font-weight: 500; font-size: var(--text-xs);"
                  >
                    <svg
                      width="8"
                      height="8"
                      viewBox="0 0 24 24"
                      fill={statusColor(vm.status)}
                      style="display: inline-block; vertical-align: middle; margin-right: 2px;"
                      ><circle cx="12" cy="12" r="10" /></svg
                    >
                    {vm.status}
                  </span>
                </div>
                <div
                  style="display: flex; gap: 16px; margin-top: 4px; font-size: var(--text-xs); color: var(--text-muted); font-family: var(--font-mono);"
                >
                  <span>{vm.arch}</span>
                  <span>{vm.cpus} CPU</span>
                  <span>{vm.memory}</span>
                  <span>{vm.disk}</span>
                </div>
              </div>
              <div
                style="display: flex; gap: 6px;"
                onclick={(e) => e.stopPropagation()}
              >
                {#if isRunning}
                  <button
                    class="btn btn-ghost"
                    style="font-size: var(--text-xs);"
                    disabled={!!isLoading}
                    onclick={() => handleAction(vm.name, "stop")}
                  >
                    <svg
                      width="12"
                      height="12"
                      viewBox="0 0 24 24"
                      fill="currentColor"
                      ><rect x="4" y="4" width="16" height="16" rx="2" /></svg
                    > {t('linux_vms.stop', { default: 'Stop' })}
                  </button>
                {:else}
                  <button
                    class="btn btn-ghost"
                    style="font-size: var(--text-xs); color: var(--accent-green);"
                    disabled={!!isLoading}
                    onclick={() => handleAction(vm.name, "start")}
                  >
                    <svg
                      width="12"
                      height="12"
                      viewBox="0 0 24 24"
                      fill="currentColor"
                      ><polygon points="5 3 19 12 5 21 5 3" /></svg
                    > {t('linux_vms.start', { default: 'Start' })}
                  </button>
                {/if}
                <button
                  class="btn btn-ghost"
                  style="font-size: var(--text-xs); color: var(--accent-red);"
                  disabled={!!isLoading}
                  onclick={() => handleAction(vm.name, "delete")}
                  aria-label="{t('linux_vms.delete', { default: 'Delete VM' })}"
                  title="{t('linux_vms.delete', { default: 'Delete VM' })}"
                >
                  <svg
                    width="12"
                    height="12"
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    stroke-width="2"
                    ><polyline points="3 6 5 6 21 6" /><path
                      d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"
                    /></svg
                  >
                </button>
              </div>
            </div>
          </div>
        {/each}
      </div>
    {:else}
      <div class="empty-state">
        <div class="empty-state-icon">
          <svg
            width="32"
            height="32"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="1.5"
            style="color: var(--text-muted);"
          >
            <rect x="2" y="3" width="20" height="14" rx="2" /><line
              x1="8"
              y1="21"
              x2="16"
              y2="21"
            /><line x1="12" y1="17" x2="12" y2="21" />
          </svg>
        </div>
        <div class="empty-state-title">{t('linux_vms.no_vms_title', { default: 'No Linux VMs' })}</div>
        <div class="empty-state-text">{t('linux_vms.no_vms_text', { default: 'Create a Linux VM to get started.' })}</div>
        <button class="btn btn-primary" onclick={() => (showCreate = true)} style="display: flex; align-items: center; gap: 6px; margin: 0 auto;"
          ><svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg> {t('linux_vms.new_vm', { default: 'New VM' })}</button
        >
      </div>
    {/if}
  </div>

  {#if selectedVM}
    <div
      style="position: fixed; inset: 0; z-index: 1000; background: rgba(0,0,0,0.6); backdrop-filter: blur(4px); display: flex; align-items: center; justify-content: center;"
      onclick={(e) => {
        if (e.target === e.currentTarget) selectedVM = null;
      }}
    >
      <div
        style="background: var(--bg-primary); border: 1px solid var(--border-primary); border-radius: 12px; width: min(800px, 95vw); max-height: 80vh; display: flex; flex-direction: column; overflow: hidden; box-shadow: 0 20px 40px rgba(0,0,0,0.5);"
      >
        <div
          style="padding: 16px 20px; border-bottom: 1px solid var(--border-primary); display: flex; justify-content: space-between; align-items: center;"
        >
          <h2
            style="margin: 0; font-size: var(--text-lg); color: var(--text-primary); display: flex; align-items: center; gap: 8px;"
          >
            {selectedVM.name}
            <span
              style="color: {statusColor(
                selectedVM.status,
              )}; font-size: var(--text-sm); font-weight: 500;"
            >
              <svg
                width="8"
                height="8"
                viewBox="0 0 24 24"
                fill={statusColor(selectedVM.status)}
                style="display: inline-block; vertical-align: middle; margin-right: 2px;"
                ><circle cx="12" cy="12" r="10" /></svg
              >
              {selectedVM.status}
            </span>
          </h2>
          <button class="btn btn-ghost" onclick={() => (selectedVM = null)} aria-label="Close details" title="Close details">
            <svg
              width="16"
              height="16"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
              ><line x1="18" y1="6" x2="6" y2="18" /><line
                x1="6"
                y1="6"
                x2="18"
                y2="18"
              /></svg
            >
          </button>
        </div>

        <div style="padding: 20px; overflow-y: auto;">
          <div
            style="display: grid; grid-template-columns: repeat(4, 1fr); gap: 8px; margin-bottom: 16px;"
          >
            <div
              style="padding: 10px; background: var(--bg-secondary); border-radius: 8px; text-align: center;"
            >
              <div style="font-size: var(--text-xs); color: var(--text-muted);">
                Arch
              </div>
              <div style="font-weight: 600; font-family: var(--font-mono);">
                {selectedVM.arch}
              </div>
            </div>
            <div
              style="padding: 10px; background: var(--bg-secondary); border-radius: 8px; text-align: center;"
            >
              <div style="font-size: var(--text-xs); color: var(--text-muted);">
                CPUs
              </div>
              <div style="font-weight: 600; font-family: var(--font-mono);">
                {selectedVM.cpus}
              </div>
            </div>
            <div
              style="padding: 10px; background: var(--bg-secondary); border-radius: 8px; text-align: center;"
            >
              <div style="font-size: var(--text-xs); color: var(--text-muted);">
                Memory
              </div>
              <div style="font-weight: 600; font-family: var(--font-mono);">
                {selectedVM.memory}
              </div>
            </div>
            <div
              style="padding: 10px; background: var(--bg-secondary); border-radius: 8px; text-align: center;"
            >
              <div style="font-size: var(--text-xs); color: var(--text-muted);">
                Disk
              </div>
              <div style="font-weight: 600; font-family: var(--font-mono);">
                {selectedVM.disk}
              </div>
            </div>
          </div>

          {#if selectedVM.status === "Running"}
            <h3
              style="font-size: var(--text-sm); font-weight: 600; margin: 0 0 8px 0;"
            >
              Shell
            </h3>
            <div
              style="background: var(--bg-secondary); border-radius: 8px; border: 1px solid var(--border-primary); padding: 12px; margin-bottom: 12px; font-family: var(--font-mono); font-size: var(--text-xs); min-height: 120px; max-height: 300px; overflow: auto; white-space: pre-wrap; color: var(--text-secondary);"
            >
              {shellOutput || `Run commands inside '${selectedVM.name}' VM...`}
            </div>
            <div style="display: flex; gap: 8px;">
              <input
                type="text"
                bind:value={shellCmd}
                onkeydown={(e) => e.key === "Enter" && runShell()}
                placeholder="{selectedVM.name}:{shellCwd}$ Enter command..."
                class="input"
                style="flex: 1; font-family: var(--font-mono);"
              />
              <button class="btn btn-primary" onclick={runShell}>Run</button>
            </div>
          {/if}
        </div>
        <div
          style="padding: 16px 20px; border-top: 1px solid var(--border-primary); background: var(--bg-secondary); display: flex; justify-content: flex-end;"
        >
          <button class="btn btn-primary" onclick={() => (selectedVM = null)}
            >Close</button
          >
        </div>
      </div>
    </div>
  {/if}

  {#if showCreate}
    <div
      style="position: fixed; inset: 0; z-index: 2000; background: rgba(0,0,0,0.6); backdrop-filter: blur(4px); display: flex; align-items: center; justify-content: center;"
      onclick={(e) => {
        if (e.target === e.currentTarget) showCreate = false;
      }}
    >
      <div
        style="background: var(--bg-primary); border: 1px solid var(--border-primary); border-radius: 12px; width: min(560px, 95vw); box-shadow: 0 20px 40px rgba(0,0,0,0.5);"
      >
        <div
          style="padding: 16px 20px; border-bottom: 1px solid var(--border-primary); display: flex; justify-content: space-between; align-items: center;"
        >
          <h2
            style="margin: 0; font-size: var(--text-lg); color: var(--text-primary);"
          >
            Create VM
          </h2>
          <button class="btn btn-ghost" onclick={() => (showCreate = false)} aria-label="Close dialog" title="Close dialog">
            <svg
              width="16"
              height="16"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
              ><line x1="18" y1="6" x2="6" y2="18" /><line
                x1="6"
                y1="6"
                x2="18"
                y2="18"
              /></svg
            >
          </button>
        </div>

        <div
          style="padding: 20px; display: flex; flex-direction: column; gap: 16px;"
        >
          <div>
            <label
              for="limaVMName"
              style="display: block; font-size: var(--text-sm); font-weight: 500; color: var(--text-secondary); margin-bottom: 6px;"
              >VM Name</label
            >
            <input
              id="limaVMName"
              type="text"
              bind:value={newVM.name}
              placeholder="my-vm"
              class="input"
            />
          </div>

          <div>
            <label
              for="limaTemplate"
              style="display: block; font-size: var(--text-sm); font-weight: 500; color: var(--text-secondary); margin-bottom: 6px;"
              >Template</label
            >
            <select
              id="limaTemplate"
              bind:value={newVM.template}
              class="input select"
            >
              <option value="">Default (Ubuntu)</option>
              {#each templates as t (t)}
                <option value={t}>{t}</option>
              {/each}
            </select>
          </div>

          <div
            style="display: grid; grid-template-columns: repeat(3, 1fr); gap: 12px;"
          >
            <div>
              <label
                for="newVmCpus"
                style="display: block; font-size: var(--text-sm); font-weight: 500; color: var(--text-secondary); margin-bottom: 6px;"
                >CPUs</label
              >
              <input
                id="newVmCpus"
                type="number"
                bind:value={newVM.cpus}
                min="1"
                max="16"
                class="input"
              />
            </div>
            <div>
              <label
                for="newVmMem"
                style="display: block; font-size: var(--text-sm); font-weight: 500; color: var(--text-secondary); margin-bottom: 6px;"
                >Memory (GiB)</label
              >
              <input
                id="newVmMem"
                type="number"
                bind:value={newVM.memory}
                min="1"
                max="64"
                class="input"
              />
            </div>
            <div>
              <label
                for="newVmDisk"
                style="display: block; font-size: var(--text-sm); font-weight: 500; color: var(--text-secondary); margin-bottom: 6px;"
                >Disk (GiB)</label
              >
              <input
                id="newVmDisk"
                type="number"
                bind:value={newVM.disk}
                min="10"
                max="500"
                class="input"
              />
            </div>
          </div>
        </div>

        <div
          style="padding: 16px 20px; border-top: 1px solid var(--border-primary); background: var(--bg-secondary); display: flex; justify-content: flex-end; gap: 8px;"
        >
          <button class="btn btn-ghost" onclick={() => (showCreate = false)}
            >Cancel</button
          >
          <button
            class="btn btn-primary"
            onclick={handleCreate}
            disabled={!newVM.name.trim()}>Create & Start</button
          >
        </div>
      </div>
    </div>
  {/if}
{/if}
