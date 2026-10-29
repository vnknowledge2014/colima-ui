# Project: ColimaUI Daemon Connection & State Desync Resolution

## Architecture
ColimaUI is a desktop GUI for managing Colima instances and Docker containers on macOS. It consists of:
- **Tauri Backend (Rust)**:
  - System and socket discovery (`src-tauri/src/utils/path_util.rs`)
  - Instance reading and metadata parsing (`src-tauri/src/services/instance_reader.rs`)
  - Subprocess runtime execution (`src-tauri/src/services/runtime.rs`, `src-tauri/src/commands/runtime.rs`)
  - Docker state management & event watcher (`src-tauri/src/services/docker_state.rs`, `src-tauri/src/docker_state.rs`)
  - SSE API server (`src-tauri/src/api_server.rs`, `src-tauri/src/sse.rs`)
  - Self-healing supervisor (`src-tauri/src/services/self_heal.rs`, `src-tauri/src/commands/self_heal.rs`)
  - Activity logging (`src-tauri/src/services/activity.rs`, `src-tauri/src/commands/activity.rs`)
- **Svelte Frontend (TypeScript)**:
  - API clients (`src/lib/api/docker.ts`, `src/lib/api/compose.ts`, `src/lib/api/colima.ts`)
  - Reactive state stores (`src/store.svelte.ts`, `src/lib/dataPoller.ts`)
  - Views (`src/pages/Containers.svelte`, `src/pages/Compose.svelte`, `src/pages/Instances.svelte`, `src/pages/Dashboard.svelte`, `src/components/Sidebar.svelte`)

## Feature Inventory
| # | Feature | Description | Milestone | Source |
|---|---------|-------------|-----------|--------|
| 1 | Active Lima Process Liveness | Verify `ha.pid` via `libc::kill(pid, 0)` and `ha.sock` connection test | M1 | Survey / R1 |
| 2 | HTTP `GET /_ping` Socket Probing | Probe Unix domain sockets with HTTP handshake & 300ms timeout | M1 | Survey / R1 |
| 3 | Socket Health TTL Caching | 2-second TTL cache for socket health checks to prevent hot-path I/O overhead | M1 | Survey / R1 |
| 4 | Lima SSH Forwarder EOF Handling | Recognize 0-byte immediate EOF on socket as dead/booting daemon | M1 | Survey / R1 |
| 5 | Deterministic `detect_docker_host` | Prioritize active context in `~/.docker/config.json`, validate socket health, sort profiles | M1 | Survey / R1 |
| 6 | 4-Tier Instance Status | Distinguish `Running`, `Starting` (<90s), `Unhealthy` (>=90s), and `Stopped` | M1 | Survey / R1 |
| 7 | Fast Profile Runtime Reader | Direct parsing of `colima.yaml` without `colima status` subprocess | M1 | Survey / R1 |
| 8 | `StartConfig` Serde Hardening | Add `#[serde(default)]` and `u64` fields, auto-convert bytes >= 1 GiB to GiB | M2 | Survey / R2 |
| 9 | Async Command Execution | Transition `runtime::run` to `tokio::process::Command` with `kill_on_drop(true)` | M2 | Survey / R2 |
| 10 | Tiered Subprocess Timeouts | 15s `DEFAULT_TIMEOUT` for quick operations, 180s `LONG_OP_TIMEOUT` for Compose/build | M2 | Survey / R2 |
| 11 | CLI Fallback Label Preservation | Pipe CLI fallback container json through `normalize_cli_labels` | M2 | Survey / R2 |
| 12 | Centralized Lifecycle Signaling | `notify_instance_stopping` and `notify_instance_started` in `docker_state.rs` | M2 | Survey / R2 |
| 13 | `compose_start` Command & Route | Add backend command, route, and API server handler for starting stopped Compose projects | M2 | Survey / R2 |
| 14 | Resource Saver Success Check | Check `output.status.success()` in `docker stats` before calculating CPU idle metrics | M2 | Survey / R2 |
| 15 | Watcher 5s Ping Heartbeat | Add `docker.ping()` interval via `tokio::select!` in `start_docker_watcher` | M3 | Survey / R3 |
| 16 | Disconnect Event Propagation | Emit `docker-connection-lost` on Tauri & SSE and clear cache within 5s | M3 | Survey / R3 |
| 17 | SSE State Event Propagation | Emit `docker-state-updated` to SSE clients alongside Tauri | M3 | Survey / R3 |
| 18 | Cache Error Propagation | Return errors from `update_cache` and `None` from `sse::fetch_docker_state` on connection loss | M3 | Survey / R3 |
| 19 | Unresponsive VM Self-Healing | Implement `sweep_vm_unresponsive` for `HealTrigger::VmUnresponsive` | M3 | Survey / R3 |
| 20 | Real Restart Action | `HealAction::RestartVm` executes real restart sequence (`colima stop --force`, then start) | M3 | Survey / R3 |
| 21 | Test SQLite DB Isolation | Isolate test SQLite database path in `activity.rs` so tests never touch live `activity.db` | M3 | Survey / R3 |
| 22 | Frontend Container Normalization | Use `normalizeContainer` in `dockerApi.listContainers` to retain `Labels` | M4 | Survey / R4 |
| 23 | Frontend `composeApi.start` | Add `composeApi.start` method in `compose.ts` | M4 | Survey / R4 |
| 24 | Frontend Compose Start Button | Add Start button for stopped projects in `Compose.svelte` | M4 | Survey / R4 |
| 25 | Reactive `selectedContainer` | Convert `selectedContainer` in `Containers.svelte` to `$derived` from container ID | M4 | Survey / R4 |
| 26 | Action Error Cooldown Reset | Clear event cooldown and trigger immediate store refresh on action error | M4 | Survey / R4 |
| 27 | Poller Disconnect Handling | Clear container state and apply backoff on connection failure in `dataPoller.ts` | M4 | Survey / R4 |
| 28 | Instances Bytes-to-GiB | Normalize memory and disk bytes to GiB in `Instances.svelte` before calling `startInstance` | M4 | Survey / R4 |
| 29 | UI Starting & Unhealthy Badges | Add badge and tone support for `Starting` and `Unhealthy` across Instances, Dashboard, Sidebar | M4 | Survey / R4 |
| 30 | Acceptance Criteria & Release Gates | Cargo test, Clippy, npm checks, git branch/PR, CI check, app bundle build and install | M5 | Survey / R5 |

## Milestones
| # | Name | Scope | Dependencies | Status |
|---|------|-------|-------------|--------|
| M1 | Lima Process & Socket Health Probing | Features 1–7: `path_util.rs`, `instance_reader.rs` | none | DONE |
| M2 | Lifecycle Hardening & Async Subprocesses | Features 8–14: `colima.rs`, `runtime.rs`, `system.rs`, `containers.rs`, `compose.rs` | M1 | DONE |
| M3 | Real-Time State Sync & Self-Healing | Features 15–21: `docker_state.rs`, `sse.rs`, `self_heal.rs`, `activity.rs` | M1, M2 | DONE |
| M4 | Frontend Store & UI Synchronization | Features 22–29: `docker.ts`, `compose.ts`, `Containers.svelte`, `Compose.svelte`, `Instances.svelte`, `Dashboard.svelte`, `Sidebar.svelte` | M2, M3 | DONE |
| M5 | E2E Testing, Release Gates & App Installation | Feature 30: Acceptance criteria verification, cargo/npm gates, git PR, release build & install | M1, M2, M3, M4 | IN_PROGRESS |

## Interface Contracts

### M1 ↔ M2 / M3: Socket & Instance Health
- `crate::path_util::is_lima_instance_alive(lima_dir: &std::path::Path) -> bool`
- `crate::path_util::probe_docker_socket(sock: &std::path::Path, timeout: std::time::Duration) -> bool`
- `crate::path_util::probe_docker_socket_cached(sock: &std::path::Path) -> bool`
- `crate::path_util::invalidate_probe_cache()`
- `crate::path_util::detect_docker_host() -> Option<(String, String)>`
- `crate::instance_reader::read_profile_runtime(profile: &str) -> String`
- `crate::instance_reader::resolve_instance_status(colima_home: &std::path::Path, profile: &str, lima_dir: &std::path::Path, runtime: &str) -> &'static str`

### M2 ↔ M4: Backend Compose Start & Deserialization
- `compose_start(project_name: String) -> Result<String, String>` (Tauri command: `compose_start`, Route: `POST /api/compose/start`)
- `StartConfig`: all fields `#[serde(default)]`, `cpus: u32`, `memory: u64` (normalized < 1 GiB), `disk: u64` (normalized < 1 GiB).
- `notify_instance_stopping(profile: &str)`
- `notify_instance_started(profile: &str)`

### M3 ↔ M4: Real-time Event Contract
- Tauri Event & SSE Event: `"docker-connection-lost"` -> payload: `{ "host": String, "profile": String, "reason": String }`
- Tauri Event & SSE Event: `"docker-state-updated"` -> payload: `{ "containers": [...], "images": [...], "stats": {...} }`

## Code Layout
- Backend Source:
  - `src-tauri/src/utils/path_util.rs` (or `src-tauri/src/path_util.rs`)
  - `src-tauri/src/services/instance_reader.rs` (or `src-tauri/src/instance_reader.rs`)
  - `src-tauri/src/commands/colima.rs`
  - `src-tauri/src/services/runtime.rs` / `src-tauri/src/commands/runtime.rs`
  - `src-tauri/src/commands/containers.rs`
  - `src-tauri/src/commands/compose.rs`, `src-tauri/src/routes/compose.rs`, `src-tauri/src/api_server.rs`, `src-tauri/src/lib.rs`
  - `src-tauri/src/commands/system.rs`
  - `src-tauri/src/docker_state.rs`, `src-tauri/src/sse.rs`
  - `src-tauri/src/commands/self_heal.rs`
  - `src-tauri/src/commands/activity.rs`
- Frontend Source:
  - `src/lib/api/docker.ts`
  - `src/lib/api/compose.ts`
  - `src/store.svelte.ts`
  - `src/lib/dataPoller.ts`
  - `src/pages/Containers.svelte`
  - `src/pages/Compose.svelte`
  - `src/pages/Instances.svelte`
  - `src/pages/Dashboard.svelte`
  - `src/components/Sidebar.svelte`
  - `src/styles/components.css`
