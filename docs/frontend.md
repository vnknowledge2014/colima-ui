# Frontend Reference

The frontend is built with **Svelte 5** (runes-based reactivity), **TypeScript**, and **Vite 8**. It supports both native desktop (Tauri) and browser modes.

## Directory Structure

```
src/
├── App.svelte                  # Main shell: sidebar, page routing, event listeners
├── main.ts                     # Svelte mount entry point
├── index.css                   # Global styles import
├── vite-env.d.ts               # Vite type declarations
├── setupTests.ts               # Test setup (jsdom)
├── store.svelte.ts             # Global reactive state (uiState, dockerState, resourceState, dashboardState)
├── pages/                      # 17 navigable sidebar pages + 3 sub-views (20 .svelte files)
├── components/                 # Shared & domain components (ai/, activity/, compose/, security/, topology/, notifications/, settings/, transfer/, diagnostics/)
├── store/                      # Domain-specific state modules
├── lib/                        # Utilities, API layer, AI bus
├── styles/                     # CSS design system
└── locales/                    # i18n translations (en, ja, vi, zh)
```

## Pages

`src/pages/` contains **17 top-level navigable pages** (`uiState.currentPage`: `dashboard`, `instances`, `environment`, `containers`, `images`, `volumes`, `networks`, `compose`, `topology`, `activity`, `security`, `kubernetes`, `linux-vms`, `models`, `terminal`, `settings`, `help`) plus **3 embedded sub-views** (`TerminalInstance.svelte`, `XRay.svelte`, `ClusterTopology.svelte`):

| Page | File | Description |
|------|------|-------------|
| Dashboard | `Dashboard.svelte` | System overview, resource counts, quick navigation |
| Instances | `Instances.svelte` | Colima instance & Kind cluster management |
| Environment | `Environment.svelte` | Read-only provenance record of resolved Colima/Lima/Docker/Kubeconfig paths and `launchd`-vs-shell environment duality |
| Containers | `Containers.svelte` | Docker container list, actions, stats, logs, interactive PTY terminal launch (`dockerExec`) |
| Images | `Images.svelte` | Docker image management, build, and background TAR save/load |
| Volumes | `Volumes.svelte` | Docker volume management |
| Networks | `Networks.svelte` | Docker network management |
| Compose | `Compose.svelte` | Docker Compose project lifecycle, build/pull, Compose Diagnose & AI Autofix |
| Topology | `Topology.svelte` | Docker graph: containers ↔ networks/volumes/projects/images (distinct from the K3s node graph in `ClusterTopology.svelte` and K8s resource diagnostics in `XRay.svelte`) |
| Activity | `Activity.svelte` | Live container metrics, alert rules, self-healing & unified activity timeline; holds the SSE subscription that gates backend sampling |
| Security | `Security.svelte` | Local Trivy vulnerability scanning, `/100` provenance score breakdown, config rules, base-image alternatives, Falco runtime stream, and container Detonation |
| Kubernetes | `Kubernetes.svelte` | Multi-resource K8s browser, pod log streaming, port-forwarding, and PTY pod exec |
| Linux VMs | `LinuxVMs.svelte` | Lima VM management |
| Models | `Models.svelte` | Ollama AI model management |
| Terminal | `Terminal.svelte` | Integrated xterm.js PTY terminal (`colima`, `lima`, `dockerExec`, `k8sExec`) |
| Settings | `Settings.svelte` | System info, Colima `colima.yaml` editor, AI provider config, auto-updates, privacy/telemetry, notifications, disk usage, prune |
| Help | `Help.svelte` | Offline FTS5-searchable troubleshooting & setup guides across 4 locales (`en`, `ja`, `vi`, `zh`) |
| *(Sub-view)* Terminal Instance | `TerminalInstance.svelte` | Individual PTY terminal tab mounted inside `Terminal.svelte` |
| *(Sub-view)* X-Ray | `XRay.svelte` | Kubernetes resource relationship diagnostics (`Ingress ↔ Service ↔ Deployment/StatefulSet/DaemonSet ↔ Pod`) |
| *(Sub-view)* Cluster Topology | `ClusterTopology.svelte` | Kubernetes / K3s Master & Worker node cluster topology graph |

## Components

| Component | Purpose |
|-----------|---------|
| `AiChatPanel.svelte` | Command Center AI panel: multi-thread personal chat, Live UI Bridge (`agent-external-chat-turn` live mirroring for `--mirror-ui` turns while keeping `colima-a2a-default-session` isolated), live context toggle, 1-touch error diagnostic banner, schedule timers |
| `ai/AiMessageBody.svelte` | Streaming `<think>` disclosure, GFM tables, and XSS-safe Markdown AST renderer |
| `ai/CodeBlock.svelte` | Code block viewer with Copy, Insert-to-chat, and safe paste-without-Enter Terminal handoff |
| `ai/ToolStepCard.svelte` | Unified in-place card for queries, approvals (`SAFE`/`NORMAL`/`DANGEROUS`), sandbox commands, diagnostics & security tasks |
| `ai/InteractiveChoiceCard.svelte` | Consultant-mode `[ASK_CHOICE]` single/multi-select card with custom reply input |
| `ai/AiFollowUpBar.svelte` | Horizontal `[SUGGEST]` follow-up chip bar (with context-aware fallback chips) |
| `ai/AiEmptyState.svelte` | Warning-prioritized starter prompts and zero-config onboarding |
| `ai/AiModelQuickBar.svelte` | Inline AI provider/model switcher and `[LIVE_UI_CONTEXT]` toggle |
| `ConfirmDialog.svelte` | Reusable confirmation dialog (destructive actions) |
| `ContextMenu.svelte` | Right-click context menus |
| `ErrorBoundary.svelte` | Error boundary wrapper |
| `SetupWizard.svelte` | First-run guided setup |
| `GettingStartedTour.svelte` | Interactive feature walkthrough |
| `Icon.svelte` | Dynamic icon component |
| `Icons.svelte` | SVG icon definitions |
| `KubernetesHealth.svelte` | Cluster health dashboard |
| `settings/UpdateSettings.svelte` | Signed in-app auto-update check, release notes preview, and install via `tauri-plugin-updater` |

## State Management

Uses **Svelte 5 Runes** (`$state`, `$derived`, `$effect`) for fine-grained reactivity.

### Global Store (`src/store.svelte.ts`)

Exports structured `$state` objects shared across views:

```typescript
// Structured reactive state objects using Svelte 5 runes
export const dockerState = $state({ containers: [], images: [], containerStats: {}, /* ... */ });
export const resourceState = $state({ volumes: [], networks: [], /* ... */ });
export const dashboardState = $state({ colimaInstances: [], k8sStatus: null, linuxVMs: [], systemInfo: null, engineResources: null, /* ... */ });
export const uiState = $state({ currentPage: 'dashboard' as Page, theme: 'dark', aiPanelOpen: false, terminalTabs: [], /* ... */ });
```

### Domain Stores (`src/store/`)

| Store | Contents |
|-------|----------|
| `ai.svelte.ts` | Personal multi-conversation threads (isolating background `colima-a2a-default-session` turns unless `--mirror-ui` is enabled), chat messages, provider config (`6` modes), model selection, agent state |
| `k8s.svelte.ts` | Namespaces, pods, deployments, services, CRDs, selected context |
| `confirm.svelte.ts` | Confirmation dialog visibility and callbacks |
| `capabilities.svelte.ts` | Host external tool capability probes (`colima`, `docker`, `kubectl`, `trivy`, etc.) and `install_hint` cache |
| `notifications.svelte.ts` | Session-scoped unified store for running/completed background transfers, announcements, and reported errors |

## API Layer (`src/lib/api.ts` & `src/lib/api/`)

The dual-mode API layer (`src/lib/api/`, barrel-exported through `src/lib/api.ts`) automatically routes calls based on runtime:

```typescript
import { isRunningInTauri } from './env';   // never inline the check
const IS_TAURI = isRunningInTauri();

// Desktop: invoke Tauri command
// Browser: HTTP fetch to localhost:11420
async function listContainers(all: boolean) {
  if (IS_TAURI) {
    return invoke('list_containers', { all });
  } else {
    return apiFetch(`/api/containers?all=${all}`);
  }
}
```

### API Namespaces (`src/lib/api/`)

| Namespace | Responsibility |
|-----------|----------------|
| `colimaApi` / `colimaConfigApi` | Colima instance lifecycle, status, SSH, K3s worker creation, and `colima.yaml` read/preview/apply |
| `dockerApi` / `volumesApi` / `networksApi` | Container, image, volume, network CRUD, stats, logs, exec, run, build, and prune |
| `composeApi` | Docker Compose lifecycle (`up`, `down`, `restart`, `build`, `pull`), Compose Diagnose, and AI Autofix |
| `topologyApi` / `transferApi` | Docker topology graph and background image `.tar` / container `cp` transfer jobs |
| `k8sApi` / `kindApi` | Kubernetes multi-resource queries, YAML apply, scale, port-forward, benchmark, and Kind clusters |
| `limaApi` / `modelsApi` | Lima VM management and local Ollama model lifecycle (`list`, `pull`, `serve`, `delete`) |
| `aiApi` / `knowledgeBankApi` / `helpApi` / `sandboxApi` | Multi-provider AI chat, conversations/history, Knowledge Bank & LTM, offline Help articles, and shell sandbox |
| `securityApi` / `falcoApi` / `detonationApi` | Trivy CVE/audit scanning, SBOM export, policies, Falco runtime stream, and container Detonation |
| `metricsApi` / `alertsApi` / `selfHealApi` / `activityApi` | Bollard metric history, alert rules & backtest, self-healing rules, and activity timeline/export |
| `diagnosticsApi` / `environmentApi` | Redacted diagnostic bundles, VM logs, and `launchd`-vs-shell environment path provenance |
| `systemApi` / `sysMethods` / `settingsApi` | Host specs, tool probes, autostart, Resource Saver, SQLite settings, and custom VM presets |
| `announcementsApi` / `telemetryApi` / `subscriptionApi` | Static advisories feed, opt-in telemetry consent/preview, and local offline entitlement state |

## Event System

### Desktop Mode (Tauri Events)
```typescript
import { listen } from '@tauri-apps/api/event';

listen('docker-state-updated', (event) => {
  dockerState.containers = event.payload.containers;
  dockerState.images = event.payload.images;
});

listen('instances-update', (event) => {
  dashboardState.colimaInstances = event.payload;
});

// v0.3.0 Live UI Bridge: remote page/section navigation (`cui ui navigate`, MCP `ui_navigate`, POST /api/ui/navigate)
listen('ui-navigate', (event) => {
  uiState.currentPage = event.payload.page;
});

// v0.3.0 Live UI Bridge: opt-in `--mirror-ui` external CLI / MCP / A2A chat turn mirroring
listen('agent-external-chat-turn', (event) => {
  // Appends mirrored external turn into AiChatPanel when mirror_ui is true
});
```

### Browser Mode (SSE)
```typescript
// EventSource cannot set headers, so /api/events authenticates via ?token=.
// getApiToken() reads it from IPC (desktop) or from the URL fragment the tab
// was opened with (browser) — there is no endpoint to fetch it from.
const es = new EventSource(`/api/events?token=${await getApiToken()}`);

es.addEventListener('docker-state-updated', (e) => {
  const data = JSON.parse(e.data);
  dockerState.containers = data.containers;
});

es.addEventListener('instances-update', (e) => {
  const data = JSON.parse(e.data);
  dashboardState.colimaInstances = data.instances ?? data;
});

// Live UI Bridge events (`ui-navigate` and `agent-external-chat-turn`) also fan out over `/api/events`
es.addEventListener('ui-navigate', (e) => {
  const data = JSON.parse(e.data);
  uiState.currentPage = data.page;
});
```

## Lib Utilities

| Module | Purpose |
|--------|---------|
| `agentCore.ts` | 5-round Command Center AI orchestrator, conversation window hygiene, 80%/20% tool output pruner, repeat-tool guard |
| `aiEventBus.ts` | AI agent event bus with 100 domain-specific tool registrations (`SAFE` / `NORMAL` / `DANGEROUS`) |
| `aiToolParser.ts` | Bracket-balanced colon tags (`[TAG: ...]`) & streaming partial-tag parser (`stripStreamingPartialTags`, `cleanToolSyntaxRemnants`) |
| `chatMarkdown.ts` (`aiMessageParser`) | XSS-sanitized Markdown & streaming `<think>` reasoning block / GFM table / shell code AST parser |
| `aiToolStep.ts` | Structured state machine & serializer for inline `ToolStepCard` entries |
| `aiSuggestions.ts` | Warning-prioritized empty-state starters, fallback follow-up chips, and `[LIVE_UI_CONTEXT]` snapshot builder |
| `aiProviderConfig.ts` | Per-provider credential/model storage across the 6 canonical provider modes (`anthropic`, `openai`, `gemini`, `openai-compatible`, `ollama-local`, `ollama-cloud`) |
| `llmProviders.ts` | Multi-provider SSE streaming client (6 built-in provider modes + multi-packet `<think>` normalization) |
| `formatters.ts` | Display formatting (bytes, dates, durations) |
| `normalizers.ts` | Data normalization utilities |
| `markdown.ts` | General Markdown rendering utilities (Help articles, notes) |
| `k8sUtils.ts` | Kubernetes resource utilities |
| `globalToast.ts` | Transient redacted toasts; feeds both the notification store and AI error diagnostic listener |
| `transferEvents.ts` | Transfer event subscription, reconnect, and desync signal |
| `transferNotifications.ts` | Wires transfer events into the notification store |
| `osNotify.ts` | Operating-system notifications, desktop only |
| `i18n.svelte.ts` | Internationalization with reactive locale (`en`, `ja`, `vi`, `zh`) |
| `settingsStore.svelte.ts` | SQLite-backed persistent settings |
| `presetStateManager.ts` | Instance configuration preset management |
| `terminal-transport.ts` | Tauri IPC PTY transport, `SessionKind` (`colima`, `lima`, `dockerExec`, `k8sExec`), tab labels, and shell-less `exitHint` |

### Notifications and background transfers

Transfers — image export/import and container copies — run in the backend and
outlive the dialog that starts them. The dialog collects paths, starts the job and
**closes**; a multi-gigabyte export used to hold the page behind a modal for its
whole duration.

| Piece | Role |
|-------|------|
| `store/notifications.svelte.ts` | The one list: running jobs, outcomes, and reported errors. Session-scoped, never persisted |
| `components/notifications/` | The centre, opened from the bell in the sidebar header |
| `components/ErrorDetailPanel.svelte` | The same store, filtered to failures |

**Events are the fast path; `transferApi.list()` is the truth.** The SSE channel
drops frames for a client that falls behind and replays nothing after a reconnect,
so every gap — a reconnect, a lag notice, a fresh mount after a reload — triggers a
reconcile against the backend. Without it a transfer that finished while the
connection was down would sit on screen as "running" for the rest of the session,
with a cancel button that does nothing.

Two consequences worth knowing:

- A job the backend no longer lists is marked `ended`, not `success`. Terminal
  entries are retained for about a minute; past that the outcome is genuinely
  unknowable, and reporting a failed export as finished would be worse than saying
  so.
- Entries carry **no host paths** — an image reference, a path inside a container,
  a file name. That is what lets `formatEntryForClipboard` promise its output is
  safe to paste into an issue, and what lets `osNotify` send a title to the
  operating system's own notification centre.

### Announcements (`lib/announcements.ts`)

The one entry kind whose content comes from outside the machine: a static feed of
release notes and security advisories, fetched by the backend
(`GET /api/announcements`) at start-up and every six hours. Everything the
frontend does with it assumes the feed could be hostile or wrong:

- Text is rendered with normal bindings, never `{@html}`, and is length-capped.
- A `linkUrl` is only offered when it is `https:` on a short host allowlist
  (`isAllowedAnnouncementLink`); otherwise no link is drawn at all. `openExternal`
  refuses non-`https:` URLs for every caller as a second layer.
- `severity: "critical"` bypasses the audience and version filters, so a failed
  entitlement read or an unparseable version bound cannot hide an advisory.
- Ids already shown are remembered in settings, so nothing reappears on restart.
- The channel can be switched off in Settings → Notifications, and off means no
  request is made. See `docs/telemetry.md` for what a request discloses.

### Security page (`pages/Security.svelte`)

Pick a local image, scan it, and see why it scores what it does. Four components
under `components/security/`: the score breakdown, the configuration checklist,
the findings table, and the alternative base images.

- **Nothing is scanned on arrival.** A scan spawns a process, reads a whole
  image and may download a 1.2 GB database first. That is not something to do
  because somebody opened a page.
- **The score is never shown bare.** `ScoreBreakdownCard` prints the scanner,
  its version, the database date and the rule pack version beside the number —
  the same image scores differently against a newer database, and two scanners
  disagree by more than tenfold on identical input.
- **Rule and catalog text is rendered as text.** The rule pack is intended to be
  downloadable later; `{@html}` on downloadable content is how a rule pack turns
  into a script.
- **Alternatives come from a table inside the app** (`resources/catalog/v1.json`),
  not from a query. The image list never leaves the machine.
- The signed update channel for that catalog is not built yet — it needs a
  private key and a publishing process (see the plan's Unresolved #1). Nothing in
  the UI claims otherwise.

### AI Event Bus (`aiEvents/`)

Domain-specific AI tool handlers:

| Module | Tools |
|--------|-------|
| `docker.ts` | Container/image/volume/network operations |
| `colima.ts` | Instance lifecycle, diagnostics |
| `k8s.ts` | Kubernetes resource management |
| `lima.ts` | Lima VM operations |
| `compose.ts` | Docker Compose operations |
| `volumes.ts` | Volume-specific operations |
| `system.ts` | System checks, environment resolutions, tool installation, local Ollama model actions |
| `config.ts` | App configuration & custom VM hardware presets |
| `shell.ts` | Sandboxed `cli-exec` execution & UI `navigate` across all 17 pages |

## CSS Design System (`styles/`)

Vanilla CSS with a modular token-based architecture:

| File | Purpose |
|------|---------|
| `tokens.css` | Design tokens (colors, spacing, typography, shadows) |
| `reset.css` | CSS reset / normalize |
| `layout.css` | Grid, flexbox, sidebar layout |
| `components.css` | Buttons, cards, badges, tables |
| `forms.css` | Input, select, checkbox, toggle styles |
| `overlays.css` | Modals, dialogs, tooltips, toasts |
| `features.css` | Feature-specific styles (terminal, AI chat) |

## Testing

```bash
# Unit & component tests (Vitest + jsdom)
npm test

# Type & Svelte rune checking
npm run typecheck
npm run check
```

Test files follow the `*.test.ts` convention alongside source files:
- `aiToolParser.test.ts`
- `formatters.test.ts`
- `normalizers.test.ts`
- `markdown.test.ts`
- `i18n.test.ts`
- `terminal-transport.test.ts`
