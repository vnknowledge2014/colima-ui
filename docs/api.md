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

`/api/health` is the only one. There is no endpoint that hands out the token:
the desktop webview gets it over IPC (`auth::api_token`), and a browser gets it
from the app in a URL fragment. See [Architecture → HTTP Authentication](architecture.md#http-authentication).

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

## Diagnostics

| Method | Endpoint | Description |
|--------|----------|-------------|
| `POST` | `/api/diagnostics/bundle` | Build a redacted bundle (`{error?, containerId?, logLines?}`) |
| `POST` | `/api/diagnostics/save` | Write selected sections as Markdown (`{bundle, include[], destDir, fileName, overwrite?}`) |

Returns `{sections[], signature, appVersion, truncatedBytes}`. **Nothing is
transmitted** — there is no send path in the backend; the response goes to the
caller and sharing is the user's action.

Every section is redacted at construction, so a section that skipped redaction
cannot exist. `save` redacts the rendered Markdown again, because the bundle
round-trips through the client before being written. Container environment
variables are never collected; container logs are, opt-in and off by default.

`signature` uses the same algorithm as the Knowledge Bank's error matching, so the
same failure on two machines produces the same string and duplicate reports can be
grouped.

Logs are capped at 5000 lines and 2 MB *before* redaction — `redact` costs about
0.8 s/MB, so bounding the input is a latency budget as much as a byte budget.

---

## Live Metrics

| Method | Endpoint | Description |
|--------|----------|-------------|
| `POST` | `/api/metrics/interval` | Sampling period in ms (`{ms}`); clamped to 1000–60000, returns what was applied |

Samples are not fetched — they arrive on the SSE stream. Subscribe with
`GET /api/events?topics=metrics.sample`; each event carries
`{samples[], engine, intervalMs}`.

**Naming the topic is what starts sampling.** The collector runs only while
`subscriber_count("metrics.sample")` is above zero, and that count is tied to the
lifetime of the HTTP stream — a closed tab, a reload or a crashed client all stop
it with no cooperation. A client that subscribes without naming the topic still
receives every event but is counted as watching nothing, so it will not keep the
sampler alive on its own.

A `stream-lagged` event means samples were dropped for that client. Render a gap;
interpolating across it asserts data nobody observed.

---

## Background Transfers

| Method | Endpoint | Description |
|--------|----------|-------------|
| `POST` | `/api/images/save` | Export images to a TAR (`{images[], destDir, fileName, overwrite?}`) |
| `POST` | `/api/images/load` | Import a TAR (`{tarPath}`) |
| `POST` | `/api/containers/cp/to` | Copy a host file into a container (`{containerId, hostPath, containerPath}`) |
| `POST` | `/api/containers/cp/from` | Copy a container path out **as a TAR archive** (`{containerId, containerPath, destDir, fileName, overwrite?}`) |
| `POST` | `/api/transfers/cancel` | Cancel a running transfer (`{jobId}`) |
| `GET` | `/api/transfers` | List transfers this process knows about |

These return `{jobId, totalEstimate}` immediately and run in the background.
Progress arrives as `transfer.progress` events, ending in `transfer.done`
(which carries `cancelled`) or `transfer.failed`. `totalEstimate` comes from image
metadata and is an estimate, not a total — Docker reports none for these
operations.

## Security

| Method | Endpoint | Description |
|--------|----------|-------------|
| `POST` | `/api/security/scan` | Scan one local image for known vulnerabilities |
| `POST` | `/api/security/scan/cancel` | Stop a running scan by `scanId` |
| `POST` | `/api/security/sbom` | Write an SBOM (CycloneDX or SPDX-JSON) to a file |
| `POST` | `/api/security/audit` | Scan + configuration rules + score, in one call |
| `GET` | `/api/security/rules` | The rule pack this build carries |
| `GET` | `/api/security/alternatives?image=…` | Base images worth considering instead |

`/api/security/alternatives` answers from a table compiled into the app. It is a
lookup, not a query to a service: asking a server "what should I use instead of
X?" would send the user's image list to us, which says what they run and
sometimes what they are building before it is public.

`/api/security/audit` is one endpoint rather than three because a score is only
meaningful beside the findings and rule results it came from; assembling it
client-side is how a score ends up displayed next to a different image's scan.

The score is four numbers plus a total: **vulnerabilities /40** (from the scan),
**hardening /25**, **provenance /20**, **freshness /15** (from the rules). The
total exists to sort a list — the breakdown is the answer, because "42/100" says
nothing about whether the problem is CVEs or a root user on a moving tag.

Every score carries `inputs` — rule pack version, scanner, scanner version,
database date, strictness level. Show them next to the number: the same image
scores differently against a newer database, and two scanners disagree by more
than 10× on identical input. `level` is `l1` (default), `l2` or `l3`; higher
levels enable more rules and can therefore only lower a score.

The scanner is **Trivy on the host**, not bundled — its vulnerability database
alone is ~1.2 GB and changes daily. Whether it is installed comes from
`/api/capabilities` under the id `trivy`; a missing scanner is a capability
state with an install hint, not an API error.

`scanId` is chosen by the caller so a scan can be cancelled before its process
exists. Progress arrives on the SSE stream as `security-scan-progress` with a
`stage` of `database` or `scan` — the first scan in a while spends most of its
time on the database, and reporting that as scanning would look like a hang.

Results are cached by **image digest**, never by tag: rebuilding `app:latest`
and scanning again must not return the previous image's findings. Pass
`refresh: true` to bypass it.

SBOM export writes through a scratch file and renames on success, and refuses an
existing destination unless `overwrite` is set — a half-written SBOM that looks
complete is worse than none.

Nothing is pulled. An image that is not present locally is an error saying so,
because pulling spends bandwidth the user did not agree to spend. Scanning reads
the image through the local runtime; image names and results stay on the machine.

## Announcements

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/api/announcements` | Vendor announcements: releases, advisories, maintenance |

A one-way, read-only channel. The backend fetches one hard-coded static JSON file
and returns it verbatim; **nothing about the user is sent** — no id, no cookie, no
custom header, and the request takes no parameters.

Fetching happens in the backend rather than the webview so the app's CSP does not
have to list an external host, and so exactly one URL is reachable instead of the
`https://*` the HTTP plugin's capability would allow.

A feed that cannot be read is an **error**, never an empty feed: the client keeps
showing what it already had rather than clearing the list on a flaky network. A feed
declaring a `version` newer than the build understands is refused rather than
guessed at; unknown *fields* are ignored, so a feed written for a later build does
not break an earlier one.

Note that fetching at all reveals an IP, a timestamp and the app version to the
host. See `docs/telemetry.md`; the channel can be switched off, and switching it off
stops the request rather than filtering its result.

### Reconciling, not just listening

Events are the fast path and are **lossy**: the SSE channel drops frames under lag
by design, and a job can reach a terminal state before the caller even holds its id
(the command is spawned before `start` returns). `GET /api/transfers` is the source
of truth — call it on connect and after every reconnect.

Each entry is
`{jobId, kind, status, bytes, totalEstimate, startedAt, finishedAt, targetLabel, error}`,
where `status` is `starting | running | success | failed | cancelled` and `kind` is
`save | load | cp-in | cp-out`. Finished transfers stay listed for about 60 seconds,
which is what lets a client that missed the terminal event still learn the outcome;
after that, absence means "finished and already reported". `status: "success"` is
recorded only after the destination has been published, so it implies the file is
there. `targetLabel` is an image reference or a path inside a container — never a
host path, so this payload needs no redaction. `error` is redacted at the source.

`POST /api/transfers/cancel` returns `cancelled | alreadyFinished | unknownJob`
rather than a boolean: only `cancelled` is followed by a terminal event, so the
other two must be settled by the caller. Nothing is persisted — every transfer is
killed when the app exits.

A transfer is refused at start if another unfinished transfer is already writing to
the same destination. The existence check alone cannot see this, because the first
job's output is not at that path until it completes.

### Archive contract

`fileName` must end in `.tar` for `/api/images/save` and `/api/containers/cp/from`.
Both write an **uncompressed** archive, so `.tar.gz`, `.tgz` and `.zip` are refused
rather than honoured under a name that would misdescribe the contents.
`/api/images/load` rejects a file that is not a TAR before the runtime is started.

`/api/containers/cp/from` returns an archive **whether the container path is a file
or a directory**. It previously wrote the path directly, which produced a directory
for a directory source — progress then measured a directory entry rather than data,
and cancelling could not remove it. Callers that expected a bare file must extract:
`tar -xf <file>`.

Output is written to a `<fileName>.part` sibling and renamed onto `fileName` only
once the command succeeds, so a cancelled or failed transfer leaves any file already
at that path untouched.

`destDir` and `fileName` are separate fields because the write is confined to the
folder the caller named: checking a joined path against its own parent would pass
by construction. A `fileName` containing `../` is refused before any command runs.

A file picker in the desktop UI records the user's intent; it is not an
authorization mechanism, and these routes are reachable with a valid token
regardless. Confinement is enforced in `commands::file_transfer` on every path.

---

## Docker Topology

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/api/topology` | Docker graph for the current engine |

Returns `{nodes, edges, warnings}`. Nodes are `container`, `network`, `volume`,
`project` and `image`, with ids namespaced by kind (`container:abc`). Edges always
start at a container. `warnings` is non-empty when a subsystem could not be
listed — the graph is then incomplete rather than empty, and the caller should say
so instead of implying those attachments do not exist.

Takes no `instance` parameter: nothing in the app targets a second engine.

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
