# Backend Reference

The Rust backend (~42,000 lines across modular layers) is organized into **commands** (business logic), **routes** (HTTP delegation), **adapters** (CLI abstraction), and **services** (orchestration).

## Module Map

```
src-tauri/src/
├── lib.rs                 # App setup, plugin registration, command handler registration
├── main.rs                # Entry point (calls lib::run())
├── api_server.rs          # Axum router + HTTP server startup (127.0.0.1:11420–11429)
├── auth.rs                # CSPRNG Bearer token, IPC & 0600 ~/.colima-ui/api_token handoff
├── sse.rs                 # SSE broadcast, topic subscriber tracking, Docker & instance watchers
├── error.rs               # Structured ColimaError classification & remediation hints
├── redact.rs              # Credential & secret redaction before logs, UI, or LLM prompts
├── validation.rs          # Security: shell injection, banned flags, ID & path containment
├── helpers.rs             # ApiResponse, ok/err, run_blocking, run_cmd, TimedCache
├── streaming_cmd.rs       # Streaming child-process runner with process-group cancellation
├── transfer_registry.rs   # Active & recently completed background transfer tracking
├── platform.rs            # OS/arch/package manager detection
├── docker_state.rs        # Bollard Docker event stream for real-time push
├── instance_reader.rs     # Colima instance YAML config parser (fast filesystem read)
├── terminal_session.rs    # Real PTY session manager (portable-pty over Tauri IPC)
├── poller.rs              # Background instance status poller
├── path_util.rs           # macOS PATH & ~/.colima-ui app data dir helpers
├── telemetry/             # Opt-in closed-vocabulary telemetry & consent state
├── bin/                   # Standalone CLIs: cui.rs (AI client) & cli.rs (colimaui)
├── commands/              # Business logic (53 modules)
├── routes/                # HTTP route handlers (26 modules)
├── adapters/              # Unified DevOps adapter traits (7 modules)
└── services/              # High-level orchestration (4 modules)
```

## Commands Layer (`commands/`)

The central business logic layer. Both Tauri IPC and HTTP routes call into these functions.

| Module | Lines | Responsibility |
|--------|-------|---------------|
| `containers.rs` | 966 | Docker container & image CRUD, stats, exec, run, prune |
| `knowledge_bank.rs` | 1001 | SQLite knowledge bank: solutions, feedback, memory, settings, presets |
| `searxng.rs` | 628 | SearXNG/DuckDuckGo web search + HTML→Markdown conversion |
| `colima.rs` | 730 | Colima instance lifecycle + diagnostics + worker nodes |
| `ai_chat.rs` | 862 | Multi-provider AI chat (Anthropic/OpenAI/Google/Ollama/...) |
| `kubernetes.rs` | 644 | kubectl operations: pods, deployments, services, namespaces, events |
| `system.rs` | 598 | System info, tool checks, host specs, resource saver mode |
| `shell_sandbox.rs` | 291 | 3-tier command sandbox: safe/approve/banned classification |
| `lima.rs` | 285 | Lima VM lifecycle + shell + templates + create |
| `models.rs` | 200 | Ollama model management (list, pull, serve, delete) |
| `compose.rs` | 156 | Docker Compose project management |
| `networks.rs` | 166 | Docker network CRUD + prune |
| `volumes.rs` | 178 | Docker volume CRUD + prune |
| `agent_loop.rs` | 135 | AI agent tool execution loop |
| `runtime.rs` | 159 | Runtime detection (docker vs nerdctl) |
| `self_heal.rs` | 1122 | Self-healing rules & kill switch (no route performs a repair) |
| `colima_config.rs` | 1101 | Read/write Colima instance configuration |
| `file_transfer.rs` | 1067 | Container copy & image TAR transfer jobs |
| `security_scan.rs` | 942 | Trivy scan orchestration |
| `security_rules.rs` | 780 | Configuration rule pack |
| `activity.rs` | 757 | Activity record: what was done to the machine |
| `diagnostics.rs` | 680 | Diagnostic bundle collection |
| `metrics_collector.rs` | 667 | The app's only sampling loop; persistent history plugs in via `set_metric_writer` |
| `metrics_store.rs` | — | SQLite metric series persistence, retention, and writer health |
| `alerts.rs` | — | Metric alert rule evaluation, cooldowns, and historical backtesting |
| `k8s_cluster.rs` | 632 | Cluster health, nodes, component status |
| `activity_feed.rs` | 577 | Feed assembly and export (JSON/CSV) |
| `kb_articles.rs` | 492 | Bundled offline knowledge-base articles (4 locales: `en`, `ja`, `vi`, `zh`) |
| `compose_diagnose.rs` | 473 | Compose file diagnosis (schema, undefined refs, YAML) |
| `compose_autofix.rs` / `compose_autofix_fixers.rs` / `compose_autofix_apply.rs` | — | Compose autofix proposal, preview, apply, undo, and history |
| `topology.rs` | 465 | Topology graph endpoint |
| `security_score.rs` | 415 | Score computation with scanner/db provenance |
| `security_triage.rs` / `security_autofix.rs` | — | Rule impact ranking and Dockerfile hardening patch proposals |
| `security_policy.rs` / `security_history.rs` / `security_watch.rs` | — | Security policies, digest scan history, and scheduled background rescans |
| `falco_bridge.rs` / `falco_triage.rs` | — | Falco VM runtime stream reader and redacted LLM triage prompt builder |
| `detonation.rs` | — | Isolated container detonation sessions and timeline export |
| `k8s_resources.rs` | 308 | Generic K8s resource browsing & CRDs |
| `system_capabilities.rs` | 276 | Capability reporting |
| `dockerfile_parse.rs` | 270 | Dockerfile parsing for the layer/security views |
| `security_catalog.rs` | 248 | Base-image alternatives catalog (local table) |
| `announcements.rs` | 239 | Release notes & advisories feed (fetch + filter) |
| `compose_services.rs` | 232 | Per-service compose inspection |
| `engine_resources.rs` | 232 | Aggregate engine CPU/memory (one-shot, expensive) |
| `autostart.rs` | 217 | Launch-at-login toggle |
| `activity_coverage.rs` | 182 | Asserts every mutating command records activity |
| `terminal.rs` | 126 | PTY terminal command surface |
| `kind.rs` | 63 | Kind cluster lifecycle |

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
| `k8s.rs` | 832 | 30+ Kubernetes endpoints (complex kubectl logic) |
| `payloads.rs` | 896 | All request/response struct definitions |
| `system.rs` | 217 | System info, tool checks, install deps, prune |
| `containers.rs` | 171 | Container CRUD, logs, stats, exec, run |
| `ai.rs` | 255 | AI chat, CLI chat, tool execution |
| `kb.rs` | 184 | Knowledge bank queries, feedback, memory |
| `images.rs` | 87 | Image CRUD, pull, prune |
| `lima.rs` | 77 | Lima VM operations |
| `instances.rs` | 69 | Colima instance management |
| `compose.rs` | 76 | Docker Compose operations |
| `networks.rs` | 53 | Network CRUD |
| `volumes.rs` | 54 | Volume CRUD |
| `models.rs` | 45 | Ollama model management |
| `misc.rs` | 221 | SSE events endpoint |
| `security.rs` | 370 | Security scan/audit/triage/policy/watch + Falco runtime endpoints |
| `detonation.rs` | 72 | Container detonation start/cancel/session/list/export endpoints |
| `metrics.rs` | 115 | Metric history, writer health, alert rules, events & backtest endpoints |
| `colima_config.rs` | 97 | Read/preview/apply `colima.yaml` + offline Help article endpoints |
| `file_transfer.rs` | 90 | Container copy & image TAR transfer jobs |
| `self_heal.rs` | 84 | Self-healing rules & kill switch (no route performs a repair) |
| `activity.rs` | 68 | Activity record: what was done to the machine |
| `capabilities.rs` | 35 | Capability probe endpoints |
| `diagnostics.rs` | 31 | Diagnostic bundle collection |
| `announcements.rs` | 21 | Release notes & advisories feed (fetch + filter) |
| `system_capabilities.rs` | 17 | Capability reporting |
| `topology.rs` | 15 | Topology graph endpoint |

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
| `docker.rs` | 372 | Docker CLI adapter |
| `nerdctl.rs` | 372 | nerdctl CLI adapter |
| `colima.rs` | 252 | Colima CLI adapter |
| `lima.rs` | 192 | Lima CLI adapter |
| `compose.rs` | 58 | Docker Compose adapter |
| `kubectl.rs` | 55 | kubectl adapter |

## Services Layer (`services/`)

Higher-level orchestration (currently lightweight):

| Module | Lines | Purpose |
|--------|-------|---------|
| `container.rs` | 104 | Container operations via adapter traits |
| `vm.rs` | 60 | VM operations via adapter traits |
| `compose.rs` | 28 | Compose operations via adapter traits |
| `orchestration.rs` | 28 | Cross-cutting orchestration |

## Core Infrastructure

### auth.rs
- Generates a 256-bit CSPRNG Bearer token (`OnceLock<String>`) on startup
- Enforces constant-time token comparison (`Authorization: Bearer <token>`) across all protected routes, with an explicit `?token=` allowlist (`QUERY_TOKEN_PATHS`: `/api/events`, `/api/k8s/pods/logs/stream`) for browser `EventSource` streams
- Delivers credentials via Tauri IPC (`api_token`), atomic `0600` `~/.colima-ui/api_token` + `~/.colima-ui/api_port` files inside `0700` `~/.colima-ui` (`O_NOFOLLOW`, ownership-verified exit cleanup) for `cui` and local host agents, and debug-only `#token=` URL fragments — never via an unauthenticated HTTP route

### api_server.rs
- Builds the Axum router (`public_routes` exposing only `GET /api/health` merged with `protected_routes` behind `auth_middleware`)
- Restricts CORS to localhost origins (`1420`, `11420`, `tauri://localhost`)
- Binds `127.0.0.1:11420` (falling back across `11420–11429`), serves no static frontend (`ServeDir`) in release builds, prints `Browser mode: http://localhost:1420/#token=...` only under `#[cfg(debug_assertions)]`, and terminates tracked `kubectl port-forward` children on shutdown

### docker_state.rs
- Connects to Docker via Bollard (socket auto-detection)
- Streams container events (`start`, `stop`, `die`, `create`, `destroy`)
- Publishes state changes to SSE and Tauri events
- Auto-reconnects on connection loss

### instance_reader.rs
- Reads Colima instance configs from `~/.colima/` YAML files
- Returns instance list in <1ms (vs 30–60s via `colima list` CLI)
- Parses status, CPU, memory, disk, runtime, arch, K8s config

### terminal_session.rs
- Opens real pseudo-terminals via `portable-pty` (`TERM=xterm-256color`, window resize / `SIGWINCH`, 1 MB bounded output buffer, 16 ms frame-coalesced UTF-8 carry decoder, and 30-minute idle reaper)
- Driven exclusively via Tauri IPC commands (`commands/terminal.rs`) and Tauri events — HTTP/WebSocket terminal endpoints were deliberately removed so no localhost port exposes interactive shell execution
- Spawns direct `argv` commands (never shell-interpolated strings) for SSH into Colima instances and Lima VMs (with per-profile shell history) and `kubectl exec` into Kubernetes pods
