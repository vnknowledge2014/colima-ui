# Security Pipeline

ColimaUI integrates a 4-stage security pipeline for auditing container workloads, Kubernetes configs, and application code.

## Pipeline Stages
1. **Threat Model** → Identify attack surface and threats (STRIDE)
   `[SECURITY_THREAT_MODEL: /path/to/project | bootstrap]`
   Modes: interview | bootstrap | bootstrap-then-interview
   Output: THREAT_MODEL.md

2. **Vulnerability Scan** → Static source-code review
   `[SECURITY_VULN_SCAN: /path/to/project]`
   Uses THREAT_MODEL.md if present for focus areas
   Output: VULN-FINDINGS.json + VULN-FINDINGS.md

3. **Triage** → Verify, deduplicate, rank findings
   `[SECURITY_TRIAGE: /path/to/VULN-FINDINGS.json]`
   Output: TRIAGE.json + TRIAGE.md

4. **Patch Generation** → Generate candidate diffs (NEVER auto-applied)
   `[SECURITY_PATCH_GEN: /path/to/TRIAGE.json | /path/to/repo]`
   Output: PATCHES/ directory + PATCHES.md

## Quick Commands
- "Scan this project for security issues" → stages 1+2
- "Full security audit" → stages 1+2+3
- "Fix the top 3 vulnerabilities" → stage 4 with --top 3

## Safety Rules
- All stages are READ-ONLY — no code execution, no network probing
- Patch-gen writes diffs to PATCHES/ — NEVER applies to source
- User must review and apply diffs manually
