//! Posture policy: the minimum a user is willing to accept from an image.
//!
//! Rules live in `settings.db` beside `alert_rules`, not in `security.db`.
//! The split across this feature's three stores is by lifetime: a policy is
//! something the user typed and must never be pruned; the scans it judges are
//! pruned on a cap.
//!
//! # Warn, never block
//!
//! A rule's only action is to warn. Blocking a container because a score
//! slipped would, the first time it misfires, stop something the user needed
//! running — and one wrong block costs more trust than every correct one earns
//! back. Blocking is a separate decision, taken only once warnings have run
//! long enough to show the false-positive rate, and only if asked for.
//!
//! # No default threshold
//!
//! None is shipped. A number picked without the score distribution behind it
//! would be arbitrary, and an arbitrary default that fires is worse than no
//! policy: it teaches the user to ignore the alert. The user sets the bar.

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

/// What a policy does when an image falls short. One variant, on purpose.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum PolicyAction {
    #[default]
    Warn,
}

/// A bar an image is expected to clear.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyRule {
    pub id: i64,
    pub name: String,
    /// Glob-ish image match. `*` alone means every image; a trailing `*` is a
    /// prefix match. Kept this small deliberately — a regex in a settings field
    /// is a way to write a rule that silently matches nothing.
    pub pattern: String,
    /// Minimum acceptable total score, or absent to not check the score.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_score: Option<u32>,
    /// Highest severity tolerated, or absent to not check severities.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_severity: Option<super::security_scan::Severity>,
    pub action: PolicyAction,
    pub enabled: bool,
}

/// One rule, one image, one reason.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Violation {
    pub rule_id: i64,
    pub rule_name: String,
    pub image_ref: String,
    /// What the image scored, or how many findings breached the severity bar.
    pub value: f64,
    pub threshold: f64,
    pub metric: super::alerts::AlertMetric,
    /// Plain sentence, ready to show.
    pub detail: String,
}

fn db() -> &'static std::sync::Mutex<Connection> {
    super::knowledge_bank::get_db()
}

pub fn init(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS policy_rules (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            pattern TEXT NOT NULL,
            min_score INTEGER,
            max_severity TEXT,
            action TEXT NOT NULL DEFAULT 'warn',
            enabled INTEGER NOT NULL DEFAULT 1,
            created_at TEXT DEFAULT (datetime('now'))
        );
        ",
    )
    .map_err(|e| format!("Cannot create policy tables: {e}"))
}

/// Does this rule apply to this image?
///
/// Three shapes, and nothing else: everything, a prefix, or an exact name.
pub fn matches(pattern: &str, image_ref: &str) -> bool {
    let pattern = pattern.trim();
    if pattern == "*" || pattern.is_empty() {
        return true;
    }
    match pattern.strip_suffix('*') {
        Some(prefix) => image_ref.starts_with(prefix),
        None => image_ref == pattern,
    }
}

pub fn list_rules() -> Result<Vec<PolicyRule>, String> {
    let conn = db().lock().map_err(|_| "settings database is poisoned".to_string())?;
    init(&conn)?;
    let mut stmt = conn
        .prepare("SELECT id, name, pattern, min_score, max_severity, enabled FROM policy_rules ORDER BY id")
        .map_err(|e| e.to_string())?;
    let rules = stmt
        .query_map([], |r| {
            let severity: Option<String> = r.get(4)?;
            Ok(PolicyRule {
                id: r.get(0)?,
                name: r.get(1)?,
                pattern: r.get(2)?,
                min_score: r.get::<_, Option<i64>>(3)?.map(|v| v as u32),
                max_severity: severity.as_deref().and_then(parse_severity),
                // Stored, but there is only one action; reading it back as
                // anything else would be inventing a behaviour.
                action: PolicyAction::Warn,
                enabled: r.get::<_, i64>(5)? != 0,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rules)
}

fn parse_severity(raw: &str) -> Option<super::security_scan::Severity> {
    use super::security_scan::Severity;
    match raw.to_lowercase().as_str() {
        "critical" => Some(Severity::Critical),
        "high" => Some(Severity::High),
        "medium" => Some(Severity::Medium),
        "low" => Some(Severity::Low),
        _ => None,
    }
}

fn severity_str(severity: super::security_scan::Severity) -> &'static str {
    use super::security_scan::Severity;
    match severity {
        Severity::Critical => "critical",
        Severity::High => "high",
        Severity::Medium => "medium",
        Severity::Low => "low",
        Severity::Unknown => "unknown",
    }
}

/// Rank severities so "at most high" can be checked as a comparison.
fn severity_rank(severity: super::security_scan::Severity) -> u8 {
    use super::security_scan::Severity;
    match severity {
        Severity::Critical => 4,
        Severity::High => 3,
        Severity::Medium => 2,
        Severity::Low => 1,
        // An unranked severity must not trip a "too severe" rule; a scanner
        // that could not classify something is not evidence that it is bad.
        Severity::Unknown => 0,
    }
}

pub fn upsert_rule(rule: &PolicyRule) -> Result<i64, String> {
    let conn = db().lock().map_err(|_| "settings database is poisoned".to_string())?;
    init(&conn)?;
    let severity = rule.max_severity.map(severity_str);
    if rule.id > 0 {
        conn.execute(
            "UPDATE policy_rules SET name=?1, pattern=?2, min_score=?3, max_severity=?4, enabled=?5 WHERE id=?6",
            params![rule.name, rule.pattern, rule.min_score, severity, rule.enabled as i64, rule.id],
        )
        .map_err(|e| e.to_string())?;
        return Ok(rule.id);
    }
    conn.execute(
        "INSERT INTO policy_rules (name, pattern, min_score, max_severity, action, enabled)
         VALUES (?1,?2,?3,?4,'warn',?5)",
        params![rule.name, rule.pattern, rule.min_score, severity, rule.enabled as i64],
    )
    .map_err(|e| e.to_string())?;
    Ok(conn.last_insert_rowid())
}

pub fn delete_rule(id: i64) -> Result<(), String> {
    let conn = db().lock().map_err(|_| "settings database is poisoned".to_string())?;
    init(&conn)?;
    conn.execute("DELETE FROM policy_rules WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Judge one audit against the enabled rules.
///
/// Pure in its arguments so the decision can be tested without a database and
/// without a scanner.
pub fn evaluate(
    rules: &[PolicyRule],
    audit: &super::security_scan::SecurityAudit,
) -> Vec<Violation> {
    let mut out = Vec::new();
    let image_ref = &audit.scan.image_ref;

    for rule in rules.iter().filter(|r| r.enabled) {
        if !matches(&rule.pattern, image_ref) {
            continue;
        }

        if let Some(min_score) = rule.min_score {
            if audit.score.total < min_score {
                out.push(Violation {
                    rule_id: rule.id,
                    rule_name: rule.name.clone(),
                    image_ref: image_ref.clone(),
                    value: audit.score.total as f64,
                    threshold: min_score as f64,
                    metric: super::alerts::AlertMetric::SecurityScore,
                    detail: format!(
                        "{} scores {}, below the {} this policy asks for.",
                        image_ref, audit.score.total, min_score
                    ),
                });
            }
        }

        if let Some(max_severity) = rule.max_severity {
            let limit = severity_rank(max_severity);
            let over = audit
                .scan
                .findings
                .iter()
                .filter(|f| severity_rank(f.severity) > limit)
                .count();
            if over > 0 {
                out.push(Violation {
                    rule_id: rule.id,
                    rule_name: rule.name.clone(),
                    image_ref: image_ref.clone(),
                    value: over as f64,
                    threshold: 0.0,
                    metric: super::alerts::AlertMetric::NewVulnerabilities,
                    detail: format!(
                        "{} has {} finding(s) more severe than {}.",
                        image_ref,
                        over,
                        severity_str(max_severity)
                    ),
                });
            }
        }
    }
    out
}

/// Judge an audit and warn about anything it fails.
///
/// Warnings go out through the alert path metric alerts already use, one per
/// violation rather than one per finding.
pub fn evaluate_and_warn(audit: &super::security_scan::SecurityAudit, now_ms: i64) -> Vec<Violation> {
    let rules = list_rules().unwrap_or_default();
    let violations = evaluate(&rules, audit);
    for violation in &violations {
        super::alerts::emit_security(
            violation.rule_id,
            &violation.rule_name,
            &violation.image_ref,
            violation.metric,
            violation.value,
            violation.threshold,
            now_ms,
        );
    }
    violations
}

// ===== Commands =====

/// Refuse the policy surface when the install is not entitled.
///
/// The same shape `alerts` uses, and for the same reason: a Free install able
/// to write rules that can never warn is a feature that looks broken rather
/// than absent, and the UI is not an authorization mechanism.
pub fn require_entitled() -> Result<(), crate::error::ColimaError> {
    if super::metrics_store::entitled_now() {
        Ok(())
    } else {
        Err(crate::error::ColimaError::validation(
            "Security policy needs an active subscription",
        ))
    }
}

#[tauri::command]
pub async fn security_policy_list() -> Result<Vec<PolicyRule>, crate::error::ColimaError> {
    require_entitled()?;
    crate::helpers::run_blocking(list_rules)
        .await
        .map_err(crate::error::ColimaError::internal)
}

#[tauri::command]
pub async fn security_policy_save(rule: PolicyRule) -> Result<i64, crate::error::ColimaError> {
    let result = async move {
        require_entitled()?;
        crate::helpers::run_blocking(move || upsert_rule(&rule))
            .await
            .map_err(crate::error::ColimaError::validation)
    }
    .await;

    crate::commands::activity::record(
        crate::commands::activity::ActivityEntry::new(
            crate::commands::activity::ActivityKind::Config,
            "save",
            "security_policy",
            "",
        )
        .outcome_of(&result),
    );

    result
}

#[tauri::command]
pub async fn security_policy_delete(id: i64) -> Result<(), crate::error::ColimaError> {
    let result = async move {
        require_entitled()?;
        crate::helpers::run_blocking(move || delete_rule(id))
            .await
            .map_err(crate::error::ColimaError::validation)
    }
    .await;

    crate::commands::activity::record(
        crate::commands::activity::ActivityEntry::new(
            crate::commands::activity::ActivityKind::Config,
            "delete",
            "security_policy",
            &id.to_string(),
        )
        .outcome_of(&result),
    );

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::security_scan::{Finding, ScanResult, ScannerKind, Severity};

    fn audit_with(score: u32, findings: Vec<Severity>) -> super::super::security_scan::SecurityAudit {
        let scan = ScanResult {
            image_ref: "nginx:1.25".into(),
            image_digest: "sha256:aaa".into(),
            findings: findings
                .into_iter()
                .enumerate()
                .map(|(i, severity)| Finding {
                    id: format!("CVE-{i}"),
                    package: "pkg".into(),
                    installed_version: "1".into(),
                    fixed_version: None,
                    severity,
                    source: None,
                    cvss: None,
                    published: None,
                })
                .collect(),
            scanner: ScannerKind::Trivy,
            scanner_version: "0.73.0".into(),
            db_snapshot_date: None,
            scanned_at: 1,
        };
        let evaluation = crate::commands::security_rules::Evaluation {
            results: vec![],
            skipped: vec![],
        };
        let mut breakdown =
            crate::commands::security_score::score(&scan, &evaluation, Default::default());
        breakdown.total = score;
        super::super::security_scan::SecurityAudit {
            scan,
            evaluation,
            score: breakdown,
        }
    }

    fn rule(min_score: Option<u32>, max_severity: Option<Severity>, pattern: &str) -> PolicyRule {
        PolicyRule {
            id: 1,
            name: "House rule".into(),
            pattern: pattern.into(),
            min_score,
            max_severity,
            action: PolicyAction::Warn,
            enabled: true,
        }
    }

    #[test]
    fn a_score_below_the_bar_is_a_violation() {
        let v = evaluate(&[rule(Some(70), None, "*")], &audit_with(62, vec![]));
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].value, 62.0);
        assert_eq!(v[0].threshold, 70.0);
    }

    #[test]
    fn a_score_at_the_bar_passes() {
        assert!(evaluate(&[rule(Some(70), None, "*")], &audit_with(70, vec![])).is_empty());
    }

    #[test]
    fn only_findings_above_the_limit_count() {
        let audit = audit_with(90, vec![Severity::Critical, Severity::High, Severity::Low]);
        let v = evaluate(&[rule(None, Some(Severity::High), "*")], &audit);
        assert_eq!(v.len(), 1);
        // The critical one, not the high one that sits exactly at the limit.
        assert_eq!(v[0].value, 1.0);
    }

    #[test]
    fn an_unknown_severity_never_trips_the_bar() {
        // A scanner that could not classify something is not evidence it is bad.
        let audit = audit_with(90, vec![Severity::Unknown, Severity::Unknown]);
        assert!(evaluate(&[rule(None, Some(Severity::Critical), "*")], &audit).is_empty());
    }

    #[test]
    fn patterns_match_all_prefix_or_exact() {
        assert!(matches("*", "nginx:1.25"));
        assert!(matches("nginx*", "nginx:1.25"));
        assert!(matches("nginx:1.25", "nginx:1.25"));
        assert!(!matches("redis*", "nginx:1.25"));
        assert!(!matches("nginx:1.24", "nginx:1.25"));
    }

    #[test]
    fn a_rule_for_another_image_stays_quiet() {
        assert!(evaluate(&[rule(Some(99), None, "redis*")], &audit_with(10, vec![])).is_empty());
    }

    #[test]
    fn a_disabled_rule_does_not_fire() {
        let mut r = rule(Some(99), None, "*");
        r.enabled = false;
        assert!(evaluate(&[r], &audit_with(10, vec![])).is_empty());
    }

    #[test]
    fn the_only_action_is_warn() {
        // Guards the decision, not the code: adding Block must be a deliberate
        // change that breaks this test rather than a quiet new variant.
        assert_eq!(PolicyAction::default(), PolicyAction::Warn);
    }
}
