---
name: docker_compose
description: Docker Compose project management through App API. List, start, stop, and debug multi-container projects.
---

# Docker Compose Management Skill

Manage Docker Compose multi-container projects through the App Event Bus.

## Core Rule: App-First

**ALWAYS use `[QUERY]` for introspection and `[EVENT_APPROVE]` for lifecycle.** Only use CLI for `docker compose up --build` and `docker compose config`.

## 1. Introspection (Read-Only)

- **List projects**: `[QUERY: compose-list]`
- **Project logs**: `[QUERY: compose-logs | {"projectName": "myapp", "lines": 100}]`

## 2. Lifecycle (State Changes)

- **Start project**: `[EVENT_APPROVE: compose-up | {"dir": "/path/to/project"}]`
- **Stop project**: `[EVENT_APPROVE: compose-down | {"projectName": "myapp"}]`
- **Restart project**: `[EVENT_APPROVE: compose-restart | {"projectName": "myapp"}]`

## 3. CLI Escape Hatch

- **Build + start**: `[EVENT_APPROVE: cli-exec | {"command": "docker", "args": ["compose", "-f", "/path/to/docker-compose.yml", "up", "-d", "--build"]}]`
- **Validate config**: `[EVENT_APPROVE: cli-exec | {"command": "docker", "args": ["compose", "-f", "/path/to/docker-compose.yml", "config"]}]`
- **View service status**: `[EVENT_APPROVE: cli-exec | {"command": "docker", "args": ["compose", "-p", "myapp", "ps"]}]`

## 4. Troubleshooting

### Service won't start
1. `[QUERY: compose-logs | {"projectName": "myapp", "lines": 50}]`
2. Check for port conflicts, missing env vars, or volume mount errors.
3. If build-related, use CLI escape: `docker compose up --build`.
