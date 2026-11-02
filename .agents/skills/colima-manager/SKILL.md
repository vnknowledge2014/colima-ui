---
name: colima-manager
description: Control and orchestrate Colima UI via HTTP API capabilities
---

# Colima Manager Skill

This skill enables external orchestrators and IDE agents to drive a running **ColimaUI** desktop backend safely via its localhost HTTP API.

## 1. Session Port, Liveness & Authentication

ColimaUI binds to `127.0.0.1` on the first free port in `11420..=11429` and persists the active port and CSPRNG Bearer token under `~/.colima-ui/` with `0600` owner-only permissions.

> **Security Model**: Unauthenticated HTTP token-minting endpoints have been removed for security. `GET /api/health` is the **only** public endpoint. All REST endpoints require `Authorization: Bearer <TOKEN>`, while the two SSE streaming endpoints (`/api/events` and `/api/k8s/pods/logs/stream`) authenticate via the `?token=<TOKEN>` query parameter.

```bash
PORT=11420
[ -f ~/.colima-ui/api_port ] && read -r PORT < ~/.colima-ui/api_port
read -r TOKEN < ~/.colima-ui/api_token

# 1. Public liveness check (no token required)
curl -s "http://127.0.0.1:${PORT}/api/health"

# 2. Authenticated capability discovery
curl -s -H "Authorization: Bearer ${TOKEN}" \
  "http://127.0.0.1:${PORT}/api/capabilities"
```

## 2. Delegated AI Task Execution

Send a natural-language objective to ColimaUI's headless AI agent loop (`POST /api/cli/chat`), which returns `{"result": "..."}` (or `{"error": "..."}`):

```bash
curl -s -X POST "http://127.0.0.1:${PORT}/api/cli/chat" \
  -H "Authorization: Bearer ${TOKEN}" \
  -H "Content-Type: application/json" \
  -d '{"prompt": "Check Colima default instance status and list running containers"}'
```

## 3. Safe Sandboxed Command Execution

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

## 4. Real-Time SSE Event Subscription

Subscribe to live state updates, background transfer progress, security scan progress, detonation timelines, and metrics samples via Server-Sent Events (`?token=` query authentication, optional comma-separated `?topics=` filter):

```bash
curl -s -N "http://127.0.0.1:${PORT}/api/events?token=${TOKEN}&topics=docker-state,instances"
```

Live Kubernetes pod log streaming also uses `?token=` query authentication:

```bash
curl -s -N "http://127.0.0.1:${PORT}/api/k8s/pods/logs/stream?token=${TOKEN}&namespace=default&pod=web&tail=100"
```

## 5. Key REST API Domains

All JSON REST routes below require `-H "Authorization: Bearer ${TOKEN}"` (and `-H "Content-Type: application/json"` for `POST` bodies):

- **Instances & Colima Config**: `GET /api/instances`, `GET /api/instances/status?profile=default`, `POST /api/instances/start|stop|delete|k8s`, `GET /api/instances/ssh`, `GET /api/instances/config`, `POST /api/instances/config/preview|apply`
- **Containers, Images, Volumes, Networks & Transfers**:
  - Containers: `GET /api/containers`, `GET /api/containers/logs|inspect|stats|stats/all|top`, `POST /api/containers/start|stop|restart|remove|pause|unpause|rename|exec|run`
  - Images, Volumes, Networks: `GET /api/images`, `POST /api/images/pull|remove|prune|tag`, `GET /api/images/inspect`, `GET /api/volumes`, `POST /api/volumes/create|remove|prune`, `GET /api/volumes/inspect`, `GET /api/networks`, `POST /api/networks/create|remove|prune`, `GET /api/networks/inspect`, `GET /api/docker/df`, `POST /api/docker/prune`
  - Background Transfers: `POST /api/images/save|load`, `POST /api/containers/cp/to|from`, `GET /api/transfers`, `POST /api/transfers/cancel`
- **Compose, Diagnose & Autofix**: `GET /api/compose`, `POST /api/compose/up|down|restart|stop|start`, `GET /api/compose/logs|ps`, `POST /api/compose/diagnose`, `POST /api/compose/autofix/propose|apply|undo`, `GET /api/compose/autofix/history`
- **Kubernetes & Kind**: `GET /api/k8s/check|cluster-health|contexts|contexts/current|namespaces|pods|services|deployments|nodes|events|resources|describe|resources/yaml|crds|crds/resources|port-forward/list`, `POST /api/k8s/contexts/set|apply|scale|scale-generic|pods/delete|resources/delete|resources/restart|nodes/action|port-forward/start|port-forward/stop|exec|benchmark`, `GET /api/kind`, `POST /api/kind/create|delete`
- **Lima VMs**: `GET /api/lima`, `GET /api/lima/info|templates`, `POST /api/lima/create|start|stop|delete|shell`
- **Security (Trivy, SBOM, Falco & Detonation)**: `POST /api/security/scan|scan/cancel|sbom|audit|triage|autofix/propose`, `GET /api/security/history|rules|alternatives`, `GET|POST /api/security/policy`, `POST /api/security/policy/delete`, `GET|POST /api/security/watch`, `GET /api/security/falco/state|events`, `GET|POST /api/security/falco/watch`, `POST /api/security/falco/explain`, `POST /api/detonation/start|cancel|session|export`, `GET /api/detonation/list`
- **Topology**: `GET /api/topology`
- **Activity, Metrics, Alerts & Self-Heal**: `GET /api/activity`, `GET /api/activity/feed`, `POST /api/activity/export`, `POST /api/metrics/interval`, `GET /api/metrics/history|health`, `GET|POST /api/alerts/rules`, `POST /api/alerts/rules/delete`, `GET /api/alerts/events`, `POST /api/alerts/backtest`, `GET|POST /api/self-heal/rules`, `GET /api/self-heal/log`, `GET|POST /api/self-heal/enabled`
- **Knowledge Bank, AI, Diagnostics & System**: `POST /api/kb/query|feedback|learn|search`, `GET /api/kb/memories`, `POST /api/kb/memories/add|update|delete`, `GET /api/kb/articles|articles/get|articles/search`, `GET|POST /api/ai/conversations`, `POST /api/ai/conversations/rename|delete`, `GET|POST /api/ai/history`, `POST /api/ai/history/clear`, `POST /api/ai/chat|models|search|fetch-page`, `GET /api/ai/context`, `GET|POST /api/settings`, `GET /api/diagnostics/logs`, `POST /api/diagnostics/bundle|save`, `GET /api/system/check|version|host-specs|platform|capabilities|df|reference`

## Anti-Patterns

- **Never call unauthenticated endpoints other than `/api/health`** — every protected route rejects missing `Authorization: Bearer <TOKEN>` headers with `401 Unauthorized`.
- **Never attempt to fetch tokens over HTTP** — there is no HTTP token-minting endpoint; read `~/.colima-ui/api_token` (`0600`) directly on the local host.
- **Never pass `Authorization` headers to SSE endpoints** — `/api/events` and `/api/k8s/pods/logs/stream` require `?token=<TOKEN>` in the query string and reject header-only auth.
- **Never hardcode port `11420` without checking `~/.colima-ui/api_port`** — when `11420` is occupied, ColimaUI binds the next free port in `11420..=11429` and records it in `~/.colima-ui/api_port`.
