<script lang="ts">
  import { onMount } from "svelte";
  import { type SystemInfo, dockerApi, aiApi, knowledgeBankApi, sysMethods } from "../lib/api";
  import { globalToast } from "../lib/globalToast";
  import { confirm } from "../store/confirm.svelte";
  import Icon from "../components/Icon.svelte";
  import { setAppSetting, getAppSetting } from "../lib/settingsStore.svelte";
  import { t, setLanguage, getLanguage } from "../lib/i18n.svelte";

  export interface AgentMemoryItem {
    id: string;
    memory_type: string;
    content: string;
    created_at: number;
  }

  const AI_PROVIDERS = [
    { id: "anthropic", label: "Anthropic" },
    { id: "openai", label: "OpenAI" },
    { id: "gemini", label: "Google Gemini" },
    { id: "ollama-local", label: "Ollama Local" },
    { id: "ollama-cloud", label: "Ollama Cloud" },
  ];

  interface DiskUsage {
    type: string;
    total: string;
    active: string;
    size: string;
    reclaimable: string;
  }

  let { systemInfo } = $props<{ systemInfo: SystemInfo | null }>();

  let diskUsage = $state<DiskUsage[]>([]);
  let pruning = $state(false);

  // AI & Diagnostics state
  let aiProvider = $state(getAppSetting("ai_provider", "anthropic"));
  let aiModel = $state(getAppSetting("ai_model", ""));
  let aiApiKey = $state(getAppSetting("ai_api_key", ""));
  let aiEndpoint = $state(getAppSetting("ai_endpoint", ""));
  
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

  // Resource Saver Mode state
  let autoPauseEnabled = $state(getAppSetting("colimaui_auto_pause") === "true");
  let autoPauseMinutes = $state(parseInt(getAppSetting("colimaui_auto_pause_mins") || "15", 10));

  $effect(() => {
    setAppSetting("colimaui_auto_pause", String(autoPauseEnabled));
    setAppSetting("colimaui_auto_pause_mins", String(autoPauseMinutes));
    sysMethods.setResourceSaver(autoPauseEnabled, autoPauseMinutes).catch(() => {});
  });

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
    fetchDiskUsage();
  });

  // Persist AI settings
  $effect(() => { setAppSetting("ai_provider", aiProvider); });
  $effect(() => { setAppSetting("ai_model", aiModel); });
  $effect(() => { setAppSetting("ai_api_key", aiApiKey); });
  $effect(() => { setAppSetting("ai_endpoint", aiEndpoint); });
  $effect(() => {
    const arr = searxngInstances.split("\n").map((s: string) => s.trim()).filter(Boolean);
    setAppSetting("ai_searxng_instances", JSON.stringify(arr));
  });
  $effect(() => { setAppSetting("ai_diag_content_mode", contentMode); });
  $effect(() => { setAppSetting("ai_diag_max_page_size", maxPageSize); });
  $effect(() => { setAppSetting("ai_diag_auto_trigger", String(autoTrigger)); });

  async function fetchModels() {
    modelsFetching = true;
    try {
      const raw = await aiApi.listModels(aiProvider, aiApiKey, aiEndpoint);
      const parsed: string[] = JSON.parse(typeof raw === "string" ? raw : "[]");
      availableModels = [...new Set(parsed)];
    } catch {
      availableModels = [];
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
      const msg = String(e);
      if (msg.includes("429") || msg.includes("Too Many")) {
        searxngError = "Rate limited (429) — all instances busy";
      } else if (msg.includes("Connection refused") || msg.includes("connection refused")) {
        searxngError = "Connection refused — is SearXNG running?";
      } else if (msg.includes("timeout")) {
        searxngError = "Connection timed out";
      } else {
        searxngError = msg.length > 100 ? msg.slice(0, 100) + "…" : msg;
      }
    } finally {
      searxngTesting = false;
    }
  }

  async function fetchDiskUsage() {
    try {
      const raw = await dockerApi.systemDf();
      if (!raw) return;
      const text = typeof raw === 'string' ? raw : String(raw);
      const lines = text.split("\n").filter(l => l.trim());
      const rows: DiskUsage[] = [];
      for (const line of lines) {
        if (line.startsWith("TYPE") || line.startsWith("---")) continue;
        const parts = line.split(/\s{2,}/);
        if (parts.length >= 4) {
          rows.push({
            type: parts[0],
            total: parts[1],
            active: parts[2],
            size: parts[3],
            reclaimable: parts[4] || "0B",
          });
        }
      }
      diskUsage = rows;
    } catch { /* ignore */ }
  }

  async function handlePrune() {
    const ok = await confirm({ title: "System Prune", message: "Remove all unused Docker data (stopped containers, unused networks, dangling images, build cache)?", confirmText: "Prune All", variant: "warning" });
    if (!ok) return;
    pruning = true;
    try {
      await dockerApi.systemPrune();
      globalToast("success", "System pruned successfully");
      fetchDiskUsage();
    } catch (e) {
      globalToast("error", String(e));
    } finally {
      pruning = false;
    }
  }

  const deps = $derived([
    { name: "Colima", desc: "Container runtime manager", installed: systemInfo?.colima_installed, version: systemInfo?.colima_version },
    { name: "Docker", desc: "Container engine client", installed: systemInfo?.docker_installed, version: systemInfo?.docker_version },
    { name: "Lima", desc: "Linux virtual machine manager", installed: systemInfo?.lima_installed, version: systemInfo?.lima_version },
  ]);
</script>

<div class="page">
  <div class="page-header" style="justify-content: space-between;">
    <div>
      <h1 class="page-title">{t('settings.title', { default: 'Settings' })}</h1>
      <div class="page-subtitle">Configure ColimaUI, AI behavior, and resources</div>
    </div>
  </div>
  
  <div class="page-content" style="max-width: 800px; padding-bottom: 60px;">
  
    <!-- Appearance Settings -->
    <div class="card" style="margin-bottom: 24px; padding: 0;">
      <div style="padding: 16px 20px; border-bottom: 1px solid var(--border-primary); font-weight: 600; font-size: var(--text-lg); display: flex; align-items: center; gap: 8px;">
        <Icon name="Settings" size={18} />
        {t('settings.appearance', { default: 'Appearance' })}
      </div>
      <div style="padding: 24px 20px;">
        <div style="display: flex; flex-direction: column; gap: 16px;">
          <div style="border-top: 1px solid var(--border-subtle); padding-top: 16px; display: flex; justify-content: space-between; align-items: center;">
            <div>
              <div style="font-weight: 500;">{t('settings.language', { default: 'Language' })}</div>
              <div style="font-size: var(--text-sm); color: var(--text-secondary); margin-top: 2px;">{t('settings.language_desc', { default: 'Change the application language' })}</div>
            </div>
            <select class="input" style="width: 200px;" value={getLanguage()} onchange={(e) => {
              setLanguage(e.currentTarget.value);
            }}>
              <option value="en">English</option>
              <option value="vi">Tiếng Việt</option>
              <option value="zh">中文</option>
              <option value="ja">日本語</option>
            </select>
          </div>
        </div>
      </div>
    </div>

  <!-- System Dependencies -->
  <div class="card" style="margin-bottom: 24px;">
    <h3 style="font-size: var(--text-lg); font-weight: 600; margin-bottom: 20px;">System Dependencies</h3>
    <div style="display: flex; flex-direction: column; gap: 0;">
      {#each deps as dep, i}
        <div style="display: flex; justify-content: space-between; align-items: center; padding: 12px 0; border-bottom: {i < deps.length - 1 ? '1px solid var(--border-subtle)' : 'none'};">
          <div>
            <div style="font-weight: 500;">{dep.name}</div>
            <div style="font-size: var(--text-xs); color: var(--text-muted);">{dep.desc}</div>
          </div>
          <div style="text-align: right;">
            <span class="badge {dep.installed ? 'badge-running' : 'badge-stopped'}">
              {dep.installed ? "Installed" : "Not Found"}
            </span>
            {#if dep.version}
              <div style="font-size: var(--text-xs); color: var(--text-muted); font-family: var(--font-mono); margin-top: 4px;">
                {dep.version.split("\n")[0]}
              </div>
            {/if}
          </div>
        </div>
      {/each}
    </div>
  </div>

  <!-- Resource Saver Mode -->
  <div class="card" style="margin-bottom: 24px;">
    <h3 style="font-size: var(--text-lg); font-weight: 600; margin-bottom: 8px;">Resource Saver Mode</h3>
    <p style="font-size: var(--text-sm); color: var(--text-secondary); margin-bottom: 20px;">
      Automatically pause the Colima instance if there are no active CPU spikes or running containers for a set period.
    </p>
    <div style="display: flex; flex-direction: column; gap: 16px;">
      <div style="display: flex; align-items: center; justify-content: space-between;">
        <label style="font-size: var(--text-sm); font-weight: 500; cursor: pointer; display: flex; align-items: center; gap: 8px;">
          <input type="checkbox" class="checkbox" bind:checked={autoPauseEnabled} />
          <span>Enable Auto-Pause</span>
        </label>
      </div>
      
      {#if autoPauseEnabled}
      <div style="display: flex; align-items: center; gap: 12px; background: var(--bg-secondary); padding: 12px; border-radius: 6px;">
        <label for="idleThreshold" style="font-size: var(--text-sm); color: var(--text-secondary);">Idle threshold (minutes)</label>
        <input id="idleThreshold" type="number" min="1" max="1440" bind:value={autoPauseMinutes} 
               style="width: 80px; padding: 4px 8px; border: 1px solid var(--border-primary); border-radius: 4px; background: var(--bg-primary); color: var(--text-primary); font-size: var(--text-sm);" />
      </div>
      {/if}
    </div>
  </div>

  <!-- Docker Disk Usage -->
  {#if diskUsage.length > 0}
    <div class="card" style="margin-bottom: 24px;">
      <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 16px;">
        <h3 style="font-size: var(--text-lg); font-weight: 600; margin: 0;">Docker Disk Usage</h3>
        <button class="btn btn-ghost" style="color: var(--accent-red); font-size: var(--text-xs);"
          disabled={pruning} onclick={handlePrune}>
          {#if pruning}
            Pruning...
          {:else}
            <Icon name="Broom" size={12} style="vertical-align: middle;" /> System Prune
          {/if}
        </button>
      </div>
      <table class="data-table">
        <thead>
          <tr><th>Type</th><th>Total</th><th>Active</th><th>Size</th><th>Reclaimable</th></tr>
        </thead>
        <tbody>
          {#each diskUsage as row}
            <tr>
              <td style="font-weight: 500; font-size: var(--text-sm);">{row.type}</td>
              <td style="font-family: var(--font-mono); font-size: var(--text-xs);">{row.total}</td>
              <td style="font-family: var(--font-mono); font-size: var(--text-xs);">{row.active}</td>
              <td style="font-family: var(--font-mono); font-size: var(--text-xs); color: var(--accent-yellow);">{row.size}</td>
              <td style="font-family: var(--font-mono); font-size: var(--text-xs); color: var(--accent-green);">{row.reclaimable}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}

  <!-- AI & Diagnostics -->
  <div class="card" style="margin-bottom: 24px;">
    <h3 style="font-size: var(--text-lg); font-weight: 600; margin-bottom: 20px; display: flex; align-items: center; gap: 8px;">
      <Icon name="Robot" size={18} /> AI & Diagnostics
    </h3>

    <!-- AI Provider -->
    <div style="margin-bottom: 16px;">
      <div style="font-size: var(--text-xs); font-weight: 600; color: var(--text-secondary); margin-bottom: 10px; text-transform: uppercase; letter-spacing: 0.05em; display: flex; align-items: center; gap: 6px;">
        <Icon name="Gear" size={12} /> AI Provider
      </div>
      <div style="display: flex; gap: 8px; margin-bottom: 8px;">
        <div style="flex: 1;">
          <label for="aiProvider" style="font-size: 11px; color: var(--text-muted); display: block; margin-bottom: 4px;">Provider</label>
          <select id="aiProvider" bind:value={aiProvider} onchange={() => aiModel = ""} class="settings-select">
            {#each AI_PROVIDERS as p}
              <option value={p.id}>{p.label}</option>
            {/each}
          </select>
        </div>
        <div style="flex: 1;">
          <label for="aiModel" style="font-size: 11px; color: var(--text-muted); display: block; margin-bottom: 4px;">
            Model {#if modelsFetching}<span class="spinner" style="width: 10px; height: 10px; border-width: 1.5px; display: inline-block; vertical-align: middle; margin-left: 4px;"></span>{/if}
          </label>
          <input id="aiModel" type="text" list="settings-ai-models" bind:value={aiModel} placeholder="Type or select..." class="settings-input" />
          <datalist id="settings-ai-models">
            {#each availableModels as m}
              <option value={m}></option>
            {/each}
          </datalist>
          <button class="btn btn-ghost" style="font-size: 10px; padding: 2px 6px; margin-top: 4px; display: flex; align-items: center; gap: 3px;"
            onclick={fetchModels} disabled={modelsFetching}>
            <Icon name="Refresh" size={10} /> {modelsFetching ? "Fetching..." : "Refresh models"}
          </button>
        </div>
      </div>
      {#if aiProvider !== "ollama-local"}
        <div style="margin-bottom: 8px;">
          <label for="aiApiKey" style="font-size: 11px; color: var(--text-muted); display: block; margin-bottom: 4px;">API Key</label>
          <input id="aiApiKey" type="password" bind:value={aiApiKey} placeholder="Enter API key..." class="settings-input" style="font-family: var(--font-mono);" />
        </div>
      {/if}
      {#if aiProvider === "ollama-cloud"}
        <div>
          <label for="aiEndpoint" style="font-size: 11px; color: var(--text-muted); display: block; margin-bottom: 4px;">Endpoint URL</label>
          <input id="aiEndpoint" type="text" bind:value={aiEndpoint} placeholder="https://your-ollama-server.com" class="settings-input" style="font-family: var(--font-mono);" />
        </div>
      {/if}
    </div>

    <div style="border-top: 1px solid var(--border-subtle); margin: 0 0 16px;"></div>

    <!-- Web Search -->
    <div style="margin-bottom: 16px;">
      <div style="font-size: var(--text-xs); font-weight: 600; color: var(--text-secondary); margin-bottom: 10px; text-transform: uppercase; letter-spacing: 0.05em; display: flex; align-items: center; gap: 6px;">
        <Icon name="Search" size={12} /> Web Search
      </div>
      <div style="font-size: var(--text-xs); color: var(--text-muted); margin-bottom: 10px; line-height: 1.6; padding: 8px 10px; background: rgba(88,166,255,0.06); border-radius: var(--radius-md); border: 1px solid rgba(88,166,255,0.1);">
        Search uses SearXNG instances first, then DuckDuckGo as fallback.
        Public SearXNG instances may rate-limit API access.
        For reliable results, run a local instance: <code style="font-size: 10px; background: rgba(255,255,255,0.06); padding: 1px 4px; border-radius: 3px;">docker run -d -p 8888:8080 searxng/searxng</code>
      </div>
      <div style="margin-bottom: 8px;">
        <label for="searxngInstances" style="font-size: 11px; color: var(--text-muted); display: block; margin-bottom: 4px;">SearXNG Instances (one per line)</label>
        <textarea id="searxngInstances" bind:value={searxngInstances} rows="4" placeholder="http://localhost:8888/search&#10;https://search.inetol.net/search" class="settings-input" style="font-family: var(--font-mono); resize: vertical; line-height: 1.5;"></textarea>
      </div>
      <div style="display: flex; align-items: center; gap: 8px; flex-wrap: wrap;">
        <button class="btn btn-ghost" style="font-size: var(--text-xs); display: flex; align-items: center; gap: 4px;"
          onclick={testSearxng} disabled={searxngTesting}>
          {#if searxngTesting}
            <span class="spinner" style="width: 10px; height: 10px; border-width: 1.5px;"></span> Testing...
          {:else}
            <Icon name="Search" size={12} /> Test Web Search
          {/if}
        </button>
        {#if searxngStatus === "ok"}
          <span style="color: var(--accent-green); font-size: var(--text-xs); display: flex; align-items: center; gap: 4px;">
            <Icon name="Check" size={12} color="var(--accent-green)" /> Connected{searxngError ? ` — ${searxngError}` : ''}
          </span>
        {/if}
        {#if searxngStatus === "fail"}
          <span style="color: var(--accent-red); font-size: var(--text-xs); display: flex; align-items: center; gap: 4px;">
            <Icon name="Error" size={12} color="var(--accent-red)" /> Failed{searxngError ? ` — ${searxngError}` : ''}
          </span>
        {/if}
      </div>
    </div>

    <div style="border-top: 1px solid var(--border-subtle); margin: 0 0 16px;"></div>

    <!-- Content Processing -->
    <div style="margin-bottom: 16px;">
      <div style="font-size: var(--text-xs); font-weight: 600; color: var(--text-secondary); margin-bottom: 10px; text-transform: uppercase; letter-spacing: 0.05em; display: flex; align-items: center; gap: 6px;">
        <Icon name="Bolt" size={12} /> Content Processing
      </div>
      <div style="display: flex; gap: 8px;">
        <div style="flex: 1;">
          <label for="contentMode" style="font-size: 11px; color: var(--text-muted); display: block; margin-bottom: 4px;">Content Mode</label>
          <select id="contentMode" bind:value={contentMode} class="settings-select">
            <option value="full">Full — Keep images + links</option>
            <option value="compact">Compact — Strip images only</option>
            <option value="minimal">Minimal — Strip images + links</option>
          </select>
        </div>
        <div style="flex: 1;">
          <label for="maxPageSize" style="font-size: 11px; color: var(--text-muted); display: block; margin-bottom: 4px;">Max Page Size (chars)</label>
          <input id="maxPageSize" type="number" bind:value={maxPageSize} min="1000" max="50000" step="1000" class="settings-input" style="font-family: var(--font-mono);" />
        </div>
      </div>
    </div>

    <div style="border-top: 1px solid var(--border-subtle); margin: 0 0 16px;"></div>

    <!-- Behavior -->
    <div>
      <div style="font-size: var(--text-xs); font-weight: 600; color: var(--text-secondary); margin-bottom: 10px; text-transform: uppercase; letter-spacing: 0.05em;">
        Behavior
      </div>
      <label style="display: flex; align-items: center; gap: 8px; cursor: pointer; font-size: var(--text-sm);">
        <input type="checkbox" class="checkbox" bind:checked={autoTrigger} />
        <span>Auto-trigger on errors</span>
      </label>
      <div style="font-size: var(--text-xs); color: var(--text-muted); margin-top: 4px; margin-left: 24px;">
        When enabled, any application error automatically opens the AI diagnostics bubble and starts investigation.
      </div>
    </div>
  </div>

  <!-- AI Knowledge & Memory -->
  <div class="card" style="margin-bottom: 24px;">
    <h3 style="font-size: var(--text-lg); font-weight: 600; margin-bottom: 20px; display: flex; align-items: center; gap: 8px;">
      <Icon name="Robot" size={18} /> AI Knowledge & Memory
    </h3>
    <p style="font-size: var(--text-sm); color: var(--text-secondary); margin-bottom: 16px;">
      The AI learns from your interactions to provide better context. Memories are atomic and can be safely edited or deleted without breaking context.
    </p>

    <div style="display: flex; flex-direction: column; gap: 12px;">
      {#if memories.length === 0}
        <div style="font-size: var(--text-sm); color: var(--text-muted); padding: 16px; background: rgba(255,255,255,0.03); border-radius: var(--radius-md); text-align: center;">
          No memories recorded yet.
        </div>
      {:else}
        {#each memories as memory}
          <div style="background: rgba(255,255,255,0.04); border: 1px solid var(--border-subtle); border-radius: var(--radius-md); padding: 16px; position: relative;">
            <div style="display: flex; justify-content: space-between; align-items: flex-start; margin-bottom: 12px;">
              <div style="display: flex; align-items: center; gap: 8px;">
                <span class="badge" style="background: {memory.memory_type === 'reasoning' ? 'rgba(88, 166, 255, 0.1)' : 'rgba(188, 140, 255, 0.1)'}; color: {memory.memory_type === 'reasoning' ? 'var(--accent-blue)' : 'var(--accent-purple)'}; text-transform: uppercase;">
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
                  class="settings-input"
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
  </div>

  <!-- About -->
  <div class="card">
    <h3 style="font-size: var(--text-lg); font-weight: 600; margin-bottom: 16px;">About ColimaUI</h3>
    <p style="font-size: var(--text-sm); color: var(--text-secondary); line-height: 1.7; margin: 0;">
      ColimaUI is a cross-platform graphical interface for managing Colima instances,
      Docker containers, Kubernetes clusters, and Linux VMs. Built with Tauri v2 and Svelte 5.
    </p>
    <div style="margin-top: 16px; display: flex; gap: 12px; flex-wrap: wrap;">
      <span class="badge" style="background: rgba(88, 166, 255, 0.1); color: var(--accent-blue);">v0.1.0</span>
      <span class="badge" style="background: rgba(188, 140, 255, 0.1); color: var(--accent-purple);">Tauri v2</span>
      <span class="badge" style="background: rgba(255, 62, 0, 0.1); color: #ff3e00;">Svelte 5</span>
      <span class="badge" style="background: rgba(63,185,80,0.1); color: var(--accent-green);">Rust</span>
    </div>
  </div>
</div>
</div>
