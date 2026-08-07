# HTTP API Reference

ColimaUI exposes a REST API on `http://127.0.0.1:11420` for browser-mode access and external integrations.

## Authentication

All endpoints (except public ones) require Bearer token authentication:

```
Authorization: Bearer <token>
```

### Public Endpoints (no auth required)

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/api/health` | Health check (returns `"ok"`) |
| `GET` | `/api/auth/token` | Get API token (CORS-protected to localhost) |

## SSE Events

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/api/events` | Server-Sent Events stream (docker state, instances) |

Event types: `docker-state`, `instances`, `docker-event`

---

## System

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/api/system/check` | System info (colima, docker, lima versions) |
| `GET` | `/api/system/version` | Colima version string |
| `GET` | `/api/system/homebrew` | Homebrew status |
| `GET` | `/api/system/check-tool` | Check tool installation (`?name=kubectl`) |
| `GET` | `/api/system/platform` | OS/arch/package manager info |
| `GET` | `/api/system/host-specs` | Host CPU/memory specs |
| `POST` | `/api/system/install` | Install dependency (`{name, method}`) |
| `POST` | `/api/system/prune` | Docker system prune (`?all=true`) |
| `GET` | `/api/system/df` | Docker disk usage |
| `GET` | `/api/settings` | Get all settings |
| `POST` | `/api/settings` | Set a setting (`{key, value}`) |

---

## Colima Instances

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/api/instances` | List all Colima instances |
| `POST` | `/api/instances/start` | Start instance (JSON: `StartConfig`) |
| `POST` | `/api/instances/stop` | Stop instance (`?profile=default&force=false`) |
| `POST` | `/api/instances/delete` | Delete instance (`?profile=default&force=false`) |
| `GET` | `/api/instances/status` | Instance status (`?profile=default`) |
| `GET` | `/api/instances/ssh` | Get SSH command (`?profile=default`) |
| `POST` | `/api/instances/k8s` | Kubernetes action (`?profile=default&action=start`) |

---

## Docker Containers

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/api/containers` | List containers (`?all=false`) |
| `POST` | `/api/containers/start` | Start container (`?containerId=...`) |
| `POST` | `/api/containers/stop` | Stop container (`?containerId=...`) |
| `POST` | `/api/containers/restart` | Restart container (`?containerId=...`) |
| `POST` | `/api/containers/remove` | Remove container (`?containerId=...&force=false`) |
| `GET` | `/api/containers/logs` | Container logs (`?containerId=...&lines=100`) |
| `GET` | `/api/containers/inspect` | Inspect container (`?containerId=...`) |
| `GET` | `/api/containers/stats` | Container stats (`?containerId=...`) |
| `GET` | `/api/containers/stats/all` | All container stats |
| `GET` | `/api/containers/top` | Container processes (`?containerId=...`) |
| `POST` | `/api/containers/exec` | Exec in container (`{containerId, command}`) |
| `POST` | `/api/containers/run` | Run new container (`{image, name, ports, env, ...}`) |
| `POST` | `/api/containers/rename` | Rename container (`{containerId, newName}`) |
| `POST` | `/api/containers/pause` | Pause container (`?containerId=...`) |
| `POST` | `/api/containers/unpause` | Unpause container (`?containerId=...`) |

---

## Docker Images

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/api/images` | List images |
| `POST` | `/api/images/remove` | Remove image (`?imageId=...&force=false`) |
| `POST` | `/api/images/pull` | Pull image (`?imageName=...`) |
| `POST` | `/api/images/prune` | Prune unused images |
| `GET` | `/api/images/inspect` | Inspect image (`?imageId=...`) |
| `POST` | `/api/images/tag` | Tag image (`{imageId, newTag}`) |

---

## Docker Volumes

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/api/volumes` | List volumes |
| `POST` | `/api/volumes/create` | Create volume (`{name, driver}`) |
| `POST` | `/api/volumes/remove` | Remove volume (`?name=...&force=false`) |
| `POST` | `/api/volumes/prune` | Prune unused volumes |
| `GET` | `/api/volumes/inspect` | Inspect volume (`?name=...`) |

---

## Docker Networks

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/api/networks` | List networks |
| `POST` | `/api/networks/create` | Create network (`{name, driver, subnet}`) |
| `POST` | `/api/networks/remove` | Remove network (`?name=...`) |
| `GET` | `/api/networks/inspect` | Inspect network (`?name=...`) |
| `POST` | `/api/networks/prune` | Prune unused networks |

---

## Docker Compose

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/api/compose` | List compose projects |
| `POST` | `/api/compose/up` | Compose up (`{projectDir, detach}`) |
| `POST` | `/api/compose/down` | Compose down (`{projectName}`) |
| `POST` | `/api/compose/restart` | Compose restart (`{projectName}`) |
| `GET` | `/api/compose/logs` | Compose logs (`?projectName=...&lines=100`) |
| `GET` | `/api/compose/ps` | Compose ps (`?projectName=...`) |

---

## Docker System

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/api/docker/df` | Docker disk usage |
| `POST` | `/api/docker/prune` | Full system prune (`?confirm=true` required) |

---

## Kubernetes

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/api/k8s/check` | Check kubectl connectivity |
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
| `GET` | `/api/k8s/pods/logs/stream` | Stream pod logs (SSE) |
| `GET` | `/api/k8s/pods/containers` | List pod containers |
| `GET` | `/api/k8s/pods/container-logs` | Container-specific logs |
| `POST` | `/api/k8s/pods/delete` | Delete pod |
| `GET` | `/api/k8s/describe` | Describe resource |
| `GET` | `/api/k8s/resources/yaml` | Get resource YAML |
| `POST` | `/api/k8s/scale` | Scale deployment |
| `POST` | `/api/k8s/scale-generic` | Scale any scalable resource |
| `POST` | `/api/k8s/resources/delete` | Delete resource |
| `POST` | `/api/k8s/resources/restart` | Restart resource |
| `POST` | `/api/k8s/apply` | Apply YAML |
| `POST` | `/api/k8s/exec` | Exec into pod |
| `POST` | `/api/k8s/nodes/action` | Node action (cordon/drain/uncordon) |
| `GET` | `/api/k8s/contexts` | List kubeconfig contexts |
| `GET` | `/api/k8s/contexts/current` | Current context |
| `POST` | `/api/k8s/contexts/set` | Switch context |
| `GET` | `/api/k8s/crds` | List CRDs |
| `GET` | `/api/k8s/crds/resources` | List CRD resources |
| `GET` | `/api/k8s/cluster-health` | Cluster health scan |
| `POST` | `/api/k8s/benchmark` | HTTP benchmark |

### Port Forwarding

| Method | Endpoint | Description |
|--------|----------|-------------|
| `POST` | `/api/k8s/port-forward/start` | Start port forward |
| `POST` | `/api/k8s/port-forward/stop` | Stop port forward |
| `GET` | `/api/k8s/port-forward/list` | List active forwards |

### Kind Clusters

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/api/kind` | List Kind clusters |
| `POST` | `/api/kind/create` | Create Kind cluster |
| `POST` | `/api/kind/delete` | Delete Kind cluster |

---

## Lima VMs

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/api/lima` | List Lima VMs |
| `POST` | `/api/lima/start` | Start VM (`{name}`) |
| `POST` | `/api/lima/stop` | Stop VM (`{name}`) |
| `POST` | `/api/lima/delete` | Delete VM (`{name, force}`) |
| `GET` | `/api/lima/info` | Lima system info |
| `POST` | `/api/lima/shell` | Execute command in VM (`{name, command}`) |
| `GET` | `/api/lima/templates` | List available templates |
| `POST` | `/api/lima/create` | Create VM (`{name, template, cpus, memory, disk}`) |

---

## AI Models (Ollama)

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/api/models` | List models (`?profile=...`) |
| `POST` | `/api/models/pull` | Pull model (`?profile=...&modelName=...`) |
| `POST` | `/api/models/serve` | Serve model (`?profile=...&modelName=...&port=...`) |
| `POST` | `/api/models/delete` | Delete model (`?profile=...&modelName=...`) |

---

## AI Chat & Diagnostics

| Method | Endpoint | Description |
|--------|----------|-------------|
| `POST` | `/api/ai/chat` | AI chat message |
| `POST` | `/api/ai/models` | List AI provider models |
| `POST` | `/api/ai/search` | SearXNG web search |
| `POST` | `/api/ai/fetch-page` | Fetch page as markdown |
| `GET` | `/api/ai/context` | Get app context for AI |
| `POST` | `/api/cli/chat` | Headless CLI chat (for external agents) |

---

## Knowledge Bank

| Method | Endpoint | Description |
|--------|----------|-------------|
| `POST` | `/api/kb/query` | Query knowledge bank |
| `POST` | `/api/kb/search` | Search memories |
| `GET` | `/api/kb/memories` | Get all memories |
| `POST` | `/api/kb/memories/update` | Update a memory |
| `POST` | `/api/kb/memories/delete` | Delete a memory |

---

## Shell Sandbox

| Method | Endpoint | Description |
|--------|----------|-------------|
| `POST` | `/api/sandbox/execute` | Execute safe command |
| `POST` | `/api/sandbox/execute-approved` | Execute user-approved command |

---

## Diagnostics

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/api/diagnostics/logs` | Collect diagnostic logs |

---

## Terminal Sessions

| Method | Endpoint | Description |
|--------|----------|-------------|
| `POST` | `/api/terminal/create` | Create PTY terminal session |
| `POST` | `/api/terminal/write` | Write to terminal |
| `GET` | `/api/terminal/read` | Read from terminal |
| `POST` | `/api/terminal/close` | Close terminal session |
| `POST` | `/api/terminal/resize` | Resize terminal |

---

## Response Format

All endpoints return a consistent JSON response:

```json
{
  "success": true,
  "data": "...",
  "error": null
}
```

On error:

```json
{
  "success": false,
  "data": null,
  "error": "Error message"
}
```
