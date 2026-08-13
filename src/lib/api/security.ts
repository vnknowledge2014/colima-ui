import { call } from "./client";

/**
 * Image vulnerability scanning. Client only — the Security UI is phase 3.
 *
 * Mirrors `src-tauri/src/commands/security_scan.rs`. The scanner (Trivy) is a
 * host tool the app drives; whether it is installed is answered by the existing
 * capability list under the id `trivy`, not by a second endpoint here.
 */

export type Severity = "critical" | "high" | "medium" | "low" | "unknown";

export interface Finding {
  id: string;
  package: string;
  installedVersion: string;
  /** Absent when no fix exists — which is what decides whether it is actionable. */
  fixedVersion?: string;
  severity: Severity;
  source?: string;
  cvss?: number;
  published?: string;
}

export interface ScanResult {
  imageRef: string;
  /** Content identity, and the cache key: a tag moves, a digest does not. */
  imageDigest: string;
  findings: Finding[];
  scanner: "trivy";
  scannerVersion: string;
  /** When the vulnerability database was last updated. Explains a result. */
  dbSnapshotDate?: string;
  scannedAt: number;
}

export type SbomFormat = "cyclone_dx" | "spdx";

/**
 * Progress for a running scan, delivered on the SSE stream under
 * `security-scan-progress`.
 *
 * `stage` matters more than `bytes`: the first scan after a few days spends most
 * of its time downloading a 1.2 GB database, and reporting that as "scanning"
 * would look like a hang.
 */
export interface ScanProgress {
  scanId: string;
  stage: "database" | "scan";
  bytes: number;
}

/** Strictness. Higher levels enable more rules and can only lower a score. */
export type Level = "l1" | "l2" | "l3";

export type RuleComponent = "hardening" | "provenance" | "freshness";

/**
 * A pointer into a published standard — identifiers only.
 *
 * There is deliberately no text field: CIS and the OWASP Docker Top 10 are both
 * CC BY-NC-SA and this is a commercial product, so their prose cannot ship here.
 * The rule's own `title`/`rationale`/`remediation` are written by this project.
 */
export interface StandardRef {
  standard: string;
  version?: string;
  id: string;
}

export interface Rule {
  id: string;
  title: string;
  rationale: string;
  remediation: string;
  severity: Severity;
  component: RuleComponent;
  weight: number;
  minLevel: Level;
  standardRefs: StandardRef[];
}

export interface RulePack {
  packVersion: string;
  componentMax: Record<RuleComponent, number>;
  rules: Rule[];
}

export interface RuleResult {
  ruleId: string;
  passed: boolean;
  severity: Severity;
  component: RuleComponent;
  weight: number;
  /** What was found. Never a secret's value — only the variable's name. */
  evidence?: string;
}

export interface ComponentScore {
  earned: number;
  max: number;
  failedRules: string[];
}

/**
 * What makes a score comparable. Show these next to the number, not in a
 * tooltip: a score from a three-week-old database is a different number, and two
 * scanners disagree by more than 10× on the same image.
 */
export interface ScoreInputs {
  packVersion: string;
  /** Which build evaluated the pack — see `skippedRules`. */
  engineVersion: string;
  scanner: string;
  scannerVersion: string;
  dbSnapshotDate?: string;
  level: Level;
  /**
   * Rules the backend could not run, because the pack named a check this build
   * does not implement. Their weight leaves the denominator rather than being
   * counted as a pass, so `ComponentScore.max` can be below the pack's own
   * maximum — show it, or the score looks arbitrary.
   */
  skippedRules?: string[];
}

export interface ScoreBreakdown {
  vulnerabilities: ComponentScore;
  hardening: ComponentScore;
  provenance: ComponentScore;
  freshness: ComponentScore;
  total: number;
  inputs: ScoreInputs;
}

export interface SkippedRule {
  ruleId: string;
  component: RuleComponent;
  weight: number;
}

export interface Evaluation {
  results: RuleResult[];
  /** Rules named by the pack that this build cannot run. */
  skipped: SkippedRule[];
}

export interface SecurityAudit {
  scan: ScanResult;
  evaluation: Evaluation;
  score: ScoreBreakdown;
}

export interface Alternative {
  image: string;
  /** What changes if you swap. Deliberately not an instruction. */
  why: string;
}

export interface CatalogSuggestions {
  catalogVersion: string;
  /** When the table was last revised — show it, so stale advice looks stale. */
  updatedAt: string;
  alternatives: Alternative[];
}

// A security patch is a compose patch with a rule attached: one patch shape and
// one apply path, so a fix cannot go wrong in two different ways.
import type { ComposePatch } from "./compose";

/** How much work a fix is. Mirrors `Effort` in `security_triage.rs`. */
export type Effort = "oneclick" | "review" | "manual";

/** Mirrors `TriageItem` in `security_triage.rs`. */
export interface TriageItem {
  ruleId: string;
  title: string;
  severity: Severity;
  component: RuleComponent;
  effort: Effort;
  /** Points the total gains if this rule alone starts passing. Measured by
   *  re-scoring — never a model's estimate. */
  scoreDeltaEst: number;
  whyFirst: string;
  remediation: string;
  evidence?: string;
}

/** Mirrors `TriageResult` in `security_triage.rs`. */
export interface TriageResult {
  items: TriageItem[];
  currentScore: number;
  /** Upper bound: every listed fix done. */
  potentialScore: number;
  /** Redacted. Nothing is sent until the user asks. */
  llmPayloadPreview: string;
}

/** Mirrors `SecurityPatch` in `security_autofix.rs`. */
export interface SecurityPatch {
  rule_id: string;
  patch: ComposePatch;
  /** False when applying could break a build that currently works. */
  auto_applicable: boolean;
  risk_note?: string | null;
}

/** Mirrors `ScanRun` in `security_history.rs`. */
export interface ScanRun {
  imageRef: string;
  imageDigest: string;
  score: number;
  breakdownJson: string;
  /** Two runs are only comparable under the same pack and scanner. */
  packVersion: string;
  scanner: string;
  scannerVersion: string;
  dbSnapshotDate?: string | null;
  critical: number;
  high: number;
  medium: number;
  low: number;
  scannedAt: number;
}

/** Mirrors `PolicyRule` in `security_policy.rs`. */
export interface PolicyRule {
  id: number;
  name: string;
  /** `*`, a `prefix*`, or an exact image reference. */
  pattern: string;
  minScore?: number;
  maxSeverity?: Severity;
  /** Warn is the only action. Blocking is a separate decision, not yet taken. */
  action: "warn";
  enabled: boolean;
}

/** Mirrors `WatchState` in `security_watch.rs`. */
export interface WatchState {
  enabled: boolean;
  intervalHours: number;
  /** Whether a thread is alive — not the same as `enabled` once Pro lapses. */
  running: boolean;
}

export const securityApi = {
  /**
   * Scan one image. `scanId` is chosen by the caller so the scan can be
   * cancelled before its process even exists.
   */
  scan: (scanId: string, imageRef: string, refresh = false) =>
    call<ScanResult>(
      "security_scan_image",
      { scanId, imageRef, refresh },
      "POST",
      "/api/security/scan",
      undefined,
      { scanId, imageRef, refresh },
    ),

  /**
   * Scan, evaluate the configuration rules, and score — in one request.
   *
   * One call rather than three: a score only means something beside the
   * findings and rule results it came from, and assembling it here is how a
   * score ends up rendered next to a different image's scan.
   */
  audit: (scanId: string, imageRef: string, level: Level = "l1", refresh = false) =>
    call<SecurityAudit>(
      "security_audit_image",
      { scanId, imageRef, level, refresh },
      "POST",
      "/api/security/audit",
      undefined,
      { scanId, imageRef, level, refresh },
    ),

  /** The rule pack this build carries: titles, rationale, remediation. */
  rules: () => call<RulePack>("security_rule_pack", undefined, "GET", "/api/security/rules"),

  /** This image's recorded scans, oldest first. Pro. */
  history: (imageDigest: string) =>
    call<ScanRun[]>("security_score_history", { imageDigest }, "GET", "/api/security/history", {
      imageDigest,
    }),

  policyList: () =>
    call<PolicyRule[]>("security_policy_list", undefined, "GET", "/api/security/policy"),
  policySave: (rule: PolicyRule) =>
    call<number>("security_policy_save", { rule }, "POST", "/api/security/policy", undefined, rule),
  policyDelete: (id: number) =>
    call<boolean>("security_policy_delete", { id }, "POST", "/api/security/policy/delete", undefined, {
      id,
    }),

  /**
   * Scheduled rescans. Reading the state is never gated, and neither is
   * switching them **off** — a stop control behind the gate of the feature it
   * stops leaves a lapsed user watching a job they cannot reach.
   */
  watchState: () => call<WatchState>("security_watch_state", undefined, "GET", "/api/security/watch"),
  watchSetEnabled: (enabled: boolean) =>
    call<WatchState>("security_watch_set_enabled", { enabled }, "POST", "/api/security/watch", undefined, {
      enabled,
    }),
  watchSetInterval: (hours: number) =>
    call<WatchState>("security_watch_set_interval", { hours }, "POST", "/api/security/watch", undefined, {
      intervalHours: hours,
    }),

  /**
   * Order this image's failing rules by what fixing each is worth.
   *
   * The audit is sent back rather than re-run: triage is a re-scoring of a
   * result the caller already holds, and scanning again would be both slower
   * and a different measurement.
   */
  triage: (audit: SecurityAudit, level: Level = "l1") =>
    call<TriageResult>("security_triage", { audit, level }, "POST", "/api/security/triage", undefined, {
      audit,
      level,
    }),

  /**
   * Concrete Dockerfile patches for the rules that have one.
   *
   * Takes a path, not the file's text: the backend reads it, so the contents
   * never travel through the client, and the name is checked before anything
   * is opened.
   */
  autofixPropose: (dockerfilePath: string) =>
    call<SecurityPatch[]>(
      "security_autofix_propose",
      { dockerfilePath },
      "POST",
      "/api/security/autofix/propose",
      undefined,
      { dockerfilePath },
    ),

  /**
   * Base images worth considering instead of this one.
   *
   * Answered from a table the app already carries — the image name never leaves
   * the machine, which is the point of shipping a catalog rather than querying
   * one.
   */
  alternatives: (imageRef: string) =>
    call<CatalogSuggestions>(
      "security_alternatives",
      { imageRef },
      "GET",
      "/api/security/alternatives",
      { image: imageRef },
    ),

  cancel: (scanId: string) =>
    call<boolean>(
      "security_scan_cancel",
      { scanId },
      "POST",
      "/api/security/scan/cancel",
      undefined,
      { scanId },
    ),

  /**
   * Write an SBOM to the folder the user chose. Returns the written path.
   *
   * Refuses an existing file unless `overwrite` says otherwise, and writes
   * through a scratch file so a failure never leaves a half-written document.
   */
  exportSbom: (
    imageRef: string,
    destDir: string,
    fileName: string,
    format: SbomFormat,
    overwrite = false,
  ) =>
    call<string>(
      "security_sbom_export",
      { imageRef, destDir, fileName, format, overwrite },
      "POST",
      "/api/security/sbom",
      undefined,
      { imageRef, destDir, fileName, format, overwrite },
    ),
};
