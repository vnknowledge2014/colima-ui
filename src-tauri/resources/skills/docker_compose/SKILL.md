---
name: docker_compose
description: Docker Compose project management through App API. List, start, stop, diagnose, autofix, and debug multi-container projects.
---

# Docker Compose Management Skill

Manage Docker Compose multi-container projects through the App Event Bus and built-in Compose intelligence tools.

## Core Rule: App-First

**ALWAYS use `[QUERY]` for introspection, `[EVENT_APPROVE]` for lifecycle, and `[NAVIGATE: compose]` for Compose Diagnose & AI Autofix.** Only use CLI for `docker compose up --build` and `docker compose config`.

## 1. Introspection (Read-Only)

- **List projects**: `[QUERY: compose-list]`
- **Project logs**: `[QUERY: compose-logs | {"projectName": "myapp", "lines": 100}]`

## 2. Lifecycle (State Changes)

- **Start project**: `[EVENT_APPROVE: compose-up | {"dir": "/path/to/project"}]`
- **Stop project**: `[EVENT_APPROVE: compose-down | {"projectName": "myapp"}]`
- **Restart project**: `[EVENT_APPROVE: compose-restart | {"projectName": "myapp"}]`

## 3. Built-in Compose Diagnose & AI Autofix

Navigate to `[NAVIGATE: compose]` to use ColimaUI's native Compose diagnostics and repair workflow:
- **Compose Diagnose**: Validates `docker-compose.yml` / `compose.yaml` schema, port collisions, missing environment variables, volume mount issues, and matches errors against the Knowledge Bank.
- **AI Autofix**: Generates safe candidate YAML fixes with an interactive **diff preview**, guarded **apply** (protecting against undeclared key removals), **undo** rollback, and full fix history.

## 4. CLI Escape Hatch

- **Build + start**: `[EVENT_APPROVE: cli-exec | {"command": "docker", "args": ["compose", "-f", "/path/to/docker-compose.yml", "up", "-d", "--build"]}]`
- **Validate config**: `[EVENT_APPROVE: cli-exec | {"command": "docker", "args": ["compose", "-f", "/path/to/docker-compose.yml", "config"]}]`
- **View service status**: `[EVENT_APPROVE: cli-exec | {"command": "docker", "args": ["compose", "-p", "myapp", "ps"]}]`

## 5. Troubleshooting

### Service won't start
1. `[QUERY: compose-logs | {"projectName": "myapp", "lines": 50}]`
2. Open `[NAVIGATE: compose]` to run **Compose Diagnose** for port conflicts, missing env vars, or volume mount errors, and preview an **AI Autofix** patch.
3. If image rebuild is needed, use CLI escape: `docker compose -f /path/to/docker-compose.yml up -d --build`.

## Advanced References
- `[READ_REFERENCE: skills/docker_compose/references/secrets_management.md]`

## Anti-Patterns

- **Never use `cli-exec` for basic `compose ls`, `up`, `down`, `restart`, or `logs`** — always use `compose-list`, `compose-up`, `compose-down`, `compose-restart`, and `compose-logs`.
- **Never manually rewrite broken `compose.yaml` files via shell commands** — guide the user to `[NAVIGATE: compose]` to run **Compose Diagnose** and preview **AI Autofix** diffs with undo safety.
