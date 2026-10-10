# Context Prompt Router

The following skills are available to you. Each skill documents App Event Bus operations — use `[QUERY]` for read-only and `[EVENT_APPROVE]` for state changes. Use `[READ_REFERENCE: path]` to load detailed references.

## Available Skills

1. **ColimaUI App Event Catalog (`skills/colima_ui/SKILL.md`)**
   - **READ THIS FIRST.** Complete catalog of 92 events across 10 domains (Instances, Containers, Images, Volumes, Networks, Compose, K8s, Kind, Lima, Models, System).
   - Includes payload schemas, category (SAFE/NORMAL/DANGEROUS), and usage examples for every event.

2. **Colima VM Management (`skills/colima/SKILL.md`)**
   - App-First instance lifecycle via Event Bus: create, start, stop, restart, delete.
   - References available:
     - `skills/colima/references/networking_and_dns.md`: SSHFS, VZ, DNS resolution.
     - `skills/colima/references/mount_volumes.md`: Mounting host directories.
     - `skills/colima/references/security_socket.md`: Docker socket security permissions.
     - `skills/colima/references/performance_tuning.md`: CPU/RAM allocation.

3. **Docker Container & Image Management (`skills/docker/SKILL.md`)**
   - App-First container introspection and lifecycle via `[QUERY]` and `[EVENT_APPROVE]`.
   - CLI escape only for: docker build, docker push, docker save/load.
   - References available:
     - `skills/docker/references/rootless_security.md`: Rootless mode, container breakout security.
     - `skills/docker/references/network_isolation.md`: Isolated bridge networks, iptables.

4. **Docker Compose Management (`skills/docker_compose/SKILL.md`)**
   - App-First Compose project lifecycle via Event Bus.
   - CLI escape only for: compose up --build, compose config.
   - References available:
     - `skills/docker_compose/references/secrets_management.md`: Managing secrets securely in compose.

5. **Kubernetes Management (`skills/kubernetes/SKILL.md`)**
   - App-First K8s management via 26 Event Bus operations. Replaces all kubectl commands.
   - CLI escape only for: helm commands, advanced CRD mutations.
   - References available:
     - `skills/kubernetes/references/colima_k3s_quirks.md`: Specific quirks when running K3s inside Colima.
     - `skills/kubernetes/references/rbac_security.md`: Role-Based Access Control and Service Accounts.

6. **Lima VM Management (`skills/limavm/SKILL.md`)**
   - App-First Lima VM lifecycle and shell access via Event Bus.
   - References available:
     - `skills/limavm/references/vm_security.md`: Host-guest isolation.

7. **Diagnostics & Troubleshooting (`skills/diagnostics/SKILL.md`)**
   - QUERY-first troubleshooting: try lightweight `[QUERY]` calls before resorting to `[DIAGNOSE]`.
   - References available:
     - `skills/diagnostics/references/read_logs.md`: Colima and macOS log locations.

8. **Security Pipeline (`skills/security/SKILL.md`)**
   - 4-stage security audit: Threat Model → Vuln Scan → Triage → Patch Gen.
   - All stages are read-only — patches are inert diffs for human review.

## Routing Logic

When the user asks a question:
1. Check if the action maps to an event in the **App Event Catalog** (skill #1). If yes, use `[QUERY]` or `[EVENT_APPROVE]`.
2. If domain-specific knowledge is needed, load the relevant **SKILL.md** via `[READ_REFERENCE]`.
3. If deep context is needed, load a **references/** file.
4. Only fall back to CLI (`cli-exec`) if no App event exists for the operation.
