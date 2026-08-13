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
├── store.svelte.ts             # Global reactive state
├── pages/                      # 18 page components
├── components/                 # 8 shared components
├── store/                      # Domain-specific state modules
├── lib/                        # Utilities, API layer, AI bus
├── styles/                     # CSS design system
└── locales/                    # i18n translations (en, vi, zh)
```

## Pages

| Page | File | Description |
|------|------|-------------|
| Dashboard | `Dashboard.svelte` | System overview, resource counts, quick navigation |
| Instances | `Instances.svelte` | Colima instance & Kind cluster management |
| Containers | `Containers.svelte` | Docker container list, actions, stats, logs |
| Images | `Images.svelte` | Docker image management |
| Volumes | `Volumes.svelte` | Docker volume management |
| Networks | `Networks.svelte` | Docker network management |
| Activity | `Activity.svelte` | Live container metrics; holds the SSE subscription that gates backend sampling |
| Topology | `Topology.svelte` | Docker graph: containers ↔ networks/volumes/projects/images. Distinct from the Kubernetes graph in `XRay.svelte` |
| Compose | `Compose.svelte` | Docker Compose project management |
| Kubernetes | `Kubernetes.svelte` | Multi-resource K8s browser |
| Linux VMs | `LinuxVMs.svelte` | Lima VM management |
| Models | `Models.svelte` | Ollama AI model management |
| Terminal | `Terminal.svelte` | Integrated xterm.js terminal |
| Terminal Instance | `TerminalInstance.svelte` | Individual terminal tab |
| Settings | `Settings.svelte` | System info, disk usage, prune |
| X-Ray | `XRay.svelte` | Kubernetes diagnostics |
| Cluster Topology | `ClusterTopology.svelte` | Cluster visualization |

## Components

| Component | Purpose |
|-----------|---------|
| `AiChatPanel.svelte` | AI diagnostic agent chat panel with tool execution and feedback |
| `ConfirmDialog.svelte` | Reusable confirmation dialog (destructive actions) |
| `ContextMenu.svelte` | Right-click context menus |
| `ErrorBoundary.svelte` | Error boundary wrapper |
| `SetupWizard.svelte` | First-run guided setup |
| `GettingStartedTour.svelte` | Interactive feature walkthrough |
| `Icon.svelte` | Dynamic icon component |
| `Icons.svelte` | SVG icon definitions |
| `KubernetesHealth.svelte` | Cluster health dashboard |

## State Management

Uses **Svelte 5 Runes** (`$state`, `$derived`, `$effect`) for fine-grained reactivity.

### Global Store (`store.svelte.ts`)

```typescript
// Reactive state using Svelte 5 runes
let instances = $state<ColimaInstance[]>([]);
let containers = $state<DockerContainer[]>([]);
let images = $state<DockerImage[]>([]);
// ... volumes, networks, compose, system info
```

### Domain Stores (`store/`)

| Store | Contents |
|-------|----------|
| `ai.svelte.ts` | Chat messages, provider config, model selection, agent state |
| `k8s.svelte.ts` | Namespaces, pods, deployments, services, selected context |
| `confirm.svelte.ts` | Confirmation dialog visibility and callbacks |

## API Layer (`lib/api.ts`)

The dual-mode API layer automatically routes calls based on runtime:

```typescript
const IS_TAURI = '__TAURI_IPC__' in window;

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

### API Sections

| Namespace | Methods |
|-----------|---------|
| `instanceApi` | list, start, stop, delete, status, ssh |
| `dockerApi` | containers, images, volumes, networks, compose |
| `k8sApi` | pods, deployments, services, configmaps, etc. |
| `limaApi` | list, start, stop, delete, shell, templates |
| `modelApi` | list, pull, serve, delete |
| `aiApi` | chat, listModels, history, feedback |
| `sysMethods` | checkSystem, hostSpecs, platform, tools |
| `settingsApi` | get/set settings, presets |
| `knowledgeBankApi` | query, learn, feedback, memory |
| `sandboxApi` | classify, execute, executeApproved |

## Event System

### Desktop Mode (Tauri Events)
```typescript
import { listen } from '@tauri-apps/api/event';

listen('docker-state-updated', (event) => {
  containers = event.payload.containers;
  images = event.payload.images;
});

listen('instances-update', (event) => {
  instances = event.payload;
});
```

### Browser Mode (SSE)
```typescript
// EventSource cannot set headers, so /api/events authenticates via ?token=.
// getApiToken() reads it from IPC (desktop) or from the URL fragment the tab
// was opened with (browser) — there is no endpoint to fetch it from.
const es = new EventSource(`/api/events?token=${await getApiToken()}`);

es.addEventListener('docker-state', (e) => {
  const data = JSON.parse(e.data);
  containers = data.containers;
});

es.addEventListener('instances', (e) => {
  instances = JSON.parse(e.data);
});
```

## Lib Utilities

| Module | Purpose |
|--------|---------|
| `aiEventBus.ts` | AI agent event bus with domain-specific tool registrations |
| `aiToolParser.ts` | Parse AI tool-call XML from LLM responses |
| `llmProviders.ts` | Multi-provider LLM integration config |
| `formatters.ts` | Display formatting (bytes, dates, durations) |
| `normalizers.ts` | Data normalization utilities |
| `markdown.ts` | Markdown rendering for AI responses |
| `k8sUtils.ts` | Kubernetes resource utilities |
| `globalToast.ts` | Transient toasts; every toast also becomes a notification entry |
| `transferEvents.ts` | Transfer event subscription, reconnect, and desync signal |
| `transferNotifications.ts` | Wires transfer events into the notification store |
| `osNotify.ts` | Operating-system notifications, desktop only |
| `i18n.svelte.ts` | Internationalization with reactive locale |
| `settingsStore.svelte.ts` | SQLite-backed persistent settings |
| `presetStateManager.ts` | Instance configuration preset management |

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
| `system.ts` | System checks, tool installation |
| `config.ts` | App configuration |

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
# Unit tests (Vitest + jsdom)
npx vitest run

# Type checking
npm run check    # svelte-check
```

Test files follow the `*.test.ts` convention alongside source files:
- `aiToolParser.test.ts`
- `formatters.test.ts`
- `normalizers.test.ts`
- `markdown.test.ts`
- `i18n.test.ts`
