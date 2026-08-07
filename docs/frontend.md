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
├── pages/                      # 16 page components
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
const es = new EventSource('/api/events');

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
| `globalToast.ts` | Global toast notification system |
| `i18n.svelte.ts` | Internationalization with reactive locale |
| `settingsStore.svelte.ts` | SQLite-backed persistent settings |
| `presetStateManager.ts` | Instance configuration preset management |

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
