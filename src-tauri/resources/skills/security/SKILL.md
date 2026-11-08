---
name: security
description: Image vulnerability scanning, runtime security (Trivy, SBOM, Falco, Detonation), and 4-stage source security audit.
---

# Security & Source Audit Skill

ColimaUI provides both an interactive **Container & Runtime Security** center for images and running workloads, and a **4-Stage Source Security Pipeline** for auditing repositories, Dockerfiles, and Kubernetes manifests.

## Section 1: Built-in Container & Runtime Security (Event Bus + `[NAVIGATE: security]`)

Use direct Event Bus tools (`[QUERY]` for read-only inspection, `[EVENT_APPROVE]` for scans, SBOM export, and sandbox detonation) alongside `[NAVIGATE: security]`:

### Read-Only Security & Runtime Queries (`SAFE`)
- **Scan history**: `[QUERY: security-history | {"digest": "sha256:...", "limit": 20}]` (`digest` and `limit` optional)
- **Configuration hardening rule pack (`L1`–`L3`)**: `[QUERY: security-rules]`
- **Curated base-image alternatives**: `[QUERY: security-alternatives | {"image": "node:20"}]`
- **Falco runtime availability & state**: `[QUERY: falco-state]`
- **Falco runtime security events**: `[QUERY: falco-events | {"limit": 50}]`
- **Detonation sandbox sessions**: `[QUERY: detonation-list]`

### Image Scanning, SBOM & Detonation Actions (`NORMAL` / `DANGEROUS`)
- **Local Trivy CVE scan**: `[EVENT_APPROVE: security-scan | {"image": "nginx:latest", "refresh": false}]`
- **Full `/100` provenance security audit**: `[EVENT_APPROVE: security-audit | {"image": "nginx:latest", "level": "L2", "refresh": false}]`
- **Export SBOM (`cyclonedx` / `spdx`)**: `[EVENT_APPROVE: security-sbom | {"image": "nginx:latest", "format": "cyclonedx"}]`
- **Rank & triage failing audit rules**: `[EVENT_APPROVE: security-triage | {"image": "nginx:latest", "level": "L2"}]`
- **Start isolated Container Detonation sandbox**: `[EVENT_APPROVE: detonation-start | {"image": "suspect:latest", "durationSecs": 30, "command": "..."}]` ⚠️ DANGEROUS — executes the suspect image inside an isolated behavioral monitoring sandbox
- **Cancel running Detonation session**: `[EVENT_APPROVE: detonation-cancel | {"sessionId": "<session-id>"}]`

> **Visual Security Center**: Open `[NAVIGATE: security]` for interactive score breakdowns, security policy gates, AI explanations of Falco detections, and live SSE detonation timelines.

## Section 2: 4-Stage Source Security Pipeline

Use the 4-stage pipeline tags to audit application source code, Dockerfiles, Compose files, and Kubernetes manifests:

1. **Threat Model** → Identify attack surface and threats (STRIDE)
   `[SECURITY_THREAT_MODEL: /path/to/project | bootstrap]`
   Modes: `interview` | `bootstrap` | `bootstrap-then-interview`
   Output: `THREAT_MODEL.md`

2. **Vulnerability Scan** → Static source-code and config review
   `[SECURITY_VULN_SCAN: /path/to/project]`
   Uses `THREAT_MODEL.md` if present for focus areas
   Output: `VULN-FINDINGS.json` + `VULN-FINDINGS.md`

3. **Triage** → Verify, deduplicate, and rank findings
   `[SECURITY_TRIAGE: /path/to/VULN-FINDINGS.json]`
   Output: `TRIAGE.json` + `TRIAGE.md`

4. **Patch Generation** → Generate candidate diffs (NEVER auto-applied)
   `[SECURITY_PATCH_GEN: /path/to/TRIAGE.json | /path/to/repo]`
   Output: `PATCHES/` directory + `PATCHES.md`

## Quick Commands
- "Scan `nginx:latest` for CVEs / audit score" → `[EVENT_APPROVE: security-audit | {"image": "nginx:latest", "level": "L2"}]` (or `security-scan`)
- "Export SBOM for `nginx:latest`" → `[EVENT_APPROVE: security-sbom | {"image": "nginx:latest", "format": "cyclonedx"}]`
- "Suggest safer base images / inspect Falco events" → `[QUERY: security-alternatives | {"image": "..."}]` / `[QUERY: falco-events | {"limit": 50}]` + `[NAVIGATE: security]`
- "Scan this project for security issues" → stages 1 + 2 (`[SECURITY_THREAT_MODEL]` → `[SECURITY_VULN_SCAN]`)
- "Full security audit" → stages 1 + 2 + 3 (`[SECURITY_THREAT_MODEL]` → `[SECURITY_VULN_SCAN]` → `[SECURITY_TRIAGE]`)
- "Fix the top 3 vulnerabilities" → stage 4 (`[SECURITY_PATCH_GEN]`)

```text
# Example: read-only security posture check before proposing an image audit
[QUERY: security-rules]
[QUERY: security-alternatives | {"image": "node:20"}]
[QUERY: falco-state]
```

## Error Handling & Safety Rules
- If Trivy is not installed (`not_installed` error on `security-scan` / `security-audit`), check `[QUERY: system-check]` and guide the user to install `trivy` via Homebrew or `[NAVIGATE: security]`.
- All 4 source pipeline stages are READ-ONLY — no arbitrary code execution, no external network probing.
- `detonation-start` is **DANGEROUS** — always warn the user and confirm `image`, `durationSecs`, and optional `command` before proposing.
- `SECURITY_PATCH_GEN` writes inert candidate diffs to `PATCHES/` — NEVER auto-applies changes to source files.
- Always warn the user to review generated diffs manually before applying.

## Anti-Patterns

- **Never run external Trivy or Falco CLI commands manually via `cli-exec` when inspecting local container images** — always use the built-in Event Bus operations (`security-scan`, `security-audit`, `security-sbom`, `security-triage`, `security-history`, `security-rules`, `security-alternatives`, `falco-state`, `falco-events`, `detonation-*`) and `[NAVIGATE: security]`.
- **Never propose `detonation-start` without warning that the suspect image will run inside an isolated sandbox session**.
- **Never auto-apply candidate patches from `SECURITY_PATCH_GEN`** — diffs in `PATCHES/` must always be reviewed and applied by the user.
