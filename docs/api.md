# HTTP API Reference

ColimaUI runs a localhost Axum HTTP & SSE server on `127.0.0.1:11420` (automatically falling back across `11420–11429` if port `11420` is occupied) for the `cui` CLI, local host AI agents, and development-mode browser debugging.

> **No static web frontend on `:11420`:** Release builds do not mount static assets on the API server. Opening `http://127.0.0.1:11420` directly in a browser returns `401 Unauthorized`.

---

## Authentication

### Public Endpoint (Unauthenticated)

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/api/health` | Liveness probe (`"ok"`), left open so clients can scan `11420–11429` before sending credentials |

`/api/health` is the **only** unauthenticated route. There is **no** `/api/auth/token` endpoint.

### Protected Endpoints

- **Standard REST routes**: Require `Authorization: Bearer <token>`, verified in constant time (`auth::constant_time_eq`).
- **SSE `EventSource` allowlist (`auth::QUERY_TOKEN_PATHS`)**: Because browser `EventSource` cannot set custom HTTP headers, **only** `/api/events` and `/api/k8s/pods/logs/stream` authenticate via the parsed `?token=<token>` query parameter (and reject header auth so the two mechanisms never overlap).

### Obtaining `<token>`

| Client | Handoff Mechanism |
|--------|-------------------|
| **Desktop webview (Tauri)** | Calls the `api_token` Tauri IPC command (needed for `EventSource` streams; regular actions use IPC commands directly). |
| **`cui` CLI & local host agents** | Reads `~/.colima-ui/api_token` (`0600` permissions inside `0700` `~/.colima-ui`), paired with `~/.colima-ui/api_port`. Written atomically on startup and removed on app exit. |
| **Development browser UI** | Debug builds (`npm run tauri dev`) print `Browser mode: http://localhost:1420/#token=<token>` to stdout. Vite consumes the URL fragment once and strips it from the address bar. |

---

## SSE Events

| Method | Endpoint | Auth | Description |
|--------|----------|------|-------------|
| `GET` | `/api/events` | `?token=<token>` | Server-Sent Events stream (`?topics=metrics.sample,...`) |

Common event topics: `docker-state`, `instances`, `docker-event`, `metrics.sample`, `transfer.progress`, `transfer.done`, `transfer.failed`, `security-scan-progress`, `detonation.event`, `detonation.status`.

---

## System, Capabilities & Settings

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/api/system/check` | Installed versions of `colima`, `docker`, and `limactl` (5-min cache) |
| `GET` | `/api/system/version` | Cached Colima version string |
| `GET` | `/api/system/homebrew` | Homebrew installation status |
| `GET` | `/api/system/check-tool` | Check whitelisted tool (`?name=kubectl\|kind\|helm\|krunkit\|nerdctl`) |
| `GET` | `/api/system/capabilities` | Host runtime/OS capability summary |
| `GET` | `/api/capabilities` | Feature & external tool capability probes (e.g. `trivy`) with install hints |
| `GET` | `/api/system/platform` | OS, CPU architecture, and package manager detection |
| `GET` | `/api/system/host-specs` | Host CPU core count and total memory |
| `GET` | `/api/system/engine-resources` | Aggregate CPU, memory, and disk figures for the active container engine |
| `POST` | `/api/system/install` | Install dependency (`{name, method}`) |
| `GET` | `/api/system/autostart` | Read launch-at-login status |
| `POST` | `/api/system/autostart` | Configure launch-at-login (`{enable}`) |
| `POST` | `/api/system/prune` | Docker system prune (`?all=true`) |
| `GET` | `/api/system/df` | Docker disk usage |
| `GET` | `/api/system/reference` | Read bundled reference markdown (`?path=skills/colima_ui/SKILL.md`, path-confined to `resources/`) |
| `GET` | `/api/announcements` | Static release notes & security advisories feed (fetched backend-side with zero user identifiers; see `docs/telemetry.md`) |
| `GET` | `/api/settings` | Get all persisted app settings |
| `POST` | `/api/settings` | Upsert an app setting (`{key, value}`) |
| `GET` | `/api/telemetry/consent` | Read current telemetry consent state |
| `POST` | `/api/telemetry/consent` | Update telemetry consent (`{granted}`) |
| `GET` | `/api/telemetry/should-prompt` | Whether the one-time telemetry consent prompt should appear |
| `GET` | `/api/telemetry/preview` | Inspect buffered in-memory telemetry events |

---

## Colima Instances & YAML Config

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/api/instances` | List all Colima instances (fast XDG / `~/.colima` YAML reader) |
| `POST` | `/api/instances/start` | Start or create instance (JSON: `StartConfig`) |
| `POST` | `/api/instances/stop` | Stop instance (`?profile=default&force=false`) |
| `POST` | `/api/instances/delete` | Delete instance (`?profile=default&force=false`) |
| `GET` | `/api/instances/status` | Instance runtime status (`?profile=default`) |
| `GET` | `/api/instances/ssh` | Get SSH connection command (`?profile=default`) |
| `POST` | `/api/instances/k8s` | Instance Kubernetes control (`?profile=default&action=start\|stop\|reset`) |
| `GET` | `/api/instances/config` | Read structured `colima.yaml` snapshot & `mtime` (`?profile=default`) |
| `POST` | `/api/instances/config/preview` | Preview `colima.yaml` diff & restart requirements (`{profile, changes}`) |
| `POST` | `/api/instances/config/apply` | Apply `colima.yaml` edits guarded by optimistic lock (`{profile, changes, expectedMtime}`) |

---

## Docker Containers, Images, Volumes & Networks

### Containers

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/api/containers` | List containers (`?all=false`) |
| `POST` | `/api/containers/start` | Start container (`?containerId=...`) |
| `POST` | `/api/containers/stop` | Stop container (`?containerId=...`) |
| `POST` | `/api/containers/restart` | Restart container (`?containerId=...`) |
| `POST` | `/api/containers/remove` | Remove container (`?containerId=...&force=false`) |
| `GET` | `/api/containers/logs` | Container logs (`?containerId=...&lines=100`) |
| `GET` | `/api/containers/inspect` | Inspect container (`?containerId=...`) |
| `GET` | `/api/containers/stats` | Single container stats (`?containerId=...`) |
| `GET` | `/api/containers/stats/all` | All running container stats |
| `GET` | `/api/containers/top` | Container processes (`?containerId=...`) |
| `POST` | `/api/containers/exec` | Exec command in container (`{containerId, command}`) |
| `POST` | `/api/containers/run` | Run new container (`{image, name, ports, env, ...}`) |
| `POST` | `/api/containers/rename` | Rename container (`{containerId, newName}`) |
| `POST` | `/api/containers/pause` | Pause container (`?containerId=...`) |
| `POST` | `/api/containers/unpause` | Unpause container (`?containerId=...`) |

### Images

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/api/images` | List local images |
| `POST` | `/api/images/remove` | Remove image (`?imageId=...&force=false`) |
| `POST` | `/api/images/pull` | Pull image (`?imageName=...`) |
| `POST` | `/api/images/prune` | Prune unused images |
| `GET` | `/api/images/inspect` | Inspect image (`?imageId=...`) |
| `POST` | `/api/images/tag` | Tag image (`{imageId, newTag}`) |

### Volumes & Networks

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/api/volumes` | List volumes |
| `POST` | `/api/volumes/create` | Create volume (`{name, driver}`) |
| `POST` | `/api/volumes/remove` | Remove volume (`?name=...&force=false`) |
| `POST` | `/api/volumes/prune` | Prune unused volumes |
| `GET` | `/api/volumes/inspect` | Inspect volume (`?name=...`) |
| `GET` | `/api/networks` | List networks |
| `POST` | `/api/networks/create` | Create network (`{name, driver, subnet}`) |
| `POST` | `/api/networks/remove` | Remove network (`?name=...`) |
| `GET` | `/api/networks/inspect` | Inspect network (`?name=...`) |
| `POST` | `/api/networks/prune` | Prune unused networks |
| `GET` | `/api/docker/df` | Docker disk usage |
| `POST` | `/api/docker/prune` | Full system prune (`?confirm=true` required) |

---

## Topology & Background Transfers

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/api/topology` | Docker graph (`{nodes, edges, warnings}` across `container`, `network`, `volume`, `project`, `image`) |
| `POST` | `/api/images/save` | Export images to an uncompressed `.tar` (`{images[], destDir, fileName, overwrite?}`) |
| `POST` | `/api/images/load` | Import images from a `.tar` archive (`{tarPath}`) |
| `POST` | `/api/containers/cp/to` | Copy host path into container (`{containerId, hostPath, containerPath}`) |
| `POST` | `/api/containers/cp/from` | Copy container path out **as a `.tar` archive** (`{containerId, containerPath, destDir, fileName, overwrite?}`) |
| `GET` | `/api/transfers` | Authoritative list of active and recently finished (`~60s`) transfer jobs |
| `POST` | `/api/transfers/cancel` | Cancel an in-flight transfer (`{jobId}`) → `cancelled \| alreadyFinished \| unknownJob` |

Transfer jobs return `{jobId, totalEstimate}` immediately and stream `transfer.progress`, `transfer.done`, and `transfer.failed` over SSE. Because SSE drops frames on slow consumers, callers reconcile against `GET /api/transfers` on connect and reconnect. Exports write to a `.part` sibling confined to `destDir` (`../` rejected) and atomically rename onto `fileName` (`.tar` required) only on success.

---

## Docker Compose & Autofix

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/api/compose` | List Compose projects |
| `POST` | `/api/compose/up` | Run `compose up` (`{projectDir, detach}`) |
| `POST` | `/api/compose/down` | Run `compose down` (`{projectName}`) |
| `POST` | `/api/compose/start` | Start stopped project services (`{projectName}`) |
| `POST` | `/api/compose/stop` | Stop running project services (`{projectName}`) |
| `POST` | `/api/compose/restart` | Restart project services (`{projectName}`) |
| `GET` | `/api/compose/logs` | Project logs (`?projectName=...&lines=100`) |
| `GET` | `/api/compose/ps` | Project container status (`?projectName=...`) |
| `POST` | `/api/compose/diagnose` | Static schema, reference & YAML diagnosis (`{filePath}`) |
| `POST` | `/api/compose/autofix/propose` | Propose candidate YAML fixes (`{filePath}`) |
| `POST` | `/api/compose/autofix/apply` | Apply reviewed Compose patch (`{filePath, newContent, explanation, declaredRemovals}`) |
| `POST` | `/api/compose/autofix/undo` | Revert a recorded Compose autofix (`{fixId}`) |
| `GET` | `/api/compose/autofix/history` | List applied/reverted Compose autofix records |

---

## Security, Falco & Detonation

### Image Scanning, Policy & Remediation

| Method | Endpoint | Description |
|--------|----------|-------------|
| `POST` | `/api/security/scan` | Scan local image with host Trivy (`{scanId, imageRef, refresh?}`), cached by image digest |
| `POST` | `/api/security/scan/cancel` | Cancel running scan (`{scanId}`) |
| `POST` | `/api/security/sbom` | Export CycloneDX or SPDX-JSON SBOM atomically (`{imageRef, destDir, fileName, format, overwrite?}`) |
| `POST` | `/api/security/audit` | Unified vulnerability scan + configuration rules + `/100` provenance score (`{scanId, imageRef, level?, refresh?}`) |
| `POST` | `/api/security/triage` | Rank failing rules by score impact (`{audit, level?}`) |
| `POST` | `/api/security/autofix/propose` | Propose Dockerfile hardening patches (`{dockerfilePath}`) |
| `GET` | `/api/security/history` | Historical scan runs for an image digest (`?imageDigest=...`) |
| `GET` | `/api/security/policy` | List configured security policy rules |
| `POST` | `/api/security/policy` | Upsert a security policy rule (`PolicyRule`) |
| `POST` | `/api/security/policy/delete` | Delete a security policy rule (`{id}`) |
| `GET` | `/api/security/watch` | Read scheduled background rescan state |
| `POST` | `/api/security/watch` | Configure scheduled background rescans (`{enabled?, intervalHours?}`) |
| `GET` | `/api/security/rules` | Return compiled configuration rule pack (`L1`–`L3`) |
| `GET` | `/api/security/alternatives` | Suggest hardened base images from the compiled local catalog (`?image=...`; zero outbound queries) |

`/api/security/audit` returns a 100-point breakdown (**vulnerabilities /40**, **hardening /25**, **provenance /20**, **freshness /15**) alongside `inputs` (scanner version, DB date, rule pack version, strictness level). Images are never pulled automatically, and image names never leave the machine.

### Falco Runtime Stream & Container Detonation

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/api/security/falco/state` | Detect Falco availability inside the VM |
| `GET` | `/api/security/falco/events` | Read stored Falco runtime alerts (`?limit=200`) |
| `GET` | `/api/security/falco/watch` | Check whether the Falco event reader is active |
| `POST` | `/api/security/falco/watch` | Start or stop the Falco runtime stream (`{enabled}`) |
| `POST` | `/api/security/falco/explain` | Build a redacted LLM triage payload for selected Falco events (`{eventIds}`) |
| `POST` | `/api/detonation/start` | Launch an isolated container detonation session (`DetonationConfig`); streams timeline on SSE |
| `POST` | `/api/detonation/cancel` | Stop and tear down a detonation session (`{sessionId}`) |
| `POST` | `/api/detonation/session` | Fetch snapshot of a detonation session (`{sessionId}`) |
| `GET` | `/api/detonation/list` | List detonation sessions |
| `POST` | `/api/detonation/export` | Export detonation report to disk (`{sessionId, dir, filename}`) |

---

## Live Metrics, Alerts, Self-Heal, Activity & Diagnostics

### Live Metrics & Alerts

| Method | Endpoint | Description |
|--------|----------|-------------|
| `POST` | `/api/metrics/interval` | Set Bollard sampling interval in ms (`{ms}`, clamped `1000–60000`) |
| `GET` | `/api/metrics/history` | Query persisted metric series (`?from=...&to=...&containers=id1,id2`) |
| `GET` | `/api/metrics/health` | Metric writer status and dropped-batch counter |
| `GET` | `/api/alerts/rules` | List metric alert rules |
| `POST` | `/api/alerts/rules` | Create or update an alert rule (`AlertRule`) |
| `POST` | `/api/alerts/rules/delete` | Delete an alert rule (`{id}`) |
| `GET` | `/api/alerts/events` | Recent fired alert events (`?limit=50`) |
| `POST` | `/api/alerts/backtest` | Backtest an alert rule against historical metrics (`{rule, fromMs, toMs}`) |

Live metric samples stream over `GET /api/events?token=<token>&topics=metrics.sample`; sampling runs only while `subscriber_count("metrics.sample") > 0`.

### Self-Heal, Activity & Diagnostics

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/api/self-heal/rules` | List self-healing rules |
| `POST` | `/api/self-heal/rules` | Update rule thresholds/mode (`{id, mode, threshold, maxPerHour, enabled}`) |
| `GET` | `/api/self-heal/log` | Recent self-healing actions & suggestions (`?limit=50`) |
| `GET` | `/api/self-heal/enabled` | Read global self-healing kill switch |
| `POST` | `/api/self-heal/enabled` | Toggle global self-healing kill switch (`{on}`) |
| `GET` | `/api/activity` | Query recorded machine mutations (`ActivityFilter` query params) |
| `GET` | `/api/activity/feed` | Merged timeline across activity, transfers, alerts, scans, and self-heal |
| `POST` | `/api/activity/export` | Export activity timeline as JSON or CSV (`{destDir, fileName, format, overwrite?}`) |
| `POST` | `/api/diagnostics/bundle` | Build redacted diagnostic bundle (`{error?, containerId?, logLines?}`); nothing is transmitted |
| `POST` | `/api/diagnostics/save` | Re-redact and write selected diagnostic sections to Markdown (`{bundle, include[], destDir, fileName, overwrite?}`) |
| `GET` | `/api/diagnostics/logs` | Collect raw Lima/Colima VM diagnostic report (`?profile=default`) |
| `GET` | `/api/environment/resolutions` | Inspect resolved Colima/Lima/Docker/Kubeconfig paths with provenance rules and `launchd`-vs-shell environment duality |

---

## Kubernetes & Kind Clusters

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/api/k8s/check` | Check `kubectl` connectivity |
| `GET` | `/api/k8s/namespaces` | List namespaces |
| `GET` | `/api/k8s/pods` | List pods (`?namespace=default`) |
| `GET` | `/api/k8s/services` | List services (`?namespace=default`) |
| `GET` | `/api/k8s/deployments` | List deployments (`?namespace=default`) |
| `GET` | `/api/k8s/nodes` | List nodes (text) |
| `GET` | `/api/k8s/nodes/json` | List nodes (JSON) |
| `GET` | `/api/k8s/events` | List events (text, `?namespace=default`) |
| `GET` | `/api/k8s/events/json` | List events (JSON) |
| `GET` | `/api/k8s/resources` | List resources by kind (`?kind=...&namespace=...`) |
| `GET` | `/api/k8s/pods/logs` | Pod logs (`?name=...&namespace=...&lines=100`) |
| `GET` | `/api/k8s/pods/logs/stream` | Stream pod logs via SSE (authenticates via `?token=<token>`) |
| `GET` | `/api/k8s/pods/containers` | List containers in a pod |
| `GET` | `/api/k8s/pods/container-logs` | Container-specific pod logs |
| `POST` | `/api/k8s/pods/delete` | Delete pod |
| `GET` | `/api/k8s/describe` | Describe resource |
| `GET` | `/api/k8s/resources/yaml` | Fetch resource YAML |
| `POST` | `/api/k8s/scale` | Scale deployment |
| `POST` | `/api/k8s/scale-generic` | Scale any scalable resource |
| `POST` | `/api/k8s/resources/delete` | Delete resource |
| `POST` | `/api/k8s/resources/restart` | Rollout restart resource |
| `POST` | `/api/k8s/apply` | Apply manifest YAML |
| `POST` | `/api/k8s/exec` | One-shot `kubectl exec` |
| `POST` | `/api/k8s/nodes/action` | Node action (`cordon`, `drain`, `uncordon`) |
| `GET` | `/api/k8s/contexts` | List kubeconfig contexts |
| `GET` | `/api/k8s/contexts/current` | Current kubeconfig context |
| `POST` | `/api/k8s/contexts/set` | Switch kubeconfig context |
| `GET` | `/api/k8s/crds` | List CustomResourceDefinitions |
| `GET` | `/api/k8s/crds/resources` | List custom resources for a CRD |
| `GET` | `/api/k8s/cluster-health` | Cluster health scan |
| `POST` | `/api/k8s/benchmark` | HTTP service benchmark (`p50`/`p95`/`p99`) |
| `POST` | `/api/k8s/port-forward/start` | Start `kubectl port-forward` |
| `POST` | `/api/k8s/port-forward/stop` | Stop active port forward |
| `GET` | `/api/k8s/port-forward/list` | List active port forwards |
| `GET` | `/api/kind` | List Kind clusters |
| `POST` | `/api/kind/create` | Create Kind cluster |
| `POST` | `/api/kind/delete` | Delete Kind cluster |

---

## Lima VMs & Ollama Models

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/api/lima` | List Lima VMs |
| `POST` | `/api/lima/start` | Start Lima VM (`{name}`) |
| `POST` | `/api/lima/stop` | Stop Lima VM (`{name}`) |
| `POST` | `/api/lima/delete` | Delete Lima VM (`{name, force}`) |
| `GET` | `/api/lima/info` | Lima system info |
| `POST` | `/api/lima/shell` | Run non-interactive command in VM (`{name, command}`) |
| `GET` | `/api/lima/templates` | List Lima VM templates |
| `POST` | `/api/lima/create` | Create Lima VM (`{name, template, cpus, memory, disk}`) |
| `GET` | `/api/models` | List Ollama models (`?profile=...`) |
| `POST` | `/api/models/pull` | Pull Ollama model (`?profile=...&modelName=...`) |
| `POST` | `/api/models/serve` | Serve Ollama model (`?profile=...&modelName=...&port=...`) |
| `POST` | `/api/models/delete` | Delete Ollama model (`?profile=...&modelName=...`) |

> **Interactive PTY Terminals:** HTTP `/api/terminal/*` routes were deliberately removed so no localhost HTTP endpoint exposes interactive shell execution. Real PTY sessions (`portable-pty`) run exclusively over Tauri IPC commands and events.

---

## AI Conversations, Knowledge Bank, Offline Help & Sandbox

### AI Chat, Multi-Conversation History & Headless CLI

| Method | Endpoint | Description |
|--------|----------|-------------|
| `POST` | `/api/ai/chat` | Non-streaming multi-provider AI completion (`{provider, model, api_key, messages, endpoint}`) |
| `POST` | `/api/ai/models` | List available models from an AI provider (`{provider, api_key, endpoint}`) |
| `POST` | `/api/ai/search` | SearXNG / DuckDuckGo web search (`{query, instances?, max_results?, timeout_secs?}`) |
| `POST` | `/api/ai/fetch-page` | Fetch web page and convert HTML to Markdown (`{url, max_length?, mode?}`) |
| `GET` | `/api/ai/context` | Deterministic 3-file core prompt + `~/.colima-ui/plugins.yaml` context |
| `GET` | `/api/ai/history` | Load messages for a thread (`?conversationId=default`, ordered by `created_at ASC, rowid ASC`) |
| `POST` | `/api/ai/history` | Upsert a chat message (`{message: {id, role, content}, conversationId?}`) |
| `POST` | `/api/ai/history/clear` | Clear messages in a thread (`{conversationId?}`) |
| `GET` | `/api/ai/conversations` | List all conversation threads (`id`, `title`, `updated_at`, `message_count`, `preview`) |
| `POST` | `/api/ai/conversations` | Create a conversation thread (`{id, title}`) |
| `POST` | `/api/ai/conversations/rename` | Rename a conversation thread (`{id, title}`) |
| `POST` | `/api/ai/conversations/delete` | Delete a conversation thread (`{id}`; empties `"default"` instead of removing it) |
| `POST` | `/api/cli/chat` | Headless CLI agent endpoint (`{prompt, provider?, model?}`) used by `cui` |

### Knowledge Bank, Offline Help Articles & Shell Sandbox

| Method | Endpoint | Description |
|--------|----------|-------------|
| `POST` | `/api/kb/query` | Match error text against `solutions` & `anti_patterns` (`{errorText}`) |
| `POST` | `/api/kb/feedback` | Upvote or downvote a solution (`{solutionId, isLike}`) |
| `POST` | `/api/kb/learn` | Persist a new error pattern → solution pair (`{errorPattern, solutionText}`) |
| `POST` | `/api/kb/search` | FTS5 search across `agent_memory` (`{query, limit?}`) |
| `GET` | `/api/kb/memories` | List all stored agent memories |
| `POST` | `/api/kb/memories/add` | Add memory entry (`{memoryType, content}` — `reasoning`, `preference`, `learned`) |
| `POST` | `/api/kb/memories/update` | Update memory content (`{id, content}`) |
| `POST` | `/api/kb/memories/delete` | Delete memory entry (`{id}`) |
| `GET` | `/api/kb/articles` | List compiled offline Help articles (`?locale=en\|ja\|vi\|zh`) |
| `GET` | `/api/kb/articles/get` | Fetch a single offline Help article by slug (`?slug=start-colima&locale=en`) |
| `GET` | `/api/kb/articles/search` | FTS5 search across offline Help articles (`?q=...&locale=en`) |
| `POST` | `/api/sandbox/execute` | Execute command only if `shell_sandbox` classifies it `Safe` (`{command}`) |
| `POST` | `/api/sandbox/execute-approved` | Execute user-approved `NeedsApproval` command (`{command}`; `Banned` commands still rejected) |

---

## Response Format

Every endpoint wraps its result in `ApiResponse<T>`:

```json
{
  "success": true,
  "data": "...",
  "error": null
}
```

On failure, `error` carries a structured, pre-redacted `ColimaError` object (identical to Tauri IPC errors):

```json
{
  "success": false,
  "data": null,
  "error": {
    "code": "not_running",
    "detail": "Cannot connect to the Docker daemon",
    "hint": "The Colima VM is not running. Start it from the Instances page.",
    "doc_id": "start-colima"
  }
}
```

- `code` (`ErrorCode`): `not_installed` | `not_running` | `permission_denied` | `timeout` | `command_failed` | `network` | `validation` | `not_found` | `internal` | `unknown`.
- Optional fields `command`, `exit_code`, `hint`, and `doc_id` are serialized only when present (`skip_serializing_if = "Option::is_none"`). When a known error has no remediation link or hint, those fields are omitted (or `null` at the client normalizer boundary).
