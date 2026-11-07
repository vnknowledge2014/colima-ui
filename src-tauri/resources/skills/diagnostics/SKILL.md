---
name: diagnostics
description: System diagnostics and troubleshooting for ColimaUI. Collect logs, check prerequisites, and debug issues.
---

# Diagnostics Skill

Use this skill ONLY when the user explicitly reports a bug, error, or failure. NEVER use proactively.

## Before Running Full Diagnostics

**Try lightweight queries first** — they are faster and often sufficient:

1. `[QUERY: colima-status | {"profile": "default"}]` — Is the VM running?
2. `[QUERY: environment-resolutions]` — Are `$COLIMA_HOME` (`~/.config/colima` vs `~/.colima`), Lima `_lima/colima`, Docker socket, or `launchd`-vs-shell `PATH` settings mismatched? (Also viewable in `[NAVIGATE: environment]`.)
3. `[QUERY: system-check]` — Are prerequisites (colima, docker, lima) installed?
4. `[QUERY: system-check-tool | {"name": "kubectl"}]` — Is a specific tool available?
5. `[QUERY: k8s-cluster-health]` — Is the K8s cluster healthy?
6. `[QUERY: system-df]` — Is disk full?
7. `[QUERY: k8s-events | {"namespace": "all"}]` — Any K8s warnings?

## Full Diagnostics

Only if lightweight queries don't reveal the issue:

- `[DIAGNOSE]` — Collects comprehensive logs (colima status, docker info, process locks, lima logs). This is expensive — avoid unless necessary.

## Common Issues

### "Cannot connect to Docker daemon"
1. `[QUERY: list-instances]` — check if any instance is Running.
2. `[QUERY: environment-resolutions]` — verify the resolved Docker socket and Colima home (`~/.config/colima` vs `~/.colima`), or open `[NAVIGATE: environment]`.
3. If none running: `[EVENT_APPROVE: start-instance | {"profile": "default"}]`
4. If running but Docker unresponsive: `[EVENT_APPROVE: colima-restart | {"profile": "default"}]`

### "kubectl: connection refused"
1. `[QUERY: k8s-current-context]` — is context set?
2. `[QUERY: environment-resolutions]` — check resolved `KUBECONFIG` and tool paths (or inspect `[NAVIGATE: environment]`).
3. `[QUERY: colima-status | {"profile": "default"}]` — is K8s enabled on the instance?
4. If not: recreate instance with `kubernetes: true`.

## Anti-Patterns

- **Never call `[DIAGNOSE]` proactively on greeting or simple status questions** — always try lightweight `[QUERY]` calls (`colima-status`, `system-check`, `list-containers`) first.
- **Never combine `[DIAGNOSE]` or `[QUERY]` with speculative answers or `[SUGGEST]` chips in the same turn** — emit only the read tool tag and wait for `[Tool Results]`.
