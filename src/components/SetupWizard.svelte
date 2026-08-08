<script lang="ts">
  import { onMount } from "svelte";
  import { sysMethods, colimaApi, type SystemInfo, type PlatformInfo } from "../lib/api";
  import { setAppSetting } from "../lib/settingsStore.svelte";
  import { t, getLanguage } from "../lib/i18n.svelte";
  import Icon from "./Icon.svelte";

  let { systemInfo, onComplete, onSkip } = $props<{
    systemInfo: SystemInfo | null;
    onComplete: () => void;
    onSkip: () => void;
  }>();

  type DepName = "homebrew" | "colima" | "docker" | "lima";
  type DepStatus = "checking" | "installed" | "missing" | "installing" | "failed";
  type InstallMethod = "brew" | "apt" | "nix" | "wsl-brew" | "manual";

  interface OptionalTool {
    name: string;
    label: string;
    desc: string;
    brewPkg: string;
    installed: boolean;
    version: string;
    checking: boolean;
  }

  interface DepState {
    name: DepName;
    label: string;
    desc: string;
    icon: string;
    status: DepStatus;
    version: string;
  }

  const STEPS = ["Welcome", "Dependencies", "Quick Setup", "Complete"];

  const OS_LABELS: Record<string, { label: string; icon: string }> = {
    macos: { label: "macOS", icon: "Monitor" },
    linux: { label: "Linux", icon: "Monitor" },
    windows: { label: "Windows", icon: "Monitor" },
  };

  const METHOD_LABELS: Record<string, { label: string; icon: string; desc: string }> = {
    brew: { label: "Homebrew", icon: "Homebrew", desc: "Recommended for macOS & Linux" },
    apt: { label: "APT", icon: "Package", desc: "Debian/Ubuntu package manager" },
    nix: { label: "Nix", icon: "Snowflake", desc: "Reproducible package manager" },
    "wsl-brew": { label: "WSL + Homebrew", icon: "Monitor", desc: "Install via Windows Subsystem for Linux" },
    manual: { label: "Manual", icon: "Gear", desc: "Download and install manually" },
  };

  let step = $state(0);
  let platform = $state<PlatformInfo | null>(null);
  let installMethod = $state<InstallMethod>("brew");
  let deps = $state<DepState[]>([
    { name: "homebrew", label: "Homebrew", desc: "Package manager for macOS", icon: "Homebrew", status: "checking", version: "" },
    { name: "colima", label: "Colima", desc: "Container runtime manager", icon: "Package", status: "checking", version: "" },
    { name: "docker", label: "Docker CLI", desc: "Container engine client", icon: "Package", status: "checking", version: "" },
    { name: "lima", label: "Lima", desc: "Linux virtual machine manager", icon: "Monitor", status: "checking", version: "" },
  ]);
  let autostart = $state(true);
  let createInstance = $state(true);
  let settingUp = $state(false);
  let setupLog = $state("");
  let optionalTools = $state<OptionalTool[]>([
    { name: "kubectl", label: "kubectl", desc: "Kubernetes CLI — required for K8s tab", brewPkg: "kubectl", installed: false, version: "", checking: true },
    { name: "kind", label: "kind", desc: "Kubernetes in Docker — create local clusters", brewPkg: "kind", installed: false, version: "", checking: true },
    { name: "helm", label: "helm", desc: "Kubernetes package manager", brewPkg: "helm", installed: false, version: "", checking: true },
    { name: "krunkit", label: "krunkit", desc: "GPU support for AI Models", brewPkg: "slp/krunkit/krunkit", installed: false, version: "", checking: true },
    { name: "qemu-img", label: "QEMU", desc: "Emulator and virtualizer for cross-arch VMs", brewPkg: "qemu", installed: false, version: "", checking: true },
  ]);

  onMount(() => {
    sysMethods.getPlatform().then(p => {
      platform = p;
      if (p.os === "windows" && p.wsl_available) {
        installMethod = "wsl-brew";
      } else if (p.os === "linux") {
        const hasBrew = p.package_managers.find(pm => pm.name === "brew")?.available;
        const hasApt = p.package_managers.find(pm => pm.name === "apt")?.available;
        if (hasBrew) installMethod = "brew";
        else if (hasApt) installMethod = "apt";
        else installMethod = "manual";
      } else {
        installMethod = "brew";
      }
    }).catch(() => {
      platform = { os: "macos", arch: "aarch64", wsl: false, wsl_available: false, package_managers: [] };
    });
  });

  async function checkDeps() {
    let updated = [...deps];

    try {
      const brew = await sysMethods.checkHomebrew();
      updated[0].status = brew.installed ? "installed" : "missing";
      updated[0].version = brew.version;
    } catch {
      if (systemInfo?.colima_installed || systemInfo?.lima_installed) {
        updated[0].status = "installed";
        updated[0].version = "";
      } else {
        updated[0].status = "missing";
      }
    }

    if (systemInfo) {
      updated[1].status = systemInfo.colima_installed ? "installed" : "missing";
      updated[1].version = systemInfo.colima_version ? systemInfo.colima_version.split("\n")[0] : "";
      updated[2].status = systemInfo.docker_installed ? "installed" : "missing";
      updated[2].version = systemInfo.docker_version ? systemInfo.docker_version.split("\n")[0] : "";
      updated[3].status = systemInfo.lima_installed ? "installed" : "missing";
      updated[3].version = systemInfo.lima_version ? systemInfo.lima_version.split("\n")[0] : "";
    } else {
      for (let i = 1; i <= 3; i++) updated[i].status = "missing";
    }

    if (platform) {
      const pm = platform.package_managers.find(p => p.name === (installMethod === "wsl-brew" ? "brew" : installMethod));
      if (pm) {
        const methodInfo = METHOD_LABELS[installMethod];
        updated[0].label = methodInfo?.label || "Package Manager";
        updated[0].desc = methodInfo?.desc || "Package manager";
        updated[0].icon = methodInfo?.icon || "Package";
        updated[0].status = pm.available ? "installed" : "missing";
        updated[0].version = pm.version;
      }
    } else if (installMethod === "manual") {
      updated[0].label = "Manual";
      updated[0].icon = "Gear";
      updated[0].status = "installed";
      updated[0].desc = "Download and install manually";
    }

    deps = updated;
  }

  async function checkOptionalTools() {
    for (let i = 0; i < optionalTools.length; i++) {
      try {
        const result = await sysMethods.checkTool(optionalTools[i].name);
        optionalTools[i].installed = result.installed;
        optionalTools[i].version = result.version ? result.version.split("\n")[0] : "";
        optionalTools[i].checking = false;
      } catch {
        optionalTools[i].checking = false;
      }
    }
  }

  import { untrack } from "svelte";

  // Effect to re-check when needed (like useEffect dependencies)
  $effect(() => {
    platform;
    installMethod;
    systemInfo;

    untrack(() => {
      checkDeps();
      checkOptionalTools();
    });
  });

  let missingDeps = $derived(deps.filter(d => d.status === "missing"));
  let allInstalled = $derived(deps.every(d => d.status === "installed"));
  let isInstalling = $derived(deps.some(d => d.status === "installing"));

  function getAvailableMethods(): InstallMethod[] {
    if (!platform) return ["brew", "manual"];
    const methods: InstallMethod[] = [];
    if (platform.os === "macos") {
      methods.push("brew", "nix", "manual");
    } else if (platform.os === "linux") {
      methods.push("brew");
      if (platform.package_managers.find(pm => pm.name === "apt")?.available) {
        methods.push("apt");
      }
      methods.push("nix", "manual");
    } else if (platform.os === "windows") {
      if (platform.wsl_available) methods.push("wsl-brew");
      methods.push("manual");
    }
    return methods;
  }

  async function handleInstallAll() {
    if (installMethod === "manual") return;

    for (let i = 1; i < deps.length; i++) {
      if (deps[i].status !== "missing") continue;
      const depName = deps[i].name as "colima" | "docker" | "lima";
      deps[i].status = "installing";

      try {
        const result = await sysMethods.installDep(depName, installMethod);
        deps[i].status = result.success ? "installed" : "failed";
      } catch {
        deps[i].status = "failed";
      }
    }
  }

  async function handleInstallSingle(index: number) {
    if (index === 0 || installMethod === "manual") return;
    const depName = deps[index].name as "colima" | "docker" | "lima";
    deps[index].status = "installing";

    try {
      const result = await sysMethods.installDep(depName, installMethod);
      deps[index].status = result.success ? "installed" : "failed";
    } catch {
      deps[index].status = "failed";
    }
  }

  async function handleQuickSetup() {
    settingUp = true;
    setupLog = "Configuring...";

    try {
      if (autostart) {
        setupLog = "Setting up auto-start on boot...";
        try {
          await sysMethods.configureAutostart(true);
          setupLog = "✓ Auto-start configured";
        } catch {
          setupLog = "⚠ Could not configure auto-start (will need manual setup)";
        }
      }

      if (createInstance) {
        setupLog += "\nCreating default Colima instance...";
        try {
          await colimaApi.startInstance({
            profile: "default",
            runtime: "docker",
            vm_type: "qemu",
            cpus: 2,
            memory: 4,
            disk: 60,
            kubernetes: false,
            kubernetes_version: "",
            arch: platform?.arch || "aarch64",
            mount_type: "",
            mounts: [],
            dns: [],
            network_address: false,
          });
          setupLog += "\n✓ Default instance created and starting";
        } catch {
          setupLog += "\n⚠ Instance may already exist or Colima is not available yet";
        }
      }

      setupLog += "\n\n✓ Setup complete!";
      setTimeout(() => step = 3, 1000);
    } finally {
      settingUp = false;
    }
  }

  let osInfo = $derived(OS_LABELS[platform?.os || "macos"] || OS_LABELS.macos);
  let availableMethods = $derived(getAvailableMethods());
</script>

<div class="wizard-overlay">
  <div class="wizard-card">
    <div class="wizard-progress">
      {#each STEPS as _, i}
        <div class="wizard-progress-dot {i === step ? 'active' : i < step ? 'done' : ''}"></div>
      {/each}
    </div>
    
    {#if step === 0}
      <div class="wizard-logo">C</div>
      <div class="wizard-title">{t('getting_started.title', { default: 'Welcome to ColimaUI' })}</div>
      <div class="wizard-subtitle">
        {t('getting_started.subtitle', { default: 'Your cross-platform graphical interface for managing Colima instances, Docker containers, Kubernetes clusters, and Linux VMs.' })}
        <br /><br />
        <div style="display: flex; justify-content: center; align-items: center; gap: 10px; margin-bottom: 20px;">
          <label for="lang-select" style="font-size: var(--text-sm);">{t('getting_started.language_select', { default: 'Select Language' })}</label>
          <select 
            id="lang-select" 
            class="settings-select" 
            style="width: 150px;"
            value={getLanguage()}
            onchange={(e) => setAppSetting("app.language", e.currentTarget.value)}
          >
            <option value="en">English</option>
            <option value="vi">Tiếng Việt</option>
            <option value="zh">中文</option>
            <option value="ja">日本語</option>
          </select>
        </div>
        
        {#if platform}
          <span style="display: inline-flex; align-items: center; gap: 6px; padding: 4px 12px; background: rgba(88,166,255,0.1); border-radius: 20px; border: 1px solid rgba(88,166,255,0.2); font-size: var(--text-xs);">
            <span><Icon name={osInfo.icon as any} size={16} /></span>
            <span>Detected: <strong>{osInfo.label}</strong> ({platform.arch})</span>
            {#if platform.wsl}<span style="color: var(--accent-yellow);">• WSL</span>{/if}
          </span>
        {/if}
      </div>
      <div class="wizard-actions" style="justify-content: center;">
        <button class="btn btn-ghost" onclick={onSkip}>{t('getting_started.skip', { default: 'Skip Setup' })}</button>
        <button class="btn btn-primary" onclick={() => step = 1} style="padding: 10px 32px; font-size: var(--text-base);">
          {t('getting_started.next', { default: 'Get Started' })} →
        </button>
      </div>
    {:else if step === 1}
      <h2 style="font-size: var(--text-xl); font-weight: 700; margin-bottom: 8px;">
        System Dependencies
      </h2>
      <p style="color: var(--text-muted); font-size: var(--text-sm); margin-bottom: 16px;">
        ColimaUI requires these tools. Choose your preferred installation method.
      </p>

      {#if availableMethods.length > 1}
        <div style="display: flex; gap: 6px; margin-bottom: 20px; flex-wrap: wrap;">
          {#each availableMethods as m}
            {@const info = METHOD_LABELS[m]}
            {@const isActive = installMethod === m}
            {@const pmInfo = platform?.package_managers.find(pm => pm.name === m)}
            {@const isManual = m === "manual"}
            {@const isAvailable = isManual || pmInfo?.available}
            <button
              class="btn {isActive ? 'btn-primary' : 'btn-ghost'}"
              style="font-size: var(--text-xs); padding: 6px 12px; opacity: {isAvailable ? 1 : 0.5}; display: flex; align-items: center; gap: 4px;"
              onclick={() => installMethod = m}
              disabled={!isAvailable && !isManual}
              title={info.desc}
            >
              <span><Icon name={info.icon as any} size={16} /></span>
              <span>{info.label}</span>
              {#if !isAvailable && !isManual}
                <span style="font-size: 9px; opacity: 0.6;">(not found)</span>
              {/if}
            </button>
          {/each}
        </div>
      {/if}

      {#if platform?.os === "windows" && !platform.wsl_available}
        <div style="padding: 12px; background: rgba(248, 81, 73, 0.1); border-radius: var(--radius-md); border: 1px solid rgba(248, 81, 73, 0.3); margin-bottom: 16px; font-size: var(--text-xs); color: var(--accent-red);">
          <Icon name="Warning" size={14} style="vertical-align: middle; margin-right: 4px;" /> WSL is required to run Colima on Windows. Install it with:
          <code style="display: block; margin-top: 4px; padding: 4px 8px; background: var(--bg-content); border-radius: 4px;">
            wsl --install -d Ubuntu
          </code>
        </div>
      {/if}

      <div class="dep-list">
        {#each deps as dep, i (dep.name)}
          <div class="dep-row">
            <div class="dep-info">
              <div class="dep-icon {dep.status === 'installed' ? 'installed' : dep.status === 'installing' ? 'installing' : 'missing'}">
                <Icon name={dep.icon as any} size={16} />
              </div>
              <div>
                <div class="dep-name">{dep.label}</div>
                <div class="dep-desc">{dep.desc}</div>
              </div>
            </div>
            <div class="dep-status">
              {#if dep.version}<span class="dep-version">{dep.version}</span>{/if}
              {#if dep.status === "installed"}
                <span class="badge badge-running">
                  <span class="badge-dot" style="animation: none;"></span>
                  <span>Installed</span>
                </span>
              {:else if dep.status === "missing"}
                <span class="badge badge-stopped">
                  <span>Missing</span>
                </span>
                {#if i === 0}
                  {#if installMethod === "brew" || installMethod === "wsl-brew"}
                    <a href="https://brew.sh" target="_blank" rel="noopener noreferrer" class="btn btn-ghost" style="font-size: var(--text-xs); padding: 2px 8px; text-decoration: none;">Install ↗</a>
                  {/if}
                {:else if installMethod !== "manual"}
                  <button class="btn btn-primary" style="font-size: var(--text-xs); padding: 4px 10px;" onclick={() => handleInstallSingle(i)}>Install</button>
                {/if}
              {:else if dep.status === "installing"}
                <span class="badge" style="background: rgba(88,166,255,0.15); color: var(--accent-blue);">
                  <div class="spinner" style="width: 10px; height: 10px; border-width: 1.5px;"></div> Installing...
                </span>
              {:else if dep.status === "checking"}
                <span class="badge" style="background: rgba(139, 148, 158, 0.1); color: var(--text-secondary);">
                  <div class="spinner" style="width: 10px; height: 10px; border-width: 1.5px;"></div>
                  <span>Checking</span>
                </span>
              {:else if dep.status === "failed"}
                <span class="badge badge-stopped">
                  <span>Failed</span>
                </span>
              {/if}
            </div>
          </div>
        {/each}
      </div>

      {#if missingDeps.length > 0 && missingDeps.some(d => d.name !== "homebrew") && installMethod !== "manual"}
        <button
          class="btn btn-primary"
          style="width: 100%; padding: 10px; margin-bottom: 16px;"
          onclick={handleInstallAll}
          disabled={isInstalling}
        >
          {#if isInstalling}
            <div class="spinner" style="width: 14px; height: 14px;"></div> Installing via {METHOD_LABELS[installMethod]?.label}...
          {:else}
            Install All Missing via {METHOD_LABELS[installMethod]?.label} ({missingDeps.filter(d => d.name !== "homebrew").length})
          {/if}
        </button>
      {/if}

      <div style="margin-top: 20px;">
        <h3 style="font-size: var(--text-sm); color: var(--text-muted); font-weight: 600; margin-bottom: 10px;">Optional Tools</h3>
        <div class="dep-list" style="opacity: 0.85;">
          {#each optionalTools as tool (tool.name)}
            <div class="dep-row">
              <div class="dep-info">
                <div class="dep-icon {tool.installed ? 'installed' : 'missing'}">
                  <Icon name="Package" size={16} />
                </div>
                <div>
                  <div class="dep-name">{tool.label}</div>
                  <div class="dep-desc">{tool.desc}</div>
                </div>
              </div>
              <div class="dep-status">
                {#if tool.version}<span class="dep-version" style="max-width: 180px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;">{tool.version}</span>{/if}
                {#if tool.installed}
                  <span class="badge badge-running">
                    <span class="badge-dot" style="animation: none;"></span>
                    <span>Installed</span>
                  </span>
                {:else if !tool.checking}
                  <span class="badge badge-stopped">
                    <span>Missing</span>
                  </span>
                {:else}
                  <span class="badge" style="background: rgba(139, 148, 158, 0.1); color: var(--text-secondary);">
                    <span class="badge-dot" style="animation: none;"></span>
                    <span>Checking</span>
                  </span>
                {/if}
              </div>
            </div>
          {/each}
        </div>
      </div>

      {#if installMethod === "manual"}
        {#if platform?.os === "windows"}
          <div style="padding: 12px; background: var(--bg-content); border-radius: var(--radius-md); font-size: var(--text-xs); color: var(--text-secondary); margin-bottom: 16px; font-family: var(--font-mono); line-height: 1.8;">
            <div style="color: var(--accent-blue); font-weight: 600; margin-bottom: 4px;">Windows + WSL Setup:</div>
            1. Install WSL: <code>wsl --install -d Ubuntu</code><br/>
            2. Open Ubuntu terminal<br/>
            3. Install Homebrew: <code>/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"</code><br/>
            4. Install deps: <code>brew install colima docker lima</code>
          </div>
        {:else}
          <div style="padding: 12px; background: var(--bg-content); border-radius: var(--radius-md); font-size: var(--text-xs); color: var(--text-secondary); margin-bottom: 16px; font-family: var(--font-mono); line-height: 1.8;">
            <div style="color: var(--accent-blue); font-weight: 600; margin-bottom: 4px;">Manual Installation:</div>
            • Colima: <code>curl -LO https://github.com/abiosoft/colima/releases/latest</code><br/>
            • Docker: <code>https://docs.docker.com/engine/install/</code><br/>
            • Lima: <code>https://lima-vm.io/docs/installation/</code>
          </div>
        {/if}
      {/if}

      <div class="wizard-actions">
        <button class="btn btn-ghost" onclick={() => step = 0}>← Back</button>
        <button class="btn btn-ghost" onclick={() => { checkDeps(); checkOptionalTools(); }} disabled={isInstalling}>↻ Re-check</button>
        <button class="btn btn-primary" onclick={() => step = 2} disabled={isInstalling}>
          {allInstalled ? "Next →" : "Continue Anyway →"}
        </button>
      </div>

    {:else if step === 2}
      <h2 style="font-size: var(--text-xl); font-weight: 700; margin-bottom: 8px;">
        Quick Setup
      </h2>
      <p style="color: var(--text-muted); font-size: var(--text-sm); margin-bottom: 24px;">
        Configure your environment for the best experience.
      </p>

      <div class="wizard-setup-grid">
        <div class="wizard-option">
          <div class="wizard-option-info">
            <span class="wizard-option-label"><Icon name="Rocket" size={14} style="vertical-align: middle; margin-right: 4px;" /> Auto-start Colima on boot</span>
            <span class="wizard-option-desc">
              {#if platform?.os === "macos"}Uses macOS LaunchAgent to start Colima automatically
              {:else if platform?.os === "linux"}{platform?.wsl ? "Starts Colima when WSL boots" : "Uses systemd service to start Colima automatically"}
              {:else if platform?.os === "windows"}Uses Task Scheduler + WSL to start Colima automatically
              {:else}Colima will start automatically when your system restarts
              {/if}
            </span>
          </div>
          <label class="toggle-switch">
            <input type="checkbox" bind:checked={autostart} />
            <span class="toggle-slider"></span>
          </label>
        </div>

        <div class="wizard-option">
          <div class="wizard-option-info">
            <span class="wizard-option-label"><Icon name="Package" size={14} style="vertical-align: middle; margin-right: 4px;" /> Create default instance</span>
            <span class="wizard-option-desc">
              2 CPUs · 4 GB RAM · 60 GB Disk · Docker runtime
            </span>
          </div>
          <label class="toggle-switch">
            <input type="checkbox" bind:checked={createInstance} />
            <span class="toggle-slider"></span>
          </label>
        </div>
      </div>

      {#if setupLog}
        <div style="padding: 12px; background: var(--bg-content); border-radius: var(--radius-md); font-family: var(--font-mono); font-size: var(--text-xs); color: var(--text-secondary); margin-bottom: 16px; white-space: pre-wrap; max-height: 120px; overflow: auto;">
          {setupLog}
        </div>
      {/if}

      <div class="wizard-actions">
        <button class="btn btn-ghost" onclick={() => step = 1} disabled={settingUp}>← Back</button>
        <button class="btn btn-ghost" onclick={() => step = 3} disabled={settingUp}>Skip</button>
        <button
          class="btn btn-primary"
          onclick={handleQuickSetup}
          disabled={settingUp}
          style="padding: 10px 24px;"
        >
          {#if settingUp}
            <div class="spinner" style="width: 14px; height: 14px;"></div> Setting up...
          {:else}
            Apply & Continue →
          {/if}
        </button>
      </div>

    {:else if step === 3}
      <div class="wizard-success-icon">
        <svg width="36" height="36" viewBox="0 0 24 24" fill="none" stroke="var(--accent-green)" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
          <polyline points="20 6 9 17 4 12" />
        </svg>
      </div>
      <div class="wizard-title" style="font-size: var(--text-xl);">You're All Set!</div>
      <div class="wizard-subtitle">
        ColimaUI is ready to use on <strong>{osInfo.label}</strong>. You can manage your
        Colima instances, Docker containers, and much more from the dashboard.
      </div>
      <div class="wizard-actions" style="justify-content: center;">
        <button
          class="btn btn-primary"
          onclick={onComplete}
          style="padding: 12px 40px; font-size: var(--text-base);"
        >
          Enter ColimaUI →
        </button>
      </div>
    {/if}
  </div>
</div>
