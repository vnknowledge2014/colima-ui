# Context Prompt Router

The following skills are available to you. Each skill documents App Event Bus operations — use `[QUERY]` for read-only and `[EVENT_APPROVE]` for state changes. Use `[READ_REFERENCE: path]` to load detailed references.

## Available Skills

1. **ColimaUI App Event Catalog (`skills/colima_ui/SKILL.md`)**
   - **READ THIS FIRST.** Complete symmetric catalog of **130 events** (`62 SAFE` + `68 NORMAL/DANGEROUS`, shared 1-for-1 across In-App AI Chat and Headless/A2A `cui`) across all domains (Instances & `colima.yaml` config get/apply, Containers, Images, Volumes, Networks, Compose Diagnose & Autofix, Topology, Activity/Alerts/Self-Heal, Container Security Trivy/SBOM/Falco/Detonation, K8s, Kind, Lima, Models & Settings, System, Navigation & Gateway) plus all **17** navigable UI pages and `ai-chat` drawer toggle.
   - Includes payload schemas, category (SAFE/NORMAL/DANGEROUS), Consultant Mode `[ASK_CHOICE]` / `[SUGGEST]` patterns, and usage examples for every event.

2. **Colima VM Management (`skills/colima/SKILL.md`)**
   - App-First instance lifecycle & configuration via Event Bus: create, start, stop, restart, delete, Consultant Mode hardware/mount/VM-type selection, direct `colima.yaml` config inspection/editing (`[QUERY: colima-config-get]`, `[EVENT_APPROVE: colima-config-apply]`, or `[NAVIGATE: settings]`), VM diagnostics (`[QUERY: diagnostic-logs]`), and XDG path provenance via `[QUERY: environment-resolutions]` / `[NAVIGATE: environment]`.
   - References available:
     - `skills/colima/references/networking_and_dns.md`: SSHFS, VZ, DNS resolution.
     - `skills/colima/references/mount_volumes.md`: Mounting host directories.
     - `skills/colima/references/security_socket.md`: Docker socket security permissions.
     - `skills/colima/references/performance_tuning.md`: CPU/RAM allocation.

3. **Docker Container & Image Management (`skills/docker/SKILL.md`)**
   - App-First container introspection, image builds (`build-image`), lifecycle, topology (`[QUERY: topology-get]`), and observability (`[QUERY: activity-feed]`, `alerts-rules`, `alerts-events`, `self-heal-rules`, `self-heal-log`) via `[QUERY]` and `[EVENT_APPROVE]`.
   - Native UI support for Image TAR Save/Load (`[NAVIGATE: images]`), Container File Copy & interactive Container PTY `dockerExec` (`[NAVIGATE: containers]` / `[NAVIGATE: terminal]`), visual network/volume graph (`[NAVIGATE: topology]`), and live metrics/alerts (`[NAVIGATE: activity]`).
   - CLI escape only for: `docker push`, multi-platform `docker buildx`.
   - References available:
     - `skills/docker/references/rootless_security.md`: Rootless mode, container breakout security.
     - `skills/docker/references/network_isolation.md`: Isolated bridge networks, iptables.

4. **Docker Compose Management (`skills/docker_compose/SKILL.md`)**
   - App-First Compose project lifecycle (`compose-list`, `compose-ps`, `compose-up`, `compose-down`, `compose-start`, `compose-stop`, `compose-restart`, `compose-build`, `compose-pull`, `compose-logs`) plus direct Event Bus **Compose Diagnose** (`compose-diagnose`) and **AI Autofix** (`compose-autofix-propose`, `compose-autofix-apply`, `compose-autofix-undo`, `compose-autofix-history`) alongside `[NAVIGATE: compose]`.
   - CLI escape only for: `compose config`.
   - References available:
     - `skills/docker_compose/references/secrets_management.md`: Managing secrets securely in compose.

5. **Kubernetes Management (`skills/kubernetes/SKILL.md`)**
   - App-First K8s management via 27 Event Bus operations (including `k8s-pods`, plus 3 Kind cluster events). Replaces all standard `kubectl` commands.
   - CLI escape only for: `helm` commands, advanced CRD mutations.
   - References available:
     - `skills/kubernetes/references/colima_k3s_quirks.md`: Specific quirks when running K3s inside Colima.
     - `skills/kubernetes/references/rbac_security.md`: Role-Based Access Control and Service Accounts.

6. **Lima VM Management (`skills/limavm/SKILL.md`)**
   - App-First Lima VM lifecycle and one-shot shell commands (`lima-shell`) via Event Bus; interactive shell via `[NAVIGATE: terminal]`.
   - References available:
     - `skills/limavm/references/vm_security.md`: Host-guest isolation.

7. **Diagnostics & Troubleshooting (`skills/diagnostics/SKILL.md`)**
   - QUERY-first troubleshooting: inspect path/socket provenance via `[QUERY: environment-resolutions]` / `[NAVIGATE: environment]`, fetch VM diagnostic logs via `[QUERY: diagnostic-logs]`, and run lightweight `[QUERY]` calls before resorting to `[DIAGNOSE]`.
   - References available:
     - `skills/diagnostics/references/read_logs.md`: Colima and macOS log locations.
     - `skills/diagnostics/references/process_locks.md`: Stale `ha.sock` locks and zombie QEMU/VZ hypervisor processes.

8. **Security & Source Audit (`skills/security/SKILL.md`)**
   - **In-App Container & Runtime Security (`[NAVIGATE: security]` + 12 Event Bus operations)**: Direct `[QUERY]` / `[EVENT_APPROVE]` tools for local Trivy image vulnerability scanning (`security-scan`, `security-audit`, `security-triage`, `security-history`), SBOM export (`security-sbom`), rule packs & base-image alternatives (`security-rules`, `security-alternatives`), Falco runtime events (`falco-state`, `falco-events`), and isolated Detonation sandbox (`detonation-list`, `detonation-start`, `detonation-cancel`).
   - **4-Stage Source Security Pipeline**: Threat Model (`[SECURITY_THREAT_MODEL]`) → Vuln Scan (`[SECURITY_VULN_SCAN]`) → Triage (`[SECURITY_TRIAGE]`) → Patch Gen (`[SECURITY_PATCH_GEN]`). Read-only audit — candidate patches are inert diffs for human review.

## Routing Logic

When the user asks a question:
1. Check if the action maps to an event in the **App Event Catalog** (skill #1) or a built-in UI workflow (`[NAVIGATE: ...]`). If yes, use `[QUERY]`, `[EVENT_APPROVE]`, or `[NAVIGATE]`.
2. If domain-specific knowledge is needed, load the relevant **SKILL.md** via `[READ_REFERENCE]`.
3. If deep context is needed (e.g., stale socket/process locks, rootless security, K3s quirks), load a **references/** file via `[READ_REFERENCE]`.
4. Only fall back to CLI (`cli-exec`) if no App event or native UI feature exists for the operation.
