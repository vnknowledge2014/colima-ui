---
name: docker
description: Docker container and image management through App API. Introspection, lifecycle, and troubleshooting.
---

# Docker Management Skill

You manage Docker containers, images, and resources running inside the Colima VM.

## Core Rule: App-First

**ALWAYS use `[QUERY]` for read-only operations, `[EVENT_APPROVE]` for state changes, and `[NAVIGATE]` for rich built-in UI workflows.** Only use CLI via `cli-exec` for operations the App does not support (`docker push`, multi-arch `docker buildx`).

## 1. Introspection (Read-Only — use [QUERY])

- **List containers**: `[QUERY: list-containers]`
- **Container logs**: `[QUERY: container-logs | {"id": "<container>", "lines": 100}]`
- **Container stats**: `[QUERY: container-stats | {"id": "<container>"}]`
- **All container stats**: `[QUERY: all-container-stats]`
- **Container processes**: `[QUERY: container-top | {"id": "<container>"}]`
- **Inspect container**: `[QUERY: inspect-container | {"id": "<container>"}]` (Check "Mounts" for volumes, "NetworkSettings" for IPs)
- **List images**: `[QUERY: list-images]`
- **Inspect image**: `[QUERY: inspect-image | {"id": "<image>"}]`
- **Disk usage**: `[QUERY: system-df]`

## 2. Lifecycle (State Changes — use [EVENT_APPROVE])

- **Start**: `[EVENT_APPROVE: start-container | {"id": "<container>"}]`
- **Stop**: `[EVENT_APPROVE: stop-container | {"id": "<container>"}]`
- **Restart**: `[EVENT_APPROVE: restart-container | {"id": "<container>"}]`
- **Pause**: `[EVENT_APPROVE: pause-container | {"id": "<container>"}]`
- **Unpause**: `[EVENT_APPROVE: unpause-container | {"id": "<container>"}]`
- **Remove**: `[EVENT_APPROVE: remove-container | {"id": "<container>"}]` ⚠️ DANGEROUS
- **Run new**: `[EVENT_APPROVE: run-container | {"image": "nginx:latest", "name": "web", "ports": ["8080:80"], "detach": true}]`
- **Rename**: `[EVENT_APPROVE: rename-container | {"id": "<container>", "newName": "new-name"}]`
- **Exec command** (one-shot, non-interactive): `[EVENT_APPROVE: container-exec | {"id": "<container>", "command": "ls -la /app"}]`
- **Build image**: `[EVENT_APPROVE: build-image | {"contextPath": ".", "dockerfile": "Dockerfile", "tag": "my-app:latest"}]`
- **Pull image**: `[EVENT_APPROVE: pull-image | {"image": "nginx:latest"}]`
- **Tag image**: `[EVENT_APPROVE: tag-image | {"source": "myapp:latest", "target": "registry.io/myapp:v1"}]`
- **Remove image**: `[EVENT_APPROVE: remove-image | {"id": "<image>"}]` ⚠️ DANGEROUS
- **Prune images**: `[EVENT_APPROVE: prune-images]` ⚠️ DANGEROUS

## 3. Native UI Workflows (Interactive PTY, Transfers, Topology & Live Metrics)

Do NOT fall back to `cli-exec` for interactive shells, archive transfers, or visual inspection — guide the user with `[NAVIGATE]`:
- **Interactive Container PTY (`dockerExec`)**: Open a live interactive shell inside any running container via `[NAVIGATE: containers]` or `[NAVIGATE: terminal]`.
- **Image TAR Save / Load**: Supported natively in `[NAVIGATE: images]` with non-blocking background transfer progress and cancellation.
- **Container File Copy (`cp` to/from host)**: Supported natively in `[NAVIGATE: containers]` with background transfer progress and safe destination path confinement.
- **Network & Volume Relationships**: Inspect container-network-volume connections visually via `[NAVIGATE: topology]`.
- **Live Metrics, Activity & Alerts**: Monitor real-time CPU/memory/network charts, alert rules, and self-heal history via `[NAVIGATE: activity]`.

## 4. CLI Escape Hatch (Only for unsupported operations)

Only `docker push` (and advanced multi-arch `docker buildx`) require `cli-exec`. Pass concrete relative or absolute paths (never use `~` or unsplit whitespace in `args`):
- **Push**: `[EVENT_APPROVE: cli-exec | {"command": "docker", "args": ["push", "registry.io/myapp:v1"]}]`
- **Multi-arch Buildx**: `[EVENT_APPROVE: cli-exec | {"command": "docker", "args": ["buildx", "build", "--platform", "linux/amd64,linux/arm64", "-t", "registry.io/myapp:v1", "/Users/alice/projects/myapp"]}]`

## 5. Common Troubleshooting

### Container Exits Immediately (Crash Loop)
1. `[QUERY: container-logs | {"id": "<container>", "lines": 50}]`
2. `[QUERY: inspect-container | {"id": "<container>"}]` — check ExitCode:
   - *137*: OOM Killer → increase Colima RAM Preset or set memory limits.
   - *255*: Entrypoint failed → check CMD/ENTRYPOINT in Dockerfile.

### Disk Space Full
1. `[QUERY: system-df]` — verify usage.
2. `[EVENT_APPROVE: system-prune]` ⚠️ Warn: deletes ALL unused resources.

## Advanced References
- `[READ_REFERENCE: skills/docker/references/rootless_security.md]`
- `[READ_REFERENCE: skills/docker/references/network_isolation.md]`

## Anti-Patterns

- **Never invoke raw `docker ps`, `docker logs`, or `docker inspect` via `cli-exec`** — always use `[QUERY: list-containers]`, `[QUERY: container-logs | ...]`, and `[QUERY: inspect-container | ...]`.
- **Never run `docker save`, `docker load`, or `docker cp` via `cli-exec`** — guide the user to `[NAVIGATE: images]` or `[NAVIGATE: containers]` for non-blocking background transfers.
- **Never emit `system-prune` or `remove-container` without warning about data loss first**.
