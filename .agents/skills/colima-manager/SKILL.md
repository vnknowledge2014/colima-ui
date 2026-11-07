---
name: colima-manager
description: Control and orchestrate Colima UI via HTTP API capabilities
---

# Colima Manager Skill

This skill enables external orchestrators and IDE agents to drive **ColimaUI** via its **Native MCP Server (`cui mcp` / `colimaui mcp`)**, **Headless AI CLI (`cui`)**, or **Localhost HTTP API**.

## 1. Native MCP Server (`cui mcp` / `colimaui mcp`)

ColimaUI ships a built-in JSON-RPC 2.0 MCP server over `stdin`/`stdout` (`cui mcp` or `colimaui mcp`). It works seamlessly whether **ColimaUI Desktop is running** (proxying via `127.0.0.1:<api_port>` + `~/.colima-ui/api_token`) **or closed** (direct native fallback).

**15 Typed MCP Tools**:
`colima_list_instances`, `colima_instance_action`, `docker_list_containers`, `docker_container_action`, `docker_container_logs`, `docker_list_images`, `docker_pull_image`, `docker_list_volumes`, `docker_list_networks`, `compose_list_projects`, `k8s_list_pods`, `k8s_cluster_status`, `environment_resolutions`, `system_diagnostics`, `colima_ai_ask`.

**Host Configuration**:
- **Claude Desktop** (`~/Library/Application Support/Claude/claude_desktop_config.json`), **Cursor** (`~/.cursor/mcp.json`), **Antigravity** (`~/.gemini/antigravity/mcp_config.json`), **Windsurf** (`~/.codeium/windsurf/mcp_config.json`):
  ```json
  {"mcpServers": {"colima-ui": {"command": "cui", "args": ["mcp"]}}}
  ```
- **Zed** (`~/.config/zed/settings.json`):
  ```json
  {"context_servers": {"colima-ui": {"command": {"path": "cui", "args": ["mcp"]}}}}
  ```
- **Claude Code**: `claude mcp add colima-ui -- cui mcp` (or add the `mcpServers` block above to `.mcp.json`).

## 2. Session Port, Liveness & Authentication

ColimaUI binds to `127.0.0.1` on the first free port in `11420..=11429` and persists the active port and CSPRNG Bearer token under `~/.colima-ui/` with `0600` owner-only permissions.

> **Security Model**: Unauthenticated HTTP token-minting endpoints have been removed for security. `GET /api/health` is the **only** public endpoint. All REST endpoints require `Authorization: Bearer <TOKEN>`, while the two SSE streaming endpoints (`/api/events` and `/api/k8s/pods/logs/stream`) authenticate via the `?token=<TOKEN>` query parameter.

```bash
PORT=11420
[ -f ~/.colima-ui/api_port ] && read -r PORT < ~/.colima-ui/api_port
read -r TOKEN < ~/.colima-ui/api_token

# 1. Public liveness check (no token required)
curl -s "http://127.0.0.1:${PORT}/api/health"

# 2. Authenticated AI capability & Event Bus schema discovery
curl -s -H "Authorization: Bearer ${TOKEN}" \
  "http://127.0.0.1:${PORT}/api/capabilities"

# 3. Host external tool capability probes (colima, docker, kubectl, trivy + install hints)
curl -s -H "Authorization: Bearer ${TOKEN}" \
  "http://127.0.0.1:${PORT}/api/system/capabilities"
```

## 3. Headless AI CLI (`cui`) & Delegated AI Task Execution

Run multi-turn natural-language tasks from the terminal via `cui` (reads `app_settings` from `~/.colima-ui/knowledge.db`) or call `POST /api/cli/chat` directly:

```bash
# Read-only AI query (default)
cui "Check Colima default instance status and list running containers"

# Allow mutating actions (--yes), structured JSON output (--json), or override provider/model
cui --yes "Restart the exited redis container"
cui --json --provider anthropic --model claude-sonnet-4-6 "Summarize K8s pod health"
# Valid --provider IDs: anthropic, openai, gemini, openai-compatible, ollama-local, ollama-cloud

# Direct HTTP equivalent (returns {"result": "..."} or {"error": "..."})
curl -s -X POST "http://127.0.0.1:${PORT}/api/cli/chat" \
  -H "Authorization: Bearer ${TOKEN}" \
  -H "Content-Type: application/json" \
  -d '{"prompt": "Check Colima default instance status and list running containers", "allow_mutations": false}'
```

## 4. Safe Sandboxed Command Execution

Execute CLI commands through ColimaUI's shell sandbox policy engine (read-only commands auto-run; state-changing commands require explicit approval):

```bash
# Policy-classified execution (SAFE commands run immediately; others return approval requirement)
curl -s -X POST "http://127.0.0.1:${PORT}/api/sandbox/execute" \
  -H "Authorization: Bearer ${TOKEN}" \
  -H "Content-Type: application/json" \
  -d '{"command": "docker ps"}'

# Run an explicitly approved command after user confirmation
curl -s -X POST "http://127.0.0.1:${PORT}/api/sandbox/execute-approved" \
  -H "Authorization: Bearer ${TOKEN}" \
  -H "Content-Type: application/json" \
  -d '{"command": "docker context use colima"}'
```

## 5. Real-Time SSE Event Subscription

Subscribe to live state updates (`docker-state-updated`, `instances-update`), background transfer progress (`transfer.progress`, `transfer.done`, `transfer.failed`), security scan progress (`security-scan-progress`), detonation timelines (`detonation.event`, `detonation.status`), and metrics samples (`metrics.sample`) via Server-Sent Events (`?token=` query authentication, optional comma-separated `?topics=` filter):

```bash
curl -s -N "http://127.0.0.1:${PORT}/api/events?token=${TOKEN}&topics=instances-update,docker-state-updated,metrics.sample"
```

Live Kubernetes pod log streaming also uses `?token=` query authentication:

```bash
curl -s -N "http://127.0.0.1:${PORT}/api/k8s/pods/logs/stream?token=${TOKEN}&namespace=default&pod=web&tail=100"
```

## 6. Key REST API Domains

All JSON REST routes below require `-H "Authorization: Bearer ${TOKEN}"` (and `-H "Content-Type: application/json"` for `POST` bodies):

- **Instances, Worker Nodes, Presets & Colima Config**: `GET /api/instances`, `GET /api/instances/status?profile=default`, `POST /api/instances/start|stop|delete|k8s`, `POST /api/instances/create-worker`, `GET /api/instances/ssh`, `GET /api/instances/config`, `POST /api/instances/config/preview|apply`, `GET|POST /api/presets`, `POST /api/presets/delete`, `GET|POST /api/presets/snapshot`, `GET /api/presets/snapshots`
- **Containers, Images, Volumes, Networks & Transfers**:
  - Containers: `GET /api/containers`, `GET /api/containers/logs|inspect|stats|stats/all|top`, `POST /api/containers/start|stop|restart|remove|pause|unpause|rename|exec|run`
  - Images, Volumes, Networks: `GET /api/images`, `POST /api/images/pull|remove|prune|tag`, `GET /api/images/inspect`, `GET /api/volumes`, `POST /api/volumes/create|remove|prune`, `GET /api/volumes/inspect`, `GET /api/networks`, `POST /api/networks/create|remove|prune`, `GET /api/networks/inspect`, `GET /api/docker/df`, `POST /api/docker/prune`
  - Background Transfers: `POST /api/images/save|load`, `POST /api/containers/cp/to|from`, `GET /api/transfers`, `POST /api/transfers/cancel`
- **Compose, Diagnose & Autofix**: `GET /api/compose`, `POST /api/compose/up|down|restart|stop|start`, `GET /api/compose/logs|ps`, `POST /api/compose/diagnose`, `POST /api/compose/autofix/propose|apply|undo`, `GET /api/compose/autofix/history`
- **Kubernetes & Kind**:
  - Read: `GET /api/k8s/check`, `GET /api/k8s/cluster-health`, `GET /api/k8s/contexts`, `GET /api/k8s/contexts/current`, `GET /api/k8s/namespaces`, `GET /api/k8s/pods`, `GET /api/k8s/pods/logs`, `GET /api/k8s/pods/containers`, `GET /api/k8s/pods/container-logs`, `GET /api/k8s/services`, `GET /api/k8s/deployments`, `GET /api/k8s/nodes`, `GET /api/k8s/nodes/json`, `GET /api/k8s/events`, `GET /api/k8s/events/json`, `GET /api/k8s/resources`, `GET /api/k8s/describe`, `GET /api/k8s/resources/yaml`, `GET /api/k8s/crds`, `GET /api/k8s/crds/resources`, `GET /api/k8s/port-forward/list`
  - Mutate: `POST /api/k8s/contexts/set`, `POST /api/k8s/apply`, `POST /api/k8s/scale`, `POST /api/k8s/scale-generic`, `POST /api/k8s/pods/delete`, `POST /api/k8s/resources/delete`, `POST /api/k8s/resources/restart`, `POST /api/k8s/nodes/action`, `POST /api/k8s/port-forward/start`, `POST /api/k8s/port-forward/stop`, `POST /api/k8s/exec`, `POST /api/k8s/benchmark`
  - Kind: `GET /api/kind`, `POST /api/kind/create`, `POST /api/kind/delete`
- **Lima VMs & Local AI Models**: `GET /api/lima`, `GET /api/lima/info|templates`, `POST /api/lima/create|start|stop|delete|shell`, `GET /api/models`, `POST /api/models/pull|serve|delete`
- **Security (Trivy, SBOM, Falco & Detonation)**: `POST /api/security/scan|scan/cancel|sbom|audit|triage|autofix/propose`, `GET /api/security/history|rules|alternatives`, `GET|POST /api/security/policy`, `POST /api/security/policy/delete`, `GET|POST /api/security/watch`, `GET /api/security/falco/state|events`, `GET|POST /api/security/falco/watch`, `POST /api/security/falco/explain`, `POST /api/detonation/start|cancel|session|export`, `GET /api/detonation/list`
- **Topology**: `GET /api/topology`
- **Activity, Metrics, Alerts & Self-Heal**: `GET /api/activity`, `GET /api/activity/feed`, `POST /api/activity/export`, `POST /api/metrics/interval`, `GET /api/metrics/history|health`, `GET|POST /api/alerts/rules`, `POST /api/alerts/rules/delete`, `GET /api/alerts/events`, `POST /api/alerts/backtest`, `GET|POST /api/self-heal/rules`, `GET /api/self-heal/log`, `GET|POST /api/self-heal/enabled`
- **Knowledge Bank, AI, Diagnostics, Environment, Telemetry & System**: `POST /api/kb/query|feedback|learn|search`, `GET /api/kb/memories`, `POST /api/kb/memories/add|update|delete`, `GET /api/kb/articles|articles/get|articles/search`, `GET|POST /api/ai/conversations`, `POST /api/ai/conversations/rename|delete`, `GET|POST /api/ai/history`, `POST /api/ai/history/clear`, `POST /api/ai/chat|models|search|fetch-page`, `GET /api/ai/context`, `GET|POST /api/settings`, `GET /api/announcements`, `GET|POST /api/telemetry/consent`, `GET /api/telemetry/should-prompt|preview`, `GET /api/pro/status`, `GET /api/subscription`, `POST /api/subscription/store`, `GET /api/diagnostics/logs`, `POST /api/diagnostics/bundle|save`, `GET /api/environment/resolutions`, `GET /api/capabilities`, `GET /api/system/check|version|homebrew|check-tool|host-specs|platform|capabilities|runtime|engine-resources|df|reference`, `GET|POST /api/system/autostart`, `POST /api/system/install|prune|resource-saver`

## Anti-Patterns

- **Never call unauthenticated endpoints other than `/api/health`** — every protected route rejects missing `Authorization: Bearer <TOKEN>` headers with `401 Unauthorized`.
- **Never attempt to fetch tokens over HTTP** — there is no HTTP token-minting endpoint; read `~/.colima-ui/api_token` (`0600`) directly on the local host.
- **Never pass `Authorization` headers to SSE endpoints** — `/api/events` and `/api/k8s/pods/logs/stream` require `?token=<TOKEN>` in the query string and reject header-only auth.
- **Never hardcode port `11420` without checking `~/.colima-ui/api_port`** — when `11420` is occupied, ColimaUI binds the next free port in `11420..=11429` and records it in `~/.colima-ui/api_port`.
