---
name: docker
description: Docker container and image management through App API. Introspection, lifecycle, and troubleshooting.
---

# Docker Management Skill

You manage Docker containers, images, and resources running inside the Colima VM.

## Core Rule: App-First

**ALWAYS use `[QUERY]` for read-only operations and `[EVENT_APPROVE]` for state changes.** Only use CLI via `cli-exec` for operations the App does not support (docker build, docker push, docker save/load).

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
- **Exec command**: `[EVENT_APPROVE: container-exec | {"id": "<container>", "command": "ls -la /app"}]`
- **Pull image**: `[EVENT_APPROVE: pull-image | {"image": "nginx:latest"}]`
- **Tag image**: `[EVENT_APPROVE: tag-image | {"source": "myapp:latest", "target": "registry.io/myapp:v1"}]`
- **Remove image**: `[EVENT_APPROVE: remove-image | {"id": "<image>"}]` ⚠️ DANGEROUS
- **Prune images**: `[EVENT_APPROVE: prune-images]` ⚠️ DANGEROUS

## 3. CLI Escape Hatch (Only for unsupported operations)

These operations are NOT available via the App API. Use `cli-exec`:
- **Build**: `[EVENT_APPROVE: cli-exec | {"command": "docker", "args": ["build", "-t", "myapp:latest", "."]}]`
- **Push**: `[EVENT_APPROVE: cli-exec | {"command": "docker", "args": ["push", "registry.io/myapp:v1"]}]`
- **Save/Load**: `[EVENT_APPROVE: cli-exec | {"command": "docker", "args": ["save", "-o", "backup.tar", "myapp:latest"]}]`

## 4. Common Troubleshooting

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
