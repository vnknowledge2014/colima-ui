---
name: limavm
description: Lima VM management through App API. Create, start, stop, and shell into lightweight Linux VMs.
---

# Lima VM Management Skill

Manage Lima virtual machines (without Docker/K8s) through the App Event Bus.

## Core Rule: App-First

**ALWAYS use `[QUERY]` for introspection and `[EVENT_APPROVE]` for lifecycle/shell.** Only use CLI for advanced `limactl` config editing.

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

- **Run command in VM**: `[EVENT_APPROVE: lima-shell | {"name": "ubuntu", "command": "uname -a"}]`

## 4. CLI Escape Hatch

- **Direct shell**: `[EVENT_APPROVE: cli-exec | {"command": "limactl", "args": ["shell", "ubuntu"]}]`
- **Edit VM config**: `[EVENT_APPROVE: cli-exec | {"command": "limactl", "args": ["edit", "ubuntu"]}]`
