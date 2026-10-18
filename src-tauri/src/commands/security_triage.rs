//! Ordering the security work: what to fix first, and what it is worth.
//!
//! The whole module rests on one rule — **the engine computes the number, a
//! model only writes the sentence.** `score_delta_est` is what a user makes a
//! decision on, so it is measured rather than estimated: the score is
//! recomputed with that single rule flipped to passing, and the difference is
//! the answer. A model asked for the same figure would produce a plausible one,
//! which is worse than no figure at all.
//!
//! Everything here runs offline. The AI half of this feature is an optional
//! layer on top: this module also builds the redacted payload a client can send
//! if the user has a key and asks for it, following the same
//! preview-then-confirm path `compose_diagnose` established. Without a key the
//! ordering, the deltas and the patches are all still here.

use super::security_autofix::{ADDRESSABLE_RULES, AUTO_APPLICABLE_RULES};
use super::security_rules::{rule_pack, Level};
use super::security_scan::SecurityAudit;
use serde::{Deserialize, Serialize};

/// How much work a fix is, from the product's point of view rather than the
/// engineer's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Effort {
    /// A patch exists and is safe to apply as-is.
    OneClick,
    /// A concrete diff exists, but it needs a human decision first.
    Review,
    /// No mechanical answer; the remediation text is the guidance.
    Manual,
}

/// One thing worth doing, with the reason and the payoff attached.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TriageItem {
    pub rule_id: String,
    pub title: String,
    pub severity: super::security_scan::Severity,
    pub component: super::security_rules::Component,
    pub effort: Effort,
    /// Points the total score gains if this rule alone starts passing.
    /// Computed by re-scoring, never guessed.
    pub score_delta_est: u32,
    /// Why this one first, in plain terms. Deterministic — the rule pack's own
    /// rationale, not a model's paraphrase of it.
    pub why_first: String,
    pub remediation: String,
    /// What the scan actually saw, when the rule reported it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evidence: Option<String>,
}

/// The ordered work list, plus the payload for the optional AI pass.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TriageResult {
    pub items: Vec<TriageItem>,
    /// The score as it stands, so the UI can show "62 → 80 if you do these".
    pub current_score: u32,
    /// Sum of every item's delta. Not a promise: rule interactions are
    /// independent in this engine, but the number is still an upper bound
    /// worth labelling as such.
    pub potential_score: u32,
    /// Exactly what would be sent to a model, secrets already gone. Nothing is
    /// sent from here; the client asks the user first.
    pub llm_payload_preview: String,
}

/// What one rule is worth, measured by re-scoring without it failing.
///
/// `score` is pure in its arguments, so a counterfactual is just a second call
/// with one `passed` flipped. That makes the number exact for this engine
/// rather than an approximation of it.
fn delta_for_rule(audit: &SecurityAudit, rule_id: &str, level: Level) -> u32 {
    let mut hypothetical = audit.evaluation.clone();
    let mut touched = false;
    for result in hypothetical.results.iter_mut() {
        if result.rule_id == rule_id && !result.passed {
            result.passed = true;
            touched = true;
        }
    }
    if !touched {
        return 0;
    }
    let improved = super::security_score::score(&audit.scan, &hypothetical, level);
    improved.total.saturating_sub(audit.score.total)
}

/// Order the failing rules by what fixing them is worth.
///
/// Sorted by payoff first, then by how little work it is: between two fixes
/// worth the same, the one already written as a patch should come first.
pub fn triage(audit: &SecurityAudit, level: Level) -> TriageResult {
    let pack = rule_pack();
    let mut items: Vec<TriageItem> = Vec::new();

    for result in &audit.evaluation.results {
        if result.passed {
            continue;
        }
        let Some(rule) = pack.rules.iter().find(|r| r.id == result.rule_id) else {
            // A result with no rule behind it means the pack changed under a
            // stored evaluation. Skipping is right: inventing a title for it
            // would present a guess as pack content.
            continue;
        };

        let effort = if AUTO_APPLICABLE_RULES.contains(&rule.id.as_str()) {
            Effort::OneClick
        } else if ADDRESSABLE_RULES.contains(&rule.id.as_str()) {
            Effort::Review
        } else {
            Effort::Manual
        };

        items.push(TriageItem {
            score_delta_est: delta_for_rule(audit, &rule.id, level),
            rule_id: rule.id.clone(),
            title: rule.title.clone(),
            severity: rule.severity,
            component: rule.component,
            effort,
            why_first: rule.rationale.clone(),
            remediation: rule.remediation.clone(),
            evidence: result.evidence.clone(),
        });
    }

    items.sort_by(|a, b| {
        b.score_delta_est
            .cmp(&a.score_delta_est)
            .then_with(|| effort_rank(a.effort).cmp(&effort_rank(b.effort)))
            .then_with(|| a.rule_id.cmp(&b.rule_id))
    });

    let potential_score = audit.score.total + items.iter().map(|i| i.score_delta_est).sum::<u32>();

    TriageResult {
        llm_payload_preview: build_payload(audit, &items),
        current_score: audit.score.total,
        potential_score,
        items,
    }
}

fn effort_rank(effort: Effort) -> u8 {
    match effort {
        Effort::OneClick => 0,
        Effort::Review => 1,
        Effort::Manual => 2,
    }
}

/// Build the text a client may send to a model.
///
/// It carries rule ids, titles and the engine's own numbers — never the image's
/// environment, never a finding's raw metadata. A model's job here is to write
/// the paragraph explaining the order, and it needs no secret to do that.
fn build_payload(audit: &SecurityAudit, items: &[TriageItem]) -> String {
    let mut out = format!(
        "Image `{}` scores {}/100 under rule pack {}.\n\n\
         These checks fail, already ordered by how many points each one is worth:\n\n",
        audit.scan.image_ref, audit.score.total, audit.score.inputs.pack_version
    );
    for (n, item) in items.iter().enumerate() {
        out.push_str(&format!(
            "{}. [{}] {} — worth {} points. {}\n",
            n + 1,
            item.rule_id,
            item.title,
            item.score_delta_est,
            item.why_first
        ));
    }
    out.push_str(
        "\nExplain, in a short paragraph, why this order makes sense for someone \
         shipping this image. Use only the rule ids listed above; do not introduce \
         other findings, CVE numbers or scores.",
    );
    out
}

/// Triage a stored audit.
#[tauri::command]
pub async fn security_triage(
    audit: SecurityAudit,
    level: Option<Level>,
) -> Result<TriageResult, crate::error::ColimaError> {
    let level = level.unwrap_or_default();
    Ok(triage(&audit, level))
}

/// Concrete Dockerfile patches for the rules that have one.
///
/// Takes a path and reads it here, the way `compose_autofix_propose` does. An
/// image does not know which Dockerfile built it, so the caller supplies that —
/// but the contents travel no further than this process, and the name is
/// checked first so this cannot become a general-purpose file reader for a
/// browser-mode client.
#[tauri::command]
pub async fn security_autofix_propose(
    dockerfile_path: String,
) -> Result<Vec<super::security_autofix::SecurityPatch>, crate::error::ColimaError> {
    let source = read_dockerfile(&dockerfile_path)?;
    // Every rule this module can answer is offered; a rule with no mechanical
    // answer simply produces nothing, so the caller does not have to know which
    // is which before asking.
    let rule_ids: Vec<String> = ADDRESSABLE_RULES.iter().map(|s| s.to_string()).collect();
    Ok(super::security_autofix::propose_deterministic(
        &source,
        &rule_ids,
        &dockerfile_path,
    ))
}

/// Read a Dockerfile, refusing anything that is not one by name.
pub fn read_dockerfile(path: &str) -> Result<String, crate::error::ColimaError> {
    if path.trim().is_empty() || crate::validation::contains_shell_injection(path) {
        return Err(crate::error::ColimaError::validation(
            "Invalid Dockerfile path".to_string(),
        ));
    }
    if !super::compose_autofix_apply::is_dockerfile(path) {
        return Err(crate::error::ColimaError::validation(
            "Not a Dockerfile: expected `Dockerfile`, `Containerfile`, `Dockerfile.*` or `*.dockerfile`"
                .to_string(),
        ));
    }
    std::fs::read_to_string(path)
        .map_err(|e| crate::error::ColimaError::from(format!("Cannot read Dockerfile: {}", e)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::security_rules::{evaluate, Evaluation, ImageFacts};
    use crate::commands::security_scan::{ScanResult, ScannerKind};

    const NOW_MS: i64 = 1_786_492_800_000;

    /// An image with the three problems the phase set out to order: it runs as
    /// root, it rides a mutable tag, and it uses `ADD`.
    fn bad_image_audit() -> SecurityAudit {
        let facts = ImageFacts {
            image_ref: "app:latest".into(),
            user: None,
            has_healthcheck: false,
            history: vec!["ADD ./app /srv/app".into()],
            ..Default::default()
        };
        let scan = ScanResult {
            image_ref: "app:latest".into(),
            image_digest: "sha256:aaa".into(),
            findings: vec![],
            scanner: ScannerKind::Trivy,
            scanner_version: "0.73.0".into(),
            db_snapshot_date: Some("2026-08-12T01:10:20Z".into()),
            scanned_at: 1,
        };
        let evaluation: Evaluation = evaluate(&facts, Level::default(), NOW_MS);
        let score = super::super::security_score::score(&scan, &evaluation, Level::default());
        SecurityAudit {
            scan,
            evaluation,
            score,
        }
    }

    #[test]
    fn every_failing_rule_gets_a_measured_delta() {
        let audit = bad_image_audit();
        let out = triage(&audit, Level::default());
        assert!(!out.items.is_empty(), "the fixture must fail some rules");
        assert_eq!(out.current_score, audit.score.total);

        for item in &out.items {
            // The delta is a re-score, so it can be zero only when the rule
            // carries no weight at this level — never negative, never invented.
            assert!(item.score_delta_est <= 100);
        }
        // At least one rule must be worth something, or the score is not
        // responding to the evaluation at all.
        assert!(out.items.iter().any(|i| i.score_delta_est > 0));
    }

    #[test]
    fn the_list_is_ordered_by_payoff() {
        let out = triage(&bad_image_audit(), Level::default());
        let deltas: Vec<u32> = out.items.iter().map(|i| i.score_delta_est).collect();
        let mut sorted = deltas.clone();
        sorted.sort_by(|a, b| b.cmp(a));
        assert_eq!(deltas, sorted);
    }

    #[test]
    fn a_rule_with_a_ready_patch_outranks_an_equal_one_without() {
        let out = triage(&bad_image_audit(), Level::default());
        // Among items worth the same, OneClick must come before Manual.
        for pair in out.items.windows(2) {
            if pair[0].score_delta_est == pair[1].score_delta_est {
                assert!(effort_rank(pair[0].effort) <= effort_rank(pair[1].effort));
            }
        }
    }

    #[test]
    fn potential_score_is_current_plus_every_delta() {
        let out = triage(&bad_image_audit(), Level::default());
        let sum: u32 = out.items.iter().map(|i| i.score_delta_est).sum();
        assert_eq!(out.potential_score, out.current_score + sum);
    }

    #[test]
    fn the_payload_carries_rule_ids_and_no_environment() {
        let mut audit = bad_image_audit();
        audit.scan.image_ref = "app:latest".into();
        let out = triage(&audit, Level::default());
        assert!(out.llm_payload_preview.contains("runs-as-root"));
        // The model is told not to invent ids, which is the whole guardrail.
        assert!(out.llm_payload_preview.contains("do not introduce"));
        assert!(!out.llm_payload_preview.to_lowercase().contains("password"));
    }

    #[test]
    fn a_passing_image_has_nothing_to_triage() {
        let mut audit = bad_image_audit();
        for r in audit.evaluation.results.iter_mut() {
            r.passed = true;
        }
        let out = triage(&audit, Level::default());
        assert!(out.items.is_empty());
        assert_eq!(out.potential_score, out.current_score);
    }
}
