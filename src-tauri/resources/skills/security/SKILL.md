---
name: security
description: Image vulnerability scanning, runtime security (Trivy, SBOM, Falco, Detonation), and 4-stage source security audit.
---

# Security & Source Audit Skill

ColimaUI provides both an interactive **Container & Runtime Security** center for images and running workloads, and a **4-Stage Source Security Pipeline** for auditing repositories, Dockerfiles, and Kubernetes manifests.

## Section 1: Built-in Container & Runtime Security (`[NAVIGATE: security]`)

Direct users to `[NAVIGATE: security]` for built-in image and runtime security capabilities:

- **Local Trivy Image Scanning & Security Score**: Scans local container images for CVEs and computes a provenance-backed security score alongside scan history and policy gates.
- **SBOM Export**: Generates and exports Software Bills of Materials (CycloneDX / SPDX) for audited images.
- **Configuration Rule-Pack Checks**: Evaluates container/image configuration against L1/L2 hardening rule packs, custom security policies, and continuous watch state, with ranked triage and security autofix proposals.
- **Curated Base-Image Alternatives**: Recommends hardened, minimal, or slimmer base-image replacements for vulnerable or bloated images.
- **Falco Runtime Event Stream**: Monitors live Falco runtime security detections inside the VM with one-click AI explanations of suspicious syscalls or container behavior.
- **Container Detonation Sandbox**: Runs untrusted or suspect images inside an isolated detonation session, streaming a live behavioral timeline over SSE and exporting session reports.

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
- "Check this image for CVEs / export SBOM / inspect Falco events" → `[NAVIGATE: security]`
- "Scan this project for security issues" → stages 1 + 2 (`[SECURITY_THREAT_MODEL]` → `[SECURITY_VULN_SCAN]`)
- "Full security audit" → stages 1 + 2 + 3 (`[SECURITY_THREAT_MODEL]` → `[SECURITY_VULN_SCAN]` → `[SECURITY_TRIAGE]`)
- "Fix the top 3 vulnerabilities" → stage 4 (`[SECURITY_PATCH_GEN]`)

## Safety Rules
- All 4 source pipeline stages are READ-ONLY — no arbitrary code execution, no external network probing.
- `SECURITY_PATCH_GEN` writes inert candidate diffs to `PATCHES/` — NEVER auto-applies changes to source files.
- Always warn the user to review generated diffs manually before applying.

## Anti-Patterns

- **Never run external Trivy or Falco CLI commands manually via `cli-exec` when inspecting local container images** — direct the user to `[NAVIGATE: security]` for integrated scanning, SBOM export, rule-pack scoring, and Falco event analysis.
- **Never auto-apply candidate patches from `SECURITY_PATCH_GEN`** — diffs in `PATCHES/` must always be reviewed and applied by the user.
