---
name: colima
description: Colima VM instance management through App API. Start, stop, create, and troubleshoot instances.
---

# Colima Instance Management Skill

Manage Colima VMs (instances) through the App Event Bus.

## Core Rule: App-First

**ALWAYS use `[QUERY]` for introspection and `[EVENT_APPROVE]` for lifecycle actions.** Only use CLI for `colima ssh` internal VM debugging.

## 1. Introspection (Read-Only)

- **List all instances**: `[QUERY: list-instances]`
- **Instance status**: `[QUERY: colima-status | {"profile": "default"}]`
- **Host specs** (for preset recommendation): `[QUERY: system-host-specs]`

## 2. Lifecycle (State Changes)

- **Create instance**: `[EVENT_APPROVE: create-instance | {"profile": "dev", "cpus": 4, "memory": 8, "disk": 60, "runtime": "docker", "vm_type": "vz", "kubernetes": false}]`
- **Start instance**: `[EVENT_APPROVE: start-instance | {"profile": "default"}]`
- **Stop instance**: `[EVENT_APPROVE: stop-instance | {"profile": "default"}]`
- **Restart instance**: `[EVENT_APPROVE: colima-restart | {"profile": "default"}]`
- **Delete instance**: `[EVENT_APPROVE: delete-instance | {"profile": "default"}]` ⚠️ DANGEROUS — deletes ALL data

## 3. Consultant Mode

When creating a new instance, ALWAYS interview the user:
1. Ask about use case (Docker dev, K8s testing, heavy build)
2. `[QUERY: system-host-specs]` to check available resources
3. Recommend a preset based on host specs
4. Present a plan with all config options
5. Wait for user confirmation before `create-instance`

## 4. CLI Escape Hatch

- **SSH into VM**: `[EVENT_APPROVE: cli-exec | {"command": "colima", "args": ["ssh", "<profile>"]}]`
- **Read VM config**: `[EVENT_APPROVE: cli-exec | {"command": "cat", "args": ["~/.colima/<profile>/colima.yaml"]}]`
