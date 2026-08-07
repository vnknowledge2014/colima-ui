---
name: diagnostics
description: System diagnostics and troubleshooting for ColimaUI. Collect logs, check prerequisites, and debug issues.
---

# Diagnostics Skill

Use this skill ONLY when the user explicitly reports a bug, error, or failure. NEVER use proactively.

## Before Running Full Diagnostics

**Try lightweight queries first** — they are faster and often sufficient:

1. `[QUERY: colima-status | {"profile": "default"}]` — Is the VM running?
2. `[QUERY: system-check]` — Are prerequisites (colima, docker, lima) installed?
3. `[QUERY: system-check-tool | {"name": "kubectl"}]` — Is a specific tool available?
4. `[QUERY: k8s-cluster-health]` — Is the K8s cluster healthy?
5. `[QUERY: system-df]` — Is disk full?
6. `[QUERY: k8s-events | {"namespace": "all"}]` — Any K8s warnings?

## Full Diagnostics

Only if lightweight queries don't reveal the issue:

- `[DIAGNOSE]` — Collects comprehensive logs (colima status, docker info, process locks, lima logs). This is expensive — avoid unless necessary.

## Common Issues

### "Cannot connect to Docker daemon"
1. `[QUERY: list-instances]` — check if any instance is Running.
2. If none running: `[EVENT_APPROVE: start-instance | {"profile": "default"}]`
3. If running but Docker unresponsive: `[EVENT_APPROVE: colima-restart | {"profile": "default"}]`

### "kubectl: connection refused"
1. `[QUERY: k8s-current-context]` — is context set?
2. `[QUERY: colima-status | {"profile": "default"}]` — is K8s enabled on the instance?
3. If not: recreate instance with `kubernetes: true`.
