# Backend Reference

The Rust backend (~12,800 lines) is organized into layers: **commands** (business logic), **routes** (HTTP delegation), **adapters** (CLI abstraction), and **services** (orchestration).

## Module Map

```
src-tauri/src/
├── lib.rs                 # App setup, plugin registration, command handler registration
├── main.rs                # Entry point (calls lib::run())
├── api_server.rs          # Axum router + HTTP server startup (port 11420)
├── auth.rs                # API token generation + Bearer auth middleware
├── sse.rs                 # SSE broadcast, Docker watcher, instance publisher
├── validation.rs          # Security: shell injection, banned flags, ID validation
├── helpers.rs             # ApiResponse, ok/err, run_blocking, run_cmd, TimedCache
├── platform.rs            # OS/arch/package manager detection
├── docker_state.rs        # Bollard Docker event stream for real-time push
├── instance_reader.rs     # Colima instance YAML config parser (fast filesystem read)
├── terminal_session.rs    # PTY-based terminal session for xterm.js
├── poller.rs              # Background instance status poller
├── path_util.rs           # macOS PATH fixup for Finder/Dock launches
├── commands/              # Business logic (14 modules)
├── routes/                # HTTP route handlers (14 modules)
├── adapters/              # Unified DevOps adapter traits (7 modules)
└── services/              # High-level orchestration (4 modules)
```

## Commands Layer (`commands/`)

The central business logic layer. Both Tauri IPC and HTTP routes call into these functions.

| Module | Lines | Responsibility |
|--------|-------|---------------|
| `containers.rs` | 717 | Docker container & image CRUD, stats, exec, run, prune |
| `knowledge_bank.rs` | 848 | SQLite knowledge bank: solutions, feedback, memory, settings, presets |
| `searxng.rs` | 631 | SearXNG/DuckDuckGo web search + HTML→Markdown conversion |
| `colima.rs` | 602 | Colima instance lifecycle + diagnostics + worker nodes |
| `ai_chat.rs` | 560 | Multi-provider AI chat (Anthropic/OpenAI/Google/Ollama/...) |
| `kubernetes.rs` | 472 | kubectl operations: pods, deployments, services, namespaces, events |
| `system.rs` | 445 | System info, tool checks, host specs, resource saver mode |
| `shell_sandbox.rs` | 282 | 3-tier command sandbox: safe/approve/banned classification |
| `lima.rs` | 241 | Lima VM lifecycle + shell + templates + create |
| `models.rs` | 188 | Ollama model management (list, pull, serve, delete) |
| `compose.rs` | 155 | Docker Compose project management |
| `networks.rs` | 145 | Docker network CRUD + prune |
| `volumes.rs` | 153 | Docker volume CRUD + prune |
| `agent_loop.rs` | 134 | AI agent tool execution loop |
| `runtime.rs` | 60 | Runtime detection (docker vs nerdctl) |

### Key Patterns

**Tauri commands** use `#[tauri::command]` and are registered in `lib.rs`:
```rust
#[tauri::command]
pub async fn start_container(container_id: String) -> Result<String, String> {
    // Security validation
    if !crate::validation::is_valid_container_id(&container_id) {
        return Err("Invalid container ID format".to_string());
    }
    // Execute via CLI
    tokio::task::spawn_blocking(move || {
        docker_output(&["start", &container_id])
    }).await.map_err(|e| format!("Task error: {}", e))?
}
```

**CLI-only variants** (`_cli` suffix) exist for operations where the Tauri command requires state (`tauri::State`) that HTTP routes cannot provide:
- `list_containers_cli()` — No Bollard/DockerState, falls back to `docker ps`
- `stop_instance_cli()` / `delete_instance_cli()` — No Docker state reconnect

## Routes Layer (`routes/`)

Thin HTTP handlers that extract request parameters and delegate to `commands/`:

| Module | Lines | Endpoints |
|--------|-------|-----------|
| `k8s.rs` | 1,182 | 30+ Kubernetes endpoints (complex kubectl logic) |
| `payloads.rs` | 807 | All request/response struct definitions |
| `system.rs` | 261 | System info, tool checks, install deps, prune |
| `containers.rs` | 171 | Container CRUD, logs, stats, exec, run |
| `ai.rs` | 142 | AI chat, CLI chat, tool execution |
| `kb.rs` | 105 | Knowledge bank queries, feedback, memory |
| `images.rs` | 87 | Image CRUD, pull, prune |
| `ws.rs` | 79 | WebSocket terminal sessions |
| `lima.rs` | 77 | Lima VM operations |
| `instances.rs` | 69 | Colima instance management |
| `compose.rs` | 65 | Docker Compose operations |
| `networks.rs` | 53 | Network CRUD |
| `volumes.rs` | 54 | Volume CRUD |
| `models.rs` | 45 | Ollama model management |
| `misc.rs` | 17 | SSE events endpoint |

### Delegation Pattern

```rust
// routes/containers.rs — thin delegation
pub async fn api_start_container(
    Query(q): Query<ContainerIdQuery>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    match containers::start_container(q.container_id).await {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}
```

## Adapters Layer (`adapters/`)

Trait-based abstraction over CLI tools for future testability:

| Module | Lines | Abstraction |
|--------|-------|-------------|
| `traits.rs` | 270 | `ContainerRuntime`, `VmManager`, `Orchestrator` traits |
| `docker.rs` | 370 | Docker CLI adapter |
| `nerdctl.rs` | 367 | nerdctl CLI adapter |
| `colima.rs` | 246 | Colima CLI adapter |
| `lima.rs` | 186 | Lima CLI adapter |
| `compose.rs` | 52 | Docker Compose adapter |
| `kubectl.rs` | 49 | kubectl adapter |

## Services Layer (`services/`)

Higher-level orchestration (currently lightweight):

| Module | Lines | Purpose |
|--------|-------|---------|
| `container.rs` | 104 | Container operations via adapter traits |
| `vm.rs` | 60 | VM operations via adapter traits |
| `compose.rs` | 28 | Compose operations via adapter traits |
| `orchestration.rs` | 28 | Cross-cutting orchestration |

## Core Infrastructure

### api_server.rs (301 lines)
- Builds the Axum router with all route groups
- Applies CORS (localhost origins), auth middleware, and CSP headers
- Starts the HTTP server on port 11420 in a background tokio task

### docker_state.rs (335 lines)
- Connects to Docker via Bollard (socket auto-detection)
- Streams container events (`start`, `stop`, `die`, `create`, `destroy`)
- Publishes state changes to SSE and Tauri events
- Auto-reconnects on connection loss

### instance_reader.rs (226 lines)
- Reads Colima instance configs from `~/.colima/` YAML files
- Returns instance list in <1ms (vs 30-60s via `colima list` CLI)
- Parses status, CPU, memory, disk, runtime, arch, K8s config

### terminal_session.rs (231 lines)
- Manages PTY sessions for integrated terminal
- WebSocket bridge between xterm.js frontend and shell process
- Supports SSH into Colima instances and Lima VMs
