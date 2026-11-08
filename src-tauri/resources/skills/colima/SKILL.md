---
name: colima
description: Colima VM instance management through App API. Start, stop, create, and troubleshoot instances.
---

# Colima Instance Management Skill

Manage Colima VMs (instances) through the App Event Bus.

## Core Rule: App-First

**ALWAYS use `[QUERY]` for introspection and `[EVENT_APPROVE]` for lifecycle actions.** Only use `cli-exec` for non-interactive one-shot guest commands inside a running VM.

## 1. Introspection (Read-Only)

- **List all instances**: `[QUERY: list-instances]`
- **Instance status**: `[QUERY: colima-status | {"profile": "default"}]`
- **Read `colima.yaml` config**: `[QUERY: colima-config-get | {"profile": "default"}]` (returns structured config + `mtime`)
- **Instance diagnostic logs**: `[QUERY: diagnostic-logs | {"profile": "default"}]`
- **Resolved paths & env (`$COLIMA_HOME`, XDG `~/.config/colima` vs legacy `~/.colima`, Lima `_lima/colima`, Docker socket, `launchd` vs shell env)**: `[QUERY: environment-resolutions]` (or open `[NAVIGATE: environment]`)
- **Host specs** (for preset recommendation): `[QUERY: system-host-specs]`

## 2. Lifecycle & Config (State Changes)

- **Create instance**: `[EVENT_APPROVE: create-instance | {"profile": "dev", "cpus": 4, "memory": 8, "disk": 60, "runtime": "docker", "vm_type": "vz", "kubernetes": false}]`
- **Start instance**: `[EVENT_APPROVE: start-instance | {"profile": "default"}]`
- **Stop instance**: `[EVENT_APPROVE: stop-instance | {"profile": "default"}]`
- **Restart instance**: `[EVENT_APPROVE: colima-restart | {"profile": "default"}]`
- **Apply `colima.yaml` config changes**: `[EVENT_APPROVE: colima-config-apply | {"profile": "default", "changes": {"cpu": 4, "memory": 8}, "expectedMtime": 1730980000}]`
- **Delete instance**: `[EVENT_APPROVE: delete-instance | {"profile": "default"}]` ⚠️ DANGEROUS — deletes ALL data

## 3. Consultant Mode

When creating a new instance, ALWAYS interview the user step by step:
1. Run `[QUERY: system-host-specs]` first (alone, per the Single-Answer Tool Rule) to inspect host CPU cores, RAM, and macOS architecture.
2. Interview the user with `[ASK_CHOICE: ...]` (1 structured choice card per turn) to select:
   - **Workload profile / hardware preset** (e.g., Minimal, Development, Standard, Power, or Kubernetes with K3s enabled) tailored to host specs
   - **Mount type**: `virtiofs` (fastest native macOS Virtualization.framework mount) vs `sshfs` (broad compatibility)
   - **VM type**: `vz` (Apple Virtualization.framework, recommended on macOS 13+) vs `qemu` (legacy/cross-arch emulation)
3. Present the final configuration summary and wait for user confirmation before emitting `[EVENT_APPROVE: create-instance | ...]`.

## 4. CLI Escape Hatch

> **Sandbox Warning**: `cli-exec` runs without a shell or PTY — interactive TTY sessions hang, and `~` is NOT expanded in arguments.

- **Non-interactive one-shot command inside VM**:
  `[EVENT_APPROVE: cli-exec | {"command": "colima", "args": ["ssh", "-p", "default", "--", "uname", "-a"]}]`
- **Viewing or editing `colima.yaml`**: Do NOT read config paths with `~` via `cli-exec` (`runSandboxed` does not invoke a shell to expand `~`). Colima `0.10.x` defaults to XDG `~/.config/colima/<profile>/colima.yaml` while older installs use `~/.colima/<profile>/colima.yaml` (or `$COLIMA_HOME`). Always use `[QUERY: colima-config-get | {"profile": "default"}]` and `[EVENT_APPROVE: colima-config-apply | {"profile": "default", "changes": {...}, "expectedMtime": ...}]`, or open `[NAVIGATE: settings]` (`[QUERY: navigate | {"page": "settings", "section": "colima-config"}]`) for the interactive visual diff editor, inspect active paths in `[NAVIGATE: environment]`, or open interactive SSH in `[NAVIGATE: terminal]`.
- **Interactive SSH session**: Navigate to `[NAVIGATE: terminal]` to open a full persistent PTY shell inside the Colima VM.

## Anti-Patterns

- **Never run interactive `colima ssh` without `-- <command>` via `cli-exec`** — non-PTY subprocesses hang waiting for terminal input; use `[NAVIGATE: terminal]` for interactive SSH.
- **Never pass `~` paths in `cli-exec` arguments** — `runSandboxed` executes binaries directly without shell tilde expansion; use `colima-config-get` / `colima-config-apply` or `[NAVIGATE: settings]` to inspect and edit `colima.yaml`.
- **Never emit `create-instance` without checking host specs first** — query `system-host-specs` and confirm options via `[ASK_CHOICE]` before creating a VM.
