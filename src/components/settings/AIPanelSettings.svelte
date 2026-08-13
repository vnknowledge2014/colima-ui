<script lang="ts">
  import { onMount, tick } from "svelte";
  import { uiState } from "../../store.svelte";
  import { aiApi, knowledgeBankApi } from "../../lib/api";
  import { globalToast } from "../../lib/globalToast";
  import { confirm } from "../../store/confirm.svelte";
  import Icon from "../Icon.svelte";
  import { setAppSetting, getAppSetting } from "../../lib/settingsStore.svelte";
  import { normalizeError } from "../../lib/errors";
  import SettingsSection from "./SettingsSection.svelte";
  import ModelPicker from "./ModelPicker.svelte";
  import type { AgentMemoryItem } from "../../lib/api/knowledgeBank";
  import {
    AI_PROVIDERS,
    DEFAULT_PROVIDER,
    activateProvider,
    findProvider,
    getProviderField,
    setProviderField,
  } from "../../lib/aiProviderConfig";

  // The fields seed from the stored provider and are edited independently
  // afterwards, so they read the saved provider rather than the live one.
  const savedProvider = getAppSetting("ai_provider", DEFAULT_PROVIDER);
  let aiProvider = $state(savedProvider);
  let aiModel = $state(getProviderField(savedProvider, "model"));
  let aiApiKey = $state(getProviderField(savedProvider, "api_key"));
  let aiEndpoint = $state(getProviderField(savedProvider, "endpoint"));

  let providerSpec = $derived(findProvider(aiProvider));

  /**
   * Swap the whole credential set, not just the model.
   *
   * Key and endpoint belong to the provider that issued them; carrying them
   * across a switch sends one provider's secret to another and points requests
   * at the wrong host.
   */
  function onProviderChange() {
    activateProvider(aiProvider);
    aiModel = getProviderField(aiProvider, "model");
    aiApiKey = getProviderField(aiProvider, "api_key");
    aiEndpoint = getProviderField(aiProvider, "endpoint");
    availableModels = [];
    modelsError = "";
  }

  function getInitialSearxngInstances() {
    try { 
      return JSON.parse(getAppSetting("ai_searxng_instances", '["http://localhost:8888/search","https://search.inetol.net/search","https://searx.be/search","https://search.brave4u.com/search","https://priv.au/search"]')).join("\n"); 
    } catch { 
      return "http://localhost:8888/search\nhttps://search.inetol.net/search\nhttps://searx.be/search\nhttps://search.brave4u.com/search\nhttps://priv.au/search"; 
    }
  }
  let searxngInstances = $state(getInitialSearxngInstances());
  let contentMode = $state(getAppSetting("ai_diag_content_mode", "full"));
  let maxPageSize = $state(getAppSetting("ai_diag_max_page_size", "8000"));
  let autoTrigger = $state(getAppSetting("ai_diag_auto_trigger") !== "false");
  
  let searxngTesting = $state(false);
  let searxngStatus = $state<"ok" | "fail" | null>(null);
  let searxngError = $state("");
  let availableModels = $state<string[]>([]);
  let modelsFetching = $state(false);
  let modelsError = $state("");

  let apiKeyInput = $state<HTMLInputElement>();
  let aiCardEl = $state<HTMLDivElement>();

  /**
   * Consume the one-shot deep link set by `openSettingsSection("ai")`.
   *
   * An `$effect` rather than `onMount` because Settings is not remounted when
   * the user is already on the page — clicking the AI panel's gear a second
   * time has to scroll again. Clearing the flag re-runs this once with a null
   * value and then settles.
   */
  $effect(() => {
    if (uiState.settingsSection !== "ai") return;
    uiState.settingsSection = null;
    tick().then(() => {
      // `ollama-local` needs no key, so the field is not rendered for it —
      // fall back to the section header rather than silently doing nothing.
      const target = apiKeyInput ?? aiCardEl;
      target?.scrollIntoView({ behavior: "smooth", block: "center" });
      apiKeyInput?.focus();
    });
  });

  // Persist AI settings, each under the provider it belongs to
  $effect(() => { setProviderField(aiProvider, "model", aiModel); });
  $effect(() => { setProviderField(aiProvider, "api_key", aiApiKey); });
  $effect(() => { setProviderField(aiProvider, "endpoint", aiEndpoint); });
  $effect(() => {
    const arr = searxngInstances.split("\n").map((s: string) => s.trim()).filter(Boolean);
    setAppSetting("ai_searxng_instances", JSON.stringify(arr));
  });
  $effect(() => { setAppSetting("ai_diag_content_mode", contentMode); });
  $effect(() => { setAppSetting("ai_diag_max_page_size", maxPageSize); });
  $effect(() => { setAppSetting("ai_diag_auto_trigger", String(autoTrigger)); });

  async function fetchModels() {
    modelsFetching = true;
    modelsError = "";
    try {
      const raw = await aiApi.listModels(aiProvider, aiApiKey, aiEndpoint);
      const parsed: string[] = JSON.parse(typeof raw === "string" ? raw : "[]");
      availableModels = [...new Set(parsed)];
      // An empty list is the shape a rejected credential arrives in, so say so
      // rather than leaving the user staring at a dropdown that never opens.
      if (availableModels.length === 0) {
        modelsError = providerSpec?.needsKey && !aiApiKey
          ? "No models — enter an API key first"
          : "No models returned — check the API key and endpoint";
      }
    } catch (e) {
      availableModels = [];
      modelsError = normalizeError(e).detail;
    } finally {
      modelsFetching = false;
    }
  }

  async function testSearxng() {
    searxngTesting = true;
    searxngStatus = null;
    searxngError = "";
    try {
      const instances = searxngInstances.split("\n").map((s: string) => s.trim()).filter(Boolean);
      const results = await aiApi.search("colima docker", instances.length > 0 ? instances : undefined, 3);
      if (Array.isArray(results) && results.length > 0) {
        const engine = results[0]?.engine || "unknown";
        searxngStatus = "ok";
        searxngError = engine === "duckduckgo" ? "via DuckDuckGo fallback" : `via ${engine}`;
      } else {
        searxngStatus = "fail";
        searxngError = "No results returned";
      }
    } catch (e) {
      searxngStatus = "fail";
      // Branch on the error code, not on the message text: the message is now
      // localized, so substring matching would behave differently per language.
      const err = normalizeError(e);
      const msg = err.detail;
      if (msg.includes("429") || msg.includes("Too Many")) {
        searxngError = "Rate limited (429) — all instances busy";
      } else if (err.code === "network" || msg.toLowerCase().includes("connection refused")) {
        searxngError = "Connection refused — is SearXNG running?";
      } else if (err.code === "timeout") {
        searxngError = "Connection timed out";
      } else {
        searxngError = msg.length > 100 ? msg.slice(0, 100) + "…" : msg;
      }
    } finally {
      searxngTesting = false;
    }
  }

  // Memory state
  let memories = $state<AgentMemoryItem[]>([]);
  let editingMemoryId = $state<string | null>(null);
  let editMemoryContent = $state("");

  async function fetchMemories() {
    try {
      const res: AgentMemoryItem[] = await knowledgeBankApi.getAllMemories();
      memories = res;
    } catch (e) {
      console.error("Failed to load memories:", e);
    }
  }

  async function handleUpdateMemory(id: string) {
    try {
      await knowledgeBankApi.updateMemory(id, editMemoryContent);
      globalToast("success", "Memory updated");
      editingMemoryId = null;
      fetchMemories();
    } catch (e) {
      globalToast("error", `Failed to update memory: ${e}`);
    }
  }

  async function handleDeleteMemory(id: string) {
    const isConfirmed = await confirm({ title: "Delete Memory", message: "Are you sure you want to delete this memory? This action cannot be undone.", confirmText: "Delete", variant: "danger" });
    if (isConfirmed) {
      try {
        await knowledgeBankApi.deleteMemory(id);
        globalToast("success", "Memory deleted");
        fetchMemories();
      } catch (e) {
        globalToast("error", `Failed to delete memory: ${e}`);
      }
    }
  }

  onMount(() => {
    fetchMemories();
  });
</script>

<!--
  Four separate sections rather than one card with hand-drawn dividers: these
  settings are independent of each other, and `SettingsSection` already draws
  the heading and the separation every other settings panel uses.
-->
<SettingsSection
  title="AI Provider"
  icon="Robot"
  description="The model every AI feature talks to. Each provider keeps its own key, endpoint and model — switching back restores them."
  bind:el={aiCardEl}
>
  <!-- Ordered by dependency: the credential has to exist before the model list
       can be fetched, so the key comes before the model field it unlocks. -->
  <div class="form-group">
    <label class="form-label" for="aiProvider">Provider</label>
    <select id="aiProvider" bind:value={aiProvider} onchange={onProviderChange} class="input select">
      {#each AI_PROVIDERS as p (p.id)}
        <option value={p.id}>{p.label}</option>
      {/each}
    </select>
  </div>

  {#if providerSpec?.needsKey}
    <div class="form-group">
      <label class="form-label" for="aiApiKey">API Key</label>
      <input id="aiApiKey" bind:this={apiKeyInput} type="password" bind:value={aiApiKey} placeholder="Enter API key…" class="input mono" />
    </div>
  {/if}

  {#if providerSpec?.needsEndpoint}
    <div class="form-group">
      <label class="form-label" for="aiEndpoint">Endpoint URL</label>
      <input id="aiEndpoint" type="text" bind:value={aiEndpoint} placeholder={providerSpec.endpointPlaceholder ?? ""} class="input mono" />
      {#if providerSpec.presets}
        <div class="preset-row">
          <span class="field-note">Presets:</span>
          {#each providerSpec.presets as preset (preset.label)}
            <button class="preset-chip" onclick={() => (aiEndpoint = preset.endpoint)}>{preset.label}</button>
          {/each}
        </div>
      {/if}
    </div>
  {/if}

  <div class="form-group" style="margin-bottom: 0;">
    <label class="form-label" for="aiModel">Model</label>
    <ModelPicker bind:value={aiModel} models={availableModels} fetching={modelsFetching} onRefresh={fetchModels} />
    {#if modelsError}
      <p class="field-note error">{modelsError}</p>
    {:else if availableModels.length > 0}
      <p class="field-note">{availableModels.length} models available</p>
    {/if}
    {#if providerSpec?.modelHint}
      <p class="field-note">{providerSpec.modelHint}</p>
    {/if}
  </div>
</SettingsSection>

<SettingsSection
  title="Web Search"
  icon="Search"
  description="Search tries your SearXNG instances first, then falls back to DuckDuckGo. Public instances often rate-limit API access."
>
  <div class="settings-inset" style="margin-bottom: 16px;">
    <p class="field-note" style="margin: 0;">
      For reliable results, run a local instance:
      <code>docker run -d -p 8888:8080 searxng/searxng</code>
    </p>
  </div>

  <div class="form-group">
    <label class="form-label" for="searxngInstances">SearXNG instances (one per line)</label>
    <textarea id="searxngInstances" bind:value={searxngInstances} rows="4" placeholder="http://localhost:8888/search&#10;https://search.inetol.net/search" class="input mono instances"></textarea>
  </div>

  <div class="action-row">
    <button class="btn btn-ghost" onclick={testSearxng} disabled={searxngTesting}>
      {#if searxngTesting}
        <span class="spinner"></span> Testing…
      {:else}
        <Icon name="Search" size={14} /> Test web search
      {/if}
    </button>
    {#if searxngStatus === "ok"}
      <span class="status ok">
        <Icon name="Check" size={14} color="var(--accent-green)" /> Connected{searxngError ? ` — ${searxngError}` : ''}
      </span>
    {:else if searxngStatus === "fail"}
      <span class="status fail">
        <Icon name="Error" size={14} color="var(--accent-red)" /> Failed{searxngError ? ` — ${searxngError}` : ''}
      </span>
    {/if}
  </div>
</SettingsSection>

<SettingsSection
  title="Content Processing"
  icon="Bolt"
  description="How much of a fetched page the AI is given to read."
>
  <div class="form-group">
    <label class="form-label" for="contentMode">Content mode</label>
    <select id="contentMode" bind:value={contentMode} class="input select">
      <option value="full">Full — keep images + links</option>
      <option value="compact">Compact — strip images only</option>
      <option value="minimal">Minimal — strip images + links</option>
    </select>
  </div>
  <div class="form-group" style="margin-bottom: 0;">
    <label class="form-label" for="maxPageSize">Max page size (characters)</label>
    <input id="maxPageSize" type="number" bind:value={maxPageSize} min="1000" max="50000" step="1000" class="input mono" />
  </div>
</SettingsSection>

<SettingsSection title="AI Behavior" icon="Gear">
  <label class="toggle-row">
    <input type="checkbox" class="checkbox" bind:checked={autoTrigger} />
    <span>Auto-trigger on errors</span>
  </label>
  <p class="field-note indented">
    Any application error opens the AI diagnostics bubble and starts investigating on its own.
  </p>
</SettingsSection>

<!-- AI Knowledge & Memory -->
<SettingsSection
  title="AI Knowledge & Memory"
  icon="Robot"
  description="The AI learns from your interactions to provide better context. Memories are atomic and can be safely edited or deleted without breaking context."
>
  <div style="display: flex; flex-direction: column; gap: 12px;">
    {#if memories.length === 0}
      <div class="settings-inset" style="font-size: var(--text-sm); color: var(--text-muted); text-align: center;">
        No memories recorded yet.
      </div>
    {:else}
      {#each memories as memory (memory.id)}
        <div class="settings-inset" style="position: relative;">
          <div style="display: flex; justify-content: space-between; align-items: flex-start; margin-bottom: 12px;">
            <div style="display: flex; align-items: center; gap: 8px;">
              <!-- Tint derived from the accent itself, so the badge cannot drift
                   to a blue the palette does not contain. -->
              <span class="badge" style="background: color-mix(in srgb, {memory.memory_type === 'reasoning' ? 'var(--accent-blue)' : 'var(--accent-purple)'} 12%, transparent); color: {memory.memory_type === 'reasoning' ? 'var(--accent-blue)' : 'var(--accent-purple)'}; text-transform: uppercase;">
                {memory.memory_type === 'reasoning' ? '🛠️ Reasoning' : '👤 Preference'}
              </span>
              <span style="font-size: 11px; color: var(--text-muted);">
                {new Date(memory.created_at / 1000000).toLocaleString()}
              </span>
            </div>
            <div style="display: flex; gap: 4px;">
              <button class="btn btn-ghost" style="padding: 4px 8px;" onclick={() => { editingMemoryId = memory.id; editMemoryContent = memory.content; }}>
                <Icon name="Edit" size={14} />
              </button>
              <button class="btn btn-ghost" style="padding: 4px 8px; color: var(--accent-red);" onclick={() => handleDeleteMemory(memory.id)}>
                <Icon name="Trash" size={14} />
              </button>
            </div>
          </div>

          {#if editingMemoryId === memory.id}
            <div>
              <textarea
                bind:value={editMemoryContent}
                class="input"
                rows="4"
                style="font-family: var(--font-mono); font-size: 13px; resize: vertical; width: 100%; margin-bottom: 8px;"
              ></textarea>
              <div style="display: flex; gap: 8px; justify-content: flex-end;">
                <button class="btn btn-ghost" onclick={() => editingMemoryId = null}>Cancel</button>
                <button class="btn btn-primary" onclick={() => handleUpdateMemory(memory.id)}>Save</button>
              </div>
            </div>
          {:else}
            <div style="font-size: var(--text-sm); line-height: 1.5; white-space: pre-wrap; color: var(--text-primary);">
              {memory.content}
            </div>
          {/if}
        </div>
      {/each}
    {/if}
  </div>
</SettingsSection>

<style>
  /* Identifiers the user copies or compares character by character — keys,
     URLs, byte counts — read in the mono face. */
  .mono {
    font-family: var(--font-mono);
  }

  .instances {
    resize: vertical;
    line-height: 1.5;
  }

  .field-note {
    margin: 0;
    font-size: var(--text-xs);
    color: var(--text-muted);
    line-height: 1.5;
  }

  .field-note.error {
    color: var(--accent-red);
  }

  /* Lines up with the label beside the checkbox, not the checkbox itself. */
  .field-note.indented {
    margin-top: 4px;
    padding-left: 24px;
  }

  .field-note code {
    font-size: var(--text-xs);
    background: var(--bg-secondary);
    padding: 1px 5px;
    border-radius: var(--radius-sm);
  }

  .preset-row {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-wrap: wrap;
    margin-top: 2px;
  }

  .preset-chip {
    padding: 2px 8px;
    font-size: var(--text-xs);
    color: var(--text-secondary);
    background: var(--surface-inset);
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-sm);
    cursor: pointer;
  }

  .preset-chip:hover {
    color: var(--text-primary);
    border-color: var(--border-primary);
  }

  .action-row {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
  }

  .status {
    display: flex;
    align-items: center;
    gap: 5px;
    font-size: var(--text-xs);
  }

  .status.ok {
    color: var(--accent-green);
  }

  .status.fail {
    color: var(--accent-red);
  }

  .toggle-row {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: var(--text-sm);
    cursor: pointer;
  }

  /* The shared spinner is sized for a page; inside a button it has to match
     the icon it replaces. */
  .btn .spinner {
    width: 12px;
    height: 12px;
    border-width: 1.5px;
  }
</style>
