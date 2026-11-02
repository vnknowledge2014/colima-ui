---
name: limavm
description: Lima VM management through App API. Create, start, stop, and shell into lightweight Linux VMs.
---

# Lima VM Management Skill

Manage Lima virtual machines (without Docker/K8s) through the App Event Bus.

## Core Rule: App-First

**ALWAYS use `[QUERY]` for introspection and `[EVENT_APPROVE]` for lifecycle and one-shot shell commands.** Only use `cli-exec` for non-interactive `limactl` inspection or template validation.

## 1. Introspection (Read-Only)

- **List VMs**: `[QUERY: lima-list]`
- **System info**: `[QUERY: lima-info]`
- **Available templates**: `[QUERY: lima-templates]`

## 2. Lifecycle (State Changes)

- **Create VM**: `[EVENT_APPROVE: lima-create | {"name": "ubuntu", "templateUrl": "https://..."}]`
- **Start VM**: `[EVENT_APPROVE: lima-start | {"name": "ubuntu"}]`
- **Stop VM**: `[EVENT_APPROVE: lima-stop | {"name": "ubuntu"}]`
- **Delete VM**: `[EVENT_APPROVE: lima-delete | {"name": "ubuntu"}]` ⚠️ DANGEROUS

## 3. Shell Access

- **One-shot command in VM**: `[EVENT_APPROVE: lima-shell | {"name": "ubuntu", "command": "uname -a"}]`
- **Interactive shell session**: Navigate to `[NAVIGATE: terminal]` to open a persistent PTY shell inside the Lima VM.

## 4. CLI Escape Hatch

> **Sandbox Warning**: NEVER run interactive shell or `$EDITOR`-based `limactl` subcommands via `cli-exec` — they require a TTY and will hang! Use `[EVENT_APPROVE: lima-shell | {"name": "ubuntu", "command": "uname -a"}]` for one-shot commands and `[NAVIGATE: terminal]` for interactive shell sessions.

- **Detailed JSON VM listing**: `[EVENT_APPROVE: cli-exec | {"command": "limactl", "args": ["list", "--json"]}]`
- **Validate a Lima YAML template**: `[EVENT_APPROVE: cli-exec | {"command": "limactl", "args": ["validate", "/absolute/path/to/template.yaml"]}]`

## Advanced References
- `[READ_REFERENCE: skills/limavm/references/vm_security.md]`

## Anti-Patterns

- **Never execute interactive `limactl shell` or `limactl edit` via `cli-exec`** — non-interactive subprocesses have no TTY/`$EDITOR` and hang; use `lima-shell` for one-shot commands or `[NAVIGATE: terminal]` for interactive sessions.
- **Never pass shell operators (`&&`, `|`, `;`) inside `cli-exec` arguments**.
