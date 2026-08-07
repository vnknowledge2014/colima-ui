<script lang="ts">
  import { onMount } from "svelte";
  import { modelsApi, colimaApi, type AiModel, type ColimaInstance } from "../lib/api";
  import { globalToast } from "../lib/globalToast";
  import Icon from "../components/Icon.svelte";
  import { t } from "../lib/i18n.svelte";

  let models = $state<AiModel[]>([]);
  let instances = $state<ColimaInstance[]>([]);
  let selectedProfile = $state("default");
  let selectedRunner = $state("");
  let loading = $state(true);
  let error = $state<string | null>(null);
  let actionLoading = $state<string | null>(null);
  let showPull = $state(false);
  let pullName = $state("");

  async function fetchModels() {
    try {
      error = null;
      loading = true;
      const list = await modelsApi.listModels(selectedProfile, selectedRunner);
      models = list;
    } catch (e) {
      error = String(e);
      models = [];
    } finally {
      loading = false;
    }
  }

  async function fetchInstances() {
    try {
      const list = await colimaApi.listInstances();
      const running = list.filter((i) => i.status === "Running");
      instances = running;
      if (running.length > 0 && !running.find((i) => {
        const p = i.name === "colima" ? "default" : i.name.replace("colima-", "");
        return p === selectedProfile;
      })) {
        const firstName = running[0].name;
        selectedProfile = firstName === "colima" ? "default" : firstName.replace("colima-", "");
      }
    } catch (_) { /* ignore */ }
  }

  onMount(() => {
    fetchInstances().then(() => {
      fetchModels();
    });
  });

  $effect(() => {
    // Re-fetch models when selected profile or runner changes, if not initially loading
    // We add a tiny delay to debounce if both change rapidly
    const t = setTimeout(() => fetchModels(), 50);
    return () => clearTimeout(t);
  });

  async function handlePull() {
    if (!pullName.trim()) return;
    const name = pullName.trim();
    // Fire-and-forget: close dialog, long operation runs in background
    globalToast("success", t('models.pulling', { default: `Pulling model '${name}'... This may take a while.` }));
    pullName = "";
    showPull = false;
    modelsApi.pullModel(selectedProfile, name, selectedRunner)
      .then(() => { globalToast("success", t('models.pulled', { default: `Model '${name}' pulled successfully` })); fetchModels(); })
      .catch((e) => globalToast("error", t('models.pull_failed', { default: `Pull failed: ${e}` })));
  }

  async function handleDelete(name: string) {
    actionLoading = `${name}-delete`;
    try {
      await modelsApi.deleteModel(selectedProfile, name, selectedRunner);
      globalToast("success", t('models.deleted', { default: `Model '${name}' deleted` }));
      fetchModels();
    } catch (e) {
      globalToast("error", String(e));
    } finally {
      actionLoading = null;
    }
  }

  async function handleServe(name: string) {
    actionLoading = `${name}-serve`;
    try {
      await modelsApi.serveModel(selectedProfile, name, 11434, selectedRunner);
      globalToast("success", t('models.serving', { default: `Model '${name}' serving on port 11434` }));
    } catch (e) {
      globalToast("error", String(e));
    } finally {
      actionLoading = null;
    }
  }

  const dockerModels = [
    { name: "ai/smollm2", desc: "Small Language Model 2", size: "~1.7 GB" },
    { name: "ai/gemma3", desc: "Google Gemma 3", size: "~5.4 GB" },
    { name: "ai/llama3.2", desc: "Meta Llama 3.2", size: "~2.0 GB" },
    { name: "ai/phi4-mini", desc: "Microsoft Phi-4 Mini", size: "~2.4 GB" },
    { name: "ai/deepseek-r1", desc: "DeepSeek R1 (distill)", size: "~4.7 GB" },
    { name: "ai/mistral-small", desc: "Mistral Small 3.1", size: "~15 GB" },
  ];

  const ramalamaModels = [
    { name: "llama3.3", desc: "Meta's latest Llama 3.3", size: "~4.7 GB" },
    { name: "gemma2", desc: "Google Gemma 2", size: "~5.4 GB" },
    { name: "qwen2.5", desc: "Alibaba Qwen 2.5", size: "~4.7 GB" },
    { name: "phi4", desc: "Microsoft Phi-4", size: "~9.1 GB" },
    { name: "deepseek-r1", desc: "DeepSeek R1", size: "~4.7 GB" },
    { name: "mistral", desc: "Mistral 7B", size: "~4.1 GB" },
  ];

  let popularModels = $derived(selectedRunner === "ramalama" ? ramalamaModels : dockerModels);
</script>

<div class="content-header">
  <h1>
    {t('models.title', { default: 'AI Models' })}
    <span style="font-size: var(--text-sm); color: var(--text-muted); font-weight: 400; margin-left: 12px;">
      {models.length} {t('models.count', { default: models.length !== 1 ? "models" : "model" })}
    </span>
  </h1>
  <div class="content-header-actions">
    {#if instances.length > 1}
      <select
        class="input select"
        style="width: 160px;"
        bind:value={selectedProfile}
      >
        {#each instances as inst}
          {@const p = inst.name === "colima" ? "default" : inst.name.replace("colima-", "")}
          <option value={p}>{inst.name}</option>
        {/each}
      </select>
    {/if}
    <select
      class="input select"
      style="width: 180px;"
      bind:value={selectedRunner}
    >
      <option value="">{t('models.docker_runner', { default: 'Docker Model Runner' })}</option>
      <option value="ramalama">{t('models.ramalama_runner', { default: 'Ramalama' })}</option>
    </select>
    <button class="btn btn-ghost" onclick={fetchModels} aria-label={t('models.refresh', { default: 'Refresh Models' })} title={t('models.refresh', { default: 'Refresh Models' })}>
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <path d="M21.5 2v6h-6M2.5 22v-6h6M2 11.5a10 10 0 0 1 18.8-4.3M22 12.5a10 10 0 0 1-18.8 4.3"/>
      </svg>
    </button>
    <button class="btn btn-primary" onclick={() => showPull = true}>
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/>
      </svg>
      {t('models.pull', { default: 'Pull Model' })}
    </button>
  </div>
</div>

<div class="content-body">
  {#if error}
    <div class="card" style="border-color: var(--accent-yellow); margin-bottom: 16px;">
      <p style="color: var(--accent-yellow); font-size: var(--text-sm);">
        <Icon name="Warning" size={14} style="vertical-align: middle; margin-right: 4px;" /> {t('models.error_support', { default: 'AI model support not available' })}
      </p>
      <p style="color: var(--text-muted); font-size: var(--text-xs); margin-top: 4px;">
        {#if error.includes("krunkit") || error.includes("vm-type")}
          {t('models.error_krunkit', { default: 'GPU support requires krunkit. Install it first:' })}
          <code style="display: block; margin: 8px 0; padding: 6px 10px; background: var(--bg-primary); border-radius: 6px; font-family: var(--font-mono);">
            brew tap slp/krunkit && brew install krunkit
          </code>
          {t('models.error_restart', { default: 'Then restart Colima:' })}
          <code style="display: block; margin: 8px 0; padding: 6px 10px; background: var(--bg-primary); border-radius: 6px; font-family: var(--font-mono);">
            colima start --runtime docker --vm-type krunkit
          </code>
        {:else if error.includes("not installed")}
          {t('models.error_tools', { default: 'Required tools are not installed. Make sure Colima is available.' })}
        {:else}
          {t('models.error_default', { default: 'Model management requires Colima started with krunkit VM type for GPU access.' })}
        {/if}
      </p>
    </div>
  {/if}

  {#if loading}
    <div class="loading-screen"><div class="spinner"></div><span>{t('models.loading', { default: 'Loading models...' })}</span></div>
  {:else if models.length > 0}
    <div style="display: grid; grid-template-columns: repeat(auto-fill, minmax(320px, 1fr)); gap: 12px;">
      {#each models as model}
        {@const isLoading = actionLoading?.startsWith(model.name)}
        <div class="card" style="opacity: {isLoading ? 0.6 : 1};">
          <div style="display: flex; justify-content: space-between; align-items: flex-start; margin-bottom: 12px;">
            <div>
              <div style="font-weight: 600; font-size: var(--text-base);">{model.name}</div>
              <div style="font-size: var(--text-xs); color: var(--text-muted); margin-top: 2px;">
                {#if model.family}<span>{model.family} · </span>{/if}
                {#if model.parameters}<span>{model.parameters} · </span>{/if}
                {model.size}
              </div>
            </div>
            <div style="display: flex; gap: 4px;">
              <button class="btn btn-ghost btn-icon" title={t('models.serve', { default: 'Serve' })} disabled={!!isLoading} onclick={() => handleServe(model.name)}>
                <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor"><polygon points="6,4 20,12 6,20"/></svg>
              </button>
              <button class="btn btn-ghost btn-icon" title={t('models.delete', { default: 'Delete' })} disabled={!!isLoading} onclick={() => handleDelete(model.name)} style="color: var(--accent-red);">
                <Icon name="Trash" size={14} />
              </button>
            </div>
          </div>
          <div style="display: flex; gap: 8px; flex-wrap: wrap;">
            {#if model.format}
              <span style="padding: 2px 8px; border-radius: var(--radius-sm); background: rgba(88, 166, 255, 0.1); color: var(--accent-blue); font-size: var(--text-xs);">
                {model.format}
              </span>
            {/if}
            {#if model.quantization}
              <span style="padding: 2px 8px; border-radius: var(--radius-sm); background: rgba(188, 140, 255, 0.1); color: var(--accent-purple); font-size: var(--text-xs);">
                Q{model.quantization}
              </span>
            {/if}
          </div>
        </div>
      {/each}
    </div>
  {:else}
    <div class="empty-state">
      <div class="empty-state-icon">
        <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" style="color: var(--text-muted);">
          <path d="M12 2L2 7l10 5 10-5-10-5z"/><path d="M2 17l10 5 10-5"/><path d="M2 12l10 5 10-5"/>
        </svg>
      </div>
      <div class="empty-state-title">
        {instances.length === 0 ? t('models.colima_not_running', { default: 'Colima is not running' }) : t('models.no_models', { default: 'No models installed' })}
      </div>
      <div class="empty-state-text">
        {instances.length === 0 ? t('models.start_colima', { default: 'Start a Colima instance first to manage AI models.' }) : t('models.pull_to_start', { default: 'Pull a model to get started with AI inference.' })}
      </div>

      <!-- Popular Models Quick-Add -->
      <div style="width: 100%; max-width: 500px; margin-top: 16px;">
        <div style="font-size: var(--text-xs); color: var(--text-muted); margin-bottom: 8px; text-align: left;">{t('models.popular', { default: 'Popular Models' })}</div>
        <div style="display: grid; grid-template-columns: 1fr 1fr; gap: 8px;">
          {#each popularModels as m}
            <div
              class="card"
              style="cursor: pointer; padding: 12px; transition: border-color 200ms;"
              onclick={() => { pullName = m.name; showPull = true; }}
            >
              <div style="font-weight: 500; font-size: var(--text-sm);">{m.name}</div>
              <div style="font-size: var(--text-xs); color: var(--text-muted);">{m.desc}</div>
              <div style="font-size: var(--text-xs); color: var(--accent-blue); margin-top: 4px;">{m.size}</div>
            </div>
          {/each}
        </div>
      </div>
    </div>
  {/if}
</div>

<!-- Pull Model Dialog -->
{#if showPull}
  <div role="button" tabindex="0" class="modal-overlay" onkeydown={(e) => e.key === 'Enter' && e.currentTarget.click()} onclick={() => showPull = false}>
    <div role="button" tabindex="0" class="modal" onkeydown={(e) => e.key === 'Enter' && e.currentTarget.click()} onclick={(e) => e.stopPropagation()} style="width: min(450px, 90vw);">
      <div class="modal-header">
        <h2 class="modal-title">Pull Model</h2>
        <button class="btn btn-icon btn-ghost" onclick={() => showPull = false}><Icon name="Close" size={16} /></button>
      </div>
      <div class="form-group">
        <label for="modelName" class="form-label">Model Name</label>
        <input
          id="modelName"
          class="input"
          placeholder="e.g. llama3.3, gemma2, phi4:14b"
          bind:value={pullName}
          onkeydown={(e) => e.key === "Enter" && handlePull()}
          autofocus
        />
        <p style="font-size: var(--text-xs); color: var(--text-muted); margin-top: 4px;">
          Use model name from Ollama registry. Append :tag for specific variants.
        </p>
      </div>
      <div class="modal-footer">
        <button class="btn btn-ghost" onclick={() => showPull = false}>Cancel</button>
        <button class="btn btn-primary" onclick={handlePull} disabled={!pullName.trim()}>
          Pull
        </button>
      </div>
    </div>
  </div>
{/if}
