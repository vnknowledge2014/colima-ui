---
name: colima-manager
version: 0.3.0
description: "Control and orchestrate ColimaUI v0.3.0 via CLI-First deterministic subcommands (cui <subcommand> --json, < 5ms), the 22-tool Native MCP Server (cui mcp), Native A2A JSON-RPC (/a2a), or the localhost HTTP/SSE API."
---

# Colima Manager Skill (`v0.3.0`)

This skill enables external orchestrators and IDE agents to drive **ColimaUI `v0.3.0`** across its **Dual-Surface Architecture**:
- **Inside App**: Desktop/Webview `AiChatPanel` + `agentCore.ts` + **100** App Events + Internal Skills (`system_prompt.md`, `router.md`, `src-tauri/resources/skills/*`) + SQLite Knowledge Bank RAG + FTS5 Memories + Live UI Bridge (`ui-navigate` & `agent-external-chat-turn`).
- **Outside App (`cui` `v0.3.0`, with `colimaui` preserved as a backward-compatible alias)**:
  1. **CLI-First Deterministic Subcommands (`< 5ms` over `:11420..11429` when Desktop is open, automatic standalone Rust fallback when Desktop is closed)** with global `--json` returning `{"ok": true, "data": ...}` or `{"ok": false, "error": "..."}`.
  2. **22-Tool Native MCP Server (`cui mcp` / `colimaui mcp`)** over `stdio`.
  3. **Agent Chat & Native HTTP A2A (`cui chat` / `POST /a2a`)** isolated to `colima-a2a-default-session` by default, with opt-in `--mirror-ui` live Desktop mirroring.

## 1. CLI-First Deterministic Subcommands (`cui <subcommand> --json`)

Always prefer direct `< 5ms` subcommands over LLM chat for deterministic operations (**22 subcommands** = 20 direct operational domains + `chat` + `mcp`):

- **Colima VMs, Config & Presets**:
  - `cui vm <list|status|start|stop|restart|delete|ssh|create-worker> --json`
  - `cui config <get|preview|apply> --json` (`colima.yaml` optimistic-locked edits)
  - `cui presets <list|save|delete|snapshots> --json`
- **Docker Containers, Images, Volumes & Networks**:
  - `cui container <list|logs|inspect|stats|top|start|stop|restart|pause|unpause|rename|remove|exec|run> --json`
  - `cui image <list|inspect|pull|tag|remove|prune|save|load> --json`
  - `cui volume <list|inspect|create|remove|prune> --json`
  - `cui network <list|inspect|create|remove|prune> --json`
- **Compose & AI Autofix**:
  - `cui compose <list|ps|logs|up|down|start|stop|restart|diagnose|autofix-propose|autofix-apply|autofix-undo|autofix-history> --json`
- **Kubernetes, Kind & Lima VMs**:
  - `cui k8s <status|contexts|use-context|namespaces|pods|deployments|services|nodes|events|resources|logs|describe|yaml|apply|scale|restart|delete|port-forward|exec|crds> --json`
  - `cui kind <list|create|delete> --json`
  - `cui lima <list|info|templates|create|start|stop|delete|shell> --json`
- **Security, Models, Alerts & Self-Heal**:
  - `cui security <scan|sbom|audit|triage|history|rules|alternatives|falco-state|falco-events|detonation-list> --json`
  - `cui models <list|pull|serve|delete> --json`
  - `cui alerts <rules|events> --json`
  - `cui self-heal <status|rules|log|enable|disable> --json`
- **Knowledge Bank, Sessions, UI Bridge, Events & Doctor**:
  - `cui memories` (alias `cui kb`) `<query|search|list|add|delete|learn|articles> --json`
  - `cui sessions <list|history|clear|delete> --json`
  - `cui ui navigate <page> [--section <sec>] [--toast <msg>] --json`
  - `cui events [--topics <csv> | --filter <topic>] --limit <N> --json`
  - `cui doctor --json`

## 2. Native MCP Server (`cui mcp` / `colimaui mcp` — 22 Typed Tools)

ColimaUI ships a built-in JSON-RPC 2.0 MCP server over `stdin`/`stdout` (`cui mcp` or `colimaui mcp`). It proxies via `127.0.0.1:<api_port>` + `~/.colima-ui/api_token` when ColimaUI Desktop is running, or executes directly via standalone native fallback when Desktop is closed.

**22 Typed MCP Tools**:
1. `colima_list_instances`
2. `colima_instance_action`
3. `docker_list_containers`
4. `docker_container_action`
5. `docker_container_logs`
6. `docker_list_images`
7. `docker_pull_image`
8. `docker_list_volumes`
9. `docker_list_networks`
10. `compose_list_projects`
11. `k8s_list_pods`
12. `k8s_cluster_status`
13. `environment_resolutions`
14. `system_diagnostics`
15. `colima_ai_ask` (`session_id` defaults to `"colima-a2a-default-session"`, supports `allow_actions` and `mirror_ui`)
16. `security_scan_image`
17. `compose_diagnose_autofix`
18. `lima_manage`
19. `colima_config`
20. `kb_memories`
21. `chat_sessions`
22. `ui_navigate`

**Host Configuration**:
- **Claude Desktop** (`~/Library/Application Support/Claude/claude_desktop_config.json`), **Cursor** (`~/.cursor/mcp.json`), **Antigravity** (`~/.gemini/antigravity/mcp_config.json`), **Windsurf** (`~/.codeium/windsurf/mcp_config.json`):
  ```json
  {"mcpServers": {"colima-ui": {"command": "cui", "args": ["mcp"]}}}
  ```
- **Zed** (`~/.config/zed/settings.json`):
  ```json
  {"context_servers": {"colima-ui": {"command": {"path": "cui", "args": ["mcp"]}}}}
  ```
- **Claude Code**: `claude mcp add colima-ui -- cui mcp`

## 3. Session Port, Liveness & Authentication

ColimaUI binds to `127.0.0.1` on the first free port in `11420..=11429` and persists the active port and CSPRNG Bearer token under `~/.colima-ui/` with `0600` owner-only permissions.

> **Security Model**: `GET /api/health` is the **only** public unauthenticated endpoint. All REST routes, UI Remote Control routes (`/api/ui/*`), and Native A2A routes (`GET /.well-known/agent.json`, `POST /a2a`) require `Authorization: Bearer <TOKEN>`. Only the two SSE streaming endpoints (`/api/events` and `/api/k8s/pods/logs/stream`) authenticate via `?token=<TOKEN>`.

```bash
PORT=11420
[ -f ~/.colima-ui/api_port ] && read -r PORT < ~/.colima-ui/api_port
read -r TOKEN < ~/.colima-ui/api_token

# 1. Public liveness check (no token required)
curl -s "http://127.0.0.1:${PORT}/api/health"

# 2. Live UI state & remote UI navigation bridge
curl -s -H "Authorization: Bearer ${TOKEN}" "http://127.0.0.1:${PORT}/api/ui/state"
curl -s -X POST "http://127.0.0.1:${PORT}/api/ui/navigate" \
  -H "Authorization: Bearer ${TOKEN}" -H "Content-Type: application/json" \
  -d '{"page": "kubernetes", "section": "pods", "toast": "Inspecting cluster"}'

# 3. Authenticated AI capability, host tool probes & internal skill references
curl -s -H "Authorization: Bearer ${TOKEN}" "http://127.0.0.1:${PORT}/api/capabilities"
curl -s -H "Authorization: Bearer ${TOKEN}" "http://127.0.0.1:${PORT}/api/system/capabilities"
curl -s -H "Authorization: Bearer ${TOKEN}" "http://127.0.0.1:${PORT}/api/ai/reference?path=skills/colima_ui/SKILL.md"
```

## 4. Agent Chat, A2A Session Isolation & Native A2A Protocol

Automated CLI (`cui chat`), MCP (`colima_ai_ask`), and HTTP A2A (`/a2a`) turns default to `colima-a2a-default-session` so background tasks never hijack or pollute the user's personal Desktop UI chat history. Pass `--mirror-ui` (`"mirror_ui": true`) when live mirroring into `AiChatPanel` (via `agent-external-chat-turn`) is desired:

```bash
# Headless isolated A2A session (read-only by default; --yes enables mutations)
cui chat "Check Colima default instance status and summarize unhealthy containers" --session colima-a2a-default-session --json
cui chat "Restart the exited redis container" --session colima-a2a-default-session --yes --mirror-ui --json
# Shorthand alias (also defaults to colima-a2a-default-session):
cui "Summarize K8s pod health" --provider anthropic --model claude-sonnet-4-6 --json
# Valid --provider IDs: anthropic, openai, gemini, openai-compatible, ollama-local, ollama-cloud

# Native A2A Discovery & JSON-RPC 2.0 (`message/send`, `tasks/send`, `tasks/get`, `tasks/cancel`)
curl -s -H "Authorization: Bearer ${TOKEN}" "http://127.0.0.1:${PORT}/.well-known/agent.json"
curl -s -X POST "http://127.0.0.1:${PORT}/a2a" \
  -H "Authorization: Bearer ${TOKEN}" \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"message/send","params":{"sessionId":"colima-a2a-default-session","message":{"role":"user","parts":[{"type":"text","text":"Check Colima default VM status"}]}}}'
```

## 5. Safe Sandboxed Command Execution & Real-Time SSE

```bash
# Policy-classified execution (SAFE commands run immediately; others return approval requirement)
curl -s -X POST "http://127.0.0.1:${PORT}/api/sandbox/execute" \
  -H "Authorization: Bearer ${TOKEN}" -H "Content-Type: application/json" \
  -d '{"command": "docker ps"}'

# Subscribe to real-time SSE topics (including ui-navigate & agent-external-chat-turn)
curl -s -N "http://127.0.0.1:${PORT}/api/events?token=${TOKEN}&topics=instances-update,docker-state-updated,ui-navigate"
```

## 6. Key REST & Protocol Domains

All JSON routes below require `-H "Authorization: Bearer ${TOKEN}"`:
- **Native A2A & UI Remote Control Bridge**: `GET /.well-known/agent.json`, `POST /a2a` (`message/send`, `tasks/send`, `tasks/get`, `tasks/cancel`), `GET /api/ui/state`, `POST /api/ui/navigate`, `GET /api/ai/reference?path=...`, `POST /api/cli/chat`
- **Instances, Worker Nodes, Presets & Colima Config**: `GET /api/instances`, `GET /api/instances/status?profile=default`, `POST /api/instances/start|stop|delete|k8s|create-worker`, `GET /api/instances/ssh|config`, `POST /api/instances/config/preview|apply`, `GET|POST /api/presets`, `POST /api/presets/delete`, `GET|POST /api/presets/snapshot`, `GET /api/presets/snapshots`
- **Containers, Images, Volumes, Networks, Topology & Transfers**: `GET /api/containers`, `GET /api/containers/logs|inspect|stats|stats/all|top`, `POST /api/containers/start|stop|restart|remove|pause|unpause|rename|exec|run`, `GET /api/images`, `POST /api/images/pull|remove|prune|tag|save|load`, `GET /api/images/inspect`, `GET /api/volumes`, `POST /api/volumes/create|remove|prune`, `GET /api/volumes/inspect`, `GET /api/networks`, `POST /api/networks/create|remove|prune`, `GET /api/networks/inspect`, `GET /api/docker/df`, `POST /api/docker/prune`, `GET /api/topology`, `POST /api/containers/cp/to|from`, `GET /api/transfers`, `POST /api/transfers/cancel`
- **Compose, Diagnose & Autofix**: `GET /api/compose`, `POST /api/compose/up|down|restart|stop|start|diagnose`, `GET /api/compose/logs|ps`, `POST /api/compose/autofix/propose|apply|undo`, `GET /api/compose/autofix/history`
- **Kubernetes, Kind, Lima VMs & Local AI Models**: `GET /api/k8s/check|cluster-health|contexts|contexts/current|namespaces|pods|pods/logs|pods/containers|pods/container-logs|services|deployments|nodes|nodes/json|events|events/json|resources|describe|resources/yaml|crds|crds/resources|port-forward/list`, `POST /api/k8s/contexts/set|apply|scale|scale-generic|pods/delete|resources/delete|resources/restart|nodes/action|port-forward/start|port-forward/stop|exec|benchmark`, `GET /api/kind`, `POST /api/kind/create|delete`, `GET /api/lima`, `GET /api/lima/info|templates`, `POST /api/lima/create|start|stop|delete|shell`, `GET /api/models`, `POST /api/models/pull|serve|delete`
- **Security, Observability, Knowledge Bank & System**: `POST /api/security/scan|scan/cancel|sbom|audit|triage|autofix/propose`, `GET /api/security/history|rules|alternatives`, `GET|POST /api/security/policy|watch`, `POST /api/security/policy/delete`, `GET /api/security/falco/state|events`, `GET|POST /api/security/falco/watch`, `POST /api/security/falco/explain`, `POST /api/detonation/start|cancel|session|export`, `GET /api/detonation/list`, `GET /api/activity`, `GET /api/activity/feed`, `POST /api/activity/export`, `POST /api/metrics/interval`, `GET /api/metrics/history|health`, `GET|POST /api/alerts/rules`, `POST /api/alerts/rules/delete`, `GET /api/alerts/events`, `POST /api/alerts/backtest`, `GET|POST /api/self-heal/rules|enabled`, `GET /api/self-heal/log`, `POST /api/kb/query|feedback|learn|search`, `GET /api/kb/memories`, `POST /api/kb/memories/add|update|delete`, `GET /api/kb/articles|articles/get|articles/search`, `GET|POST /api/ai/conversations|history`, `POST /api/ai/conversations/rename|delete`, `POST /api/ai/history/clear`, `POST /api/ai/chat|models|search|fetch-page`, `GET /api/ai/context|reference`, `GET|POST /api/settings`, `GET /api/diagnostics/logs`, `POST /api/diagnostics/bundle|save`, `GET /api/environment/resolutions`, `GET /api/capabilities`, `GET /api/system/check|version|homebrew|check-tool|host-specs|platform|capabilities|runtime|engine-resources|df|reference`, `GET|POST /api/system/autostart`, `POST /api/system/install|prune|resource-saver`

## Anti-Patterns

- **Never call `cui chat` for deterministic operations** — use `< 5ms` direct subcommands (`cui vm`, `cui container`, `cui k8s`, `cui compose`, `cui security`, `cui ui`, etc.) with `--json`.
- **Never omit `--json` in scripts or automated workflows** — human-readable tables break JSON parsers.
- **Never run `cui events` without `--limit <N>`** in automation — unbounded SSE streams hang forever.
- **Never omit `--session colima-a2a-default-session` in automated background calls** — avoid polluting the user's personal Desktop UI chat history unless `--mirror-ui` is explicitly requested.
- **Never call unauthenticated endpoints other than `/api/health`** or attempt to fetch tokens over HTTP — read `~/.colima-ui/api_token` (`0600`) directly and check `~/.colima-ui/api_port` (`11420..=11429`).
- **Never pass `Authorization` headers to SSE endpoints** — `/api/events` and `/api/k8s/pods/logs/stream` require `?token=<TOKEN>` in the query string.
