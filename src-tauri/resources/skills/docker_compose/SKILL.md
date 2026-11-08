---
name: docker_compose
description: Docker Compose project management through App API. List, start, stop, diagnose, autofix, and debug multi-container projects.
---

# Docker Compose Management Skill

Manage Docker Compose multi-container projects through the App Event Bus and built-in Compose intelligence tools.

## Core Rule: App-First

**ALWAYS use `[QUERY]` for introspection and Compose Diagnose, `[EVENT_APPROVE]` for lifecycle and AI Autofix, and `[NAVIGATE: compose]` for visual diff review.** Only use CLI via `cli-exec` for `docker compose config`.

## 1. Introspection (Read-Only)

- **List projects**: `[QUERY: compose-list]`
- **Project service status (`ps`)**: `[QUERY: compose-ps | {"projectName": "myapp"}]`
- **Project logs**: `[QUERY: compose-logs | {"projectName": "myapp", "lines": 100}]`
- **Diagnose Compose YAML**: `[QUERY: compose-diagnose | {"path": "/path/to/docker-compose.yml"}]`
- **Autofix history**: `[QUERY: compose-autofix-history]`

## 2. Lifecycle (State Changes)

- **Create & start project (`up`)**: `[EVENT_APPROVE: compose-up | {"dir": "/path/to/project"}]`
- **Stop & remove project (`down`)**: `[EVENT_APPROVE: compose-down | {"projectName": "myapp"}]`
- **Start existing stopped services**: `[EVENT_APPROVE: compose-start | {"projectName": "myapp"}]`
- **Stop running services (without removing)**: `[EVENT_APPROVE: compose-stop | {"projectName": "myapp"}]`
- **Restart project**: `[EVENT_APPROVE: compose-restart | {"projectName": "myapp"}]`
- **Build images** (`service` optional): `[EVENT_APPROVE: compose-build | {"projectDir": "/path/to/project", "service": "api"}]`
- **Pull images** (`service` optional): `[EVENT_APPROVE: compose-pull | {"projectDir": "/path/to/project", "service": "api"}]`

## 3. Built-in Compose Diagnose & AI Autofix

Run Compose diagnostics and repair directly through the Event Bus or visually in `[NAVIGATE: compose]`:
- **Compose Diagnose** (`[QUERY: compose-diagnose | {"path": "/path/to/docker-compose.yml"}]`): Validates `docker-compose.yml` / `compose.yaml` schema, port collisions, missing environment variables, volume mount issues, and matches errors against the Knowledge Bank.
- **Propose AI Autofix** (`[EVENT_APPROVE: compose-autofix-propose | {"path": "/path/to/docker-compose.yml"}]`): Generates safe candidate YAML fixes with an explanation and diff preview.
- **Apply AI Autofix** (`[EVENT_APPROVE: compose-autofix-apply | {"path": "/path/to/docker-compose.yml", "patchedContent": "...", "expectedHash": "..."}]`): Applies the reviewed YAML patch safely (protecting against concurrent modifications and undeclared key removals).
- **Undo & History**: Revert any applied fix with `[EVENT_APPROVE: compose-autofix-undo | {"id": "<fix-id>"}]` and inspect records via `[QUERY: compose-autofix-history]`.

## 4. CLI Escape Hatch

- **Validate rendered config**: `[EVENT_APPROVE: cli-exec | {"command": "docker", "args": ["compose", "-f", "/path/to/docker-compose.yml", "config"]}]`

## 5. Troubleshooting

### Service won't start
1. `[QUERY: compose-ps | {"projectName": "myapp"}]` and `[QUERY: compose-logs | {"projectName": "myapp", "lines": 50}]`
2. Run `[QUERY: compose-diagnose | {"path": "/path/to/docker-compose.yml"}]` (or open `[NAVIGATE: compose]`) for port conflicts, missing env vars, or volume mount errors, then propose a fix via `[EVENT_APPROVE: compose-autofix-propose | {"path": "/path/to/docker-compose.yml"}]`.
3. If an image rebuild is needed, run `[EVENT_APPROVE: compose-build | {"projectDir": "/path/to/project"}]` followed by `[EVENT_APPROVE: compose-up | {"dir": "/path/to/project"}]`.

## Advanced References
- `[READ_REFERENCE: skills/docker_compose/references/secrets_management.md]`

## Anti-Patterns

- **Never use `cli-exec` for `compose ls`, `ps`, `up`, `down`, `start`, `stop`, `restart`, `build`, `pull`, or `logs`** — always use `compose-list`, `compose-ps`, `compose-up`, `compose-down`, `compose-start`, `compose-stop`, `compose-restart`, `compose-build`, `compose-pull`, and `compose-logs`.
- **Never manually rewrite broken `compose.yaml` files via shell commands** — use `compose-diagnose` and `compose-autofix-propose` / `compose-autofix-apply` / `compose-autofix-undo` (or `[NAVIGATE: compose]`) for safe, undoable repairs.
