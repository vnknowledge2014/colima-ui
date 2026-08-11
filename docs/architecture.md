# Architecture

ColimaUI uses a **dual-mode, event-driven architecture** that works as both a native desktop app and a browser-based web application, sharing the same Rust backend.

## High-Level Overview

```mermaid
graph TD
    subgraph Frontend["Frontend — Svelte 5 + TypeScript"]
        APP["App.svelte"]
        PAGES["Pages (16)"]
        STORE["Rune Stores"]
        API["api.ts (dual-mode)"]
    end

    APP --> PAGES --> API
    APP --> STORE

    subgraph Dual["Dual-Mode API Layer"]
        TAURI_IPC["Tauri IPC (invoke)"]
        HTTP["Axum HTTP (:11420)"]
    end

    API -->|"Desktop"| TAURI_IPC
    API -->|"Browser"| HTTP

    subgraph Events["Real-time Push"]
        TAURI_EVT["Tauri Events"]
        SSE["SSE /api/events"]
    end

    TAURI_EVT -->|"Desktop"| STORE
    SSE -->|"Browser"| STORE

    subgraph Backend["Backend — Rust"]
        CMD["commands/ (business logic)"]
        ROUTES["routes/ (HTTP thin layer)"]
        BROADCAST["tokio::broadcast"]
        WATCHER["Docker Event Watcher"]
    end

    TAURI_IPC --> CMD
    HTTP --> ROUTES --> CMD
    WATCHER --> BROADCAST
    BROADCAST --> SSE
    BROADCAST --> TAURI_EVT

    subgraph CLI["External Tools"]
        COLIMA["colima"]
        DOCKER["docker"]
        KUBECTL["kubectl"]
        LIMACTL["limactl"]
    end

    CMD --> CLI
```

## Dual-Mode Design

The frontend detects whether it's running inside Tauri (native) or a browser:

```typescript
// src/lib/api.ts
const IS_TAURI = '__TAURI_IPC__' in window;

// Desktop mode: Tauri IPC (invoke)
// Browser mode: HTTP fetch to localhost:11420
```

This allows **one codebase** to serve both modes without conditional branches in page components.

## Event-Driven Updates

| Mode | Push Mechanism | Fallback |
|------|---------------|----------|
| **Desktop** | Tauri IPC events (`instances-update`, `docker-state-updated`) | — |
| **Browser** | `EventSource` → `/api/events` (SSE) | HTTP polling (3–5s) |

The backend uses `tokio::broadcast` channels to fan-out events to both Tauri events and SSE simultaneously.

## Request Flow

### Desktop Mode
```
User Action → Svelte Component → api.ts → tauri.invoke() → lib.rs → commands/*.rs → CLI tool → Response
```

### Browser Mode
```
User Action → Svelte Component → api.ts → fetch() → api_server.rs → routes/*.rs → commands/*.rs → CLI tool → Response
```

## Security Architecture

Security validations are applied at the **commands layer** so both entry points (Tauri IPC and HTTP) are protected:

```
┌─────────────────┐     ┌─────────────────┐
│  Tauri IPC       │     │  HTTP Routes     │
│  (invoke)        │     │  (Axum)          │
└────────┬────────┘     └────────┬────────┘
         │                       │
         └──────────┬────────────┘
                    │
         ┌──────────▼──────────┐
         │  commands/*.rs       │
         │  - shell injection   │
         │  - banned flags      │
         │  - container ID      │
         │  - host mount block  │
         └──────────┬──────────┘
                    │
         ┌──────────▼──────────┐
         │  CLI Tools           │
         │  docker / colima /   │
         │  kubectl / limactl   │
         └─────────────────────┘
```

### Validation Rules (validation.rs)

| Check | Function | Protects |
|-------|----------|----------|
| Shell injection | `contains_shell_injection()` | `container_exec`, `lima_shell` |
| Container ID format | `is_valid_container_id()` | Container operations |
| Banned Docker flags | `BANNED_DOCKER_FLAGS` | `run_container` (blocks `--privileged`, `--pid=host`, etc.) |
| Host root bind mount | Inline check | `run_container` (blocks `source=/` bind mounts) |
| K8s name format | `is_valid_k8s_name()` | Kubernetes resource names |
| Profile name | `is_valid_profile_name()` / `ensure_valid_profile()` | Colima profile arguments — a name starting with `-` would otherwise be read by `colima` as a flag, and the name is also a path component under `~/.colima` |
| Resource name | `is_valid_resource_name()` | Lima VM, volume and network names passed positionally to CLI tools |
| Path containment | `assert_path_within()` | Any filesystem path built from user input; fails closed when the parent cannot be resolved |

### Secret Redaction

`src-tauri/src/redact.rs` and `src/lib/redact.ts` strip credentials from any
string that reaches the user, a log, or an LLM prompt. Both sides exist because
the frontend calls LLM providers directly, so provider errors are built in the
browser and never pass through Rust.

Redaction works two ways: by **position** (credential-shaped query parameters
and auth headers, whatever the value looks like — this covers providers we have
never heard of) and by **shape** (known key formats anywhere in the string).
Docker image digests are deliberately excluded from the shape rules so
diagnostics stay readable.

On the frontend the choke point is `globalToast()`: it is the last place every
user-visible message passes through, and it also forwards error text to the AI
diagnostics listener — i.e. off to a third-party LLM.

## HTTP Authentication

The HTTP API uses **Bearer token** authentication:

1. Token is auto-generated at startup with a CSPRNG (`auth::get_api_token`)
2. Browser clients obtain it via `GET /api/auth/token`
3. All other endpoints require `Authorization: Bearer <token>`, compared in constant time
4. `/api/health` and `/api/auth/token` are the only unauthenticated routes; the list is explicit in `api_server.rs`, not pattern-matched
5. SSE endpoints authenticate via `?token=` because `EventSource` cannot set headers. The set of such endpoints is an explicit allowlist (`auth::QUERY_TOKEN_PATHS`) — a route is never switched to query-token auth just because its path ends in `/stream`

### Accepted risks

Two weaknesses are known and deliberately left in place, because every available
fix costs a feature the product needs. Revisit both if the threat model changes
(e.g. multi-user machines become a target).

| Risk | Why it stays |
|---|---|
| `GET /api/auth/token` is unauthenticated, so any local process can obtain the token and then read everything the API exposes — including the configured AI API key via `GET /api/settings` | Browser mode needs an unauthenticated bootstrap for a browser tab to obtain a token. Anything that blocks another local process also blocks the browser. The real fix is for the desktop app to hand the token to the browser directly, which changes how browser mode is opened |
| `capabilities/default.json` allows `https://*` | This is what lets the app reach OpenRouter, Groq, Together, Mistral, DeepSeek and user-configured endpoints (custom Ollama, private SearXNG). Tauri capabilities are compile-time, so a narrow list cannot be extended at runtime — narrowing would permanently break 6 of the 9 advertised providers. The fix is to route provider calls through Rust (`reqwest` is not bound by capabilities) |

## State Management

### Frontend (Svelte 5 Runes)

| Store | File | Contents |
|-------|------|----------|
| Global | `store.svelte.ts` | Instances, containers, images, volumes, networks, compose, system info |
| AI | `store/ai.svelte.ts` | Chat messages, provider config, agent state |
| K8s | `store/k8s.svelte.ts` | Namespaces, pods, deployments, services, CRDs |
| Confirm | `store/confirm.svelte.ts` | Confirmation dialog state |

### Backend (Rust)

| State | Scope | Purpose |
|-------|-------|---------|
| `DockerState` | `tauri::State` (managed) | Bollard client, container/image cache |
| `PollerState` | `tauri::State` (managed) | Background instance poller handle |
| `ResourceSaverState` | `tauri::State` (managed) | Resource saver mode timer |
| `SYSTEM_INFO_CACHE` | `LazyLock<Mutex<TimedCache>>` | Cached system info (colima/docker versions) |
| `SSE_TX` | `LazyLock<broadcast::Sender>` | SSE event broadcaster |
| `API_TOKEN` | `LazyLock<String>` | Random auth token |
| `PORT_FORWARDS` | `LazyLock<Mutex<HashMap>>` | Active K8s port forwards |
