//! Turning findings and rule results into a number, and — more importantly —
//! into four numbers that say *why*.
//!
//! ## Why four components and not one
//!
//! "nginx:latest scores 42" tells nobody anything. "Vulnerabilities 30/40,
//! Hardening 5/25, Provenance 0/20, Freshness 7/15" says immediately that the
//! problem is a moving tag and a root user, not CVEs. The total exists to sort a
//! list; the breakdown is the answer.
//!
//! ## Why the inputs travel with the score
//!
//! A score computed from a three-week-old vulnerability database is not the same
//! score as one computed today, and two scanners disagree by more than a factor
//! of ten on the same image (measured, 23 images, see the phase 1 bakeoff). So
//! `ScoreInputs` is part of the result rather than a tooltip: a number displayed
//! bare invites a comparison that is not valid.
//!
//! ## Purity
//!
//! `score()` reads nothing but its arguments — no clock, no cache, no daemon.
//! The time-dependent part (how old the image is) is decided in
//! `security_rules::evaluate`, which takes `now` as a parameter, so a stored
//! evaluation scores the same way forever.

use serde::{Deserialize, Serialize};

use crate::commands::security_rules::{rule_pack, Component, Evaluation, Level};
use crate::commands::security_scan::{ScanResult, Severity};

/// Points available to the vulnerability component. The other three are declared
/// by the rule pack, because their weights live in it.
const VULNERABILITY_MAX: f64 = 40.0;

/// How much each finding counts before the curve is applied.
///
/// Critical is not "ten times worse" than high in any measurable sense; these
/// are the ratios that make the ordering come out right on real images, and they
/// are stated here rather than buried so they can be argued with.
fn severity_weight(severity: Severity) -> f64 {
    match severity {
        Severity::Critical => 10.0,
        Severity::High => 3.0,
        Severity::Medium => 1.0,
        Severity::Low => 0.2,
        // An unrated finding is still a finding; treated as medium rather than
        // ignored, because "unknown" is not "harmless".
        Severity::Unknown => 1.0,
    }
}

/// Weighted findings at which the vulnerability component reaches zero.
///
/// Chosen from measurement, not taste. Scoring 11 real images off a development
/// machine produced weighted totals from **36 to 1025**
/// (`tests/security_score_against_corpus.rs` prints the column). An earlier
/// value of 300 put six of those eleven at exactly zero, which is the failure
/// this constant exists to avoid: a component that cannot order the images it
/// is most needed for.
///
/// The curve is logarithmic for the same reason — linear would flatten
/// everything with any history at all against the floor.
const VULNERABILITY_SATURATION: f64 = 1200.0;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScoreInputs {
    /// Always present. A score without these is not comparable to anything.
    pub pack_version: String,
    /// Which build evaluated the pack.
    ///
    /// A pack can name a check an older build does not implement, and that build
    /// then scores the image out of fewer points. Without this field two
    /// installs would report different totals for the same image and pack with
    /// nothing to explain the difference.
    pub engine_version: String,
    pub scanner: String,
    pub scanner_version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub db_snapshot_date: Option<String>,
    pub level: Level,
    /// Rules in the pack this build could not run. Their weight is out of the
    /// denominator, not counted as a pass.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skipped_rules: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComponentScore {
    pub earned: u32,
    pub max: u32,
    /// Rule ids that cost points here, so the UI can list them without
    /// re-deriving which rules belong to which component.
    pub failed_rules: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScoreBreakdown {
    pub vulnerabilities: ComponentScore,
    pub hardening: ComponentScore,
    pub provenance: ComponentScore,
    pub freshness: ComponentScore,
    pub total: u32,
    pub inputs: ScoreInputs,
}

fn component_max(component: Component) -> u32 {
    let key = match component {
        Component::Hardening => "hardening",
        Component::Provenance => "provenance",
        Component::Freshness => "freshness",
    };
    rule_pack().component_max.get(key).copied().unwrap_or(0)
}

/// Score one component from the rules that belong to it.
///
/// The denominator is the pack's declared maximum, not the weight of the rules
/// enabled at this level. That is what makes a stricter level able only to
/// *lower* a score: a rule that is not enabled costs nothing, and every enabled
/// failure costs its full weight.
fn score_component(component: Component, evaluation: &Evaluation) -> ComponentScore {
    let declared = component_max(component);

    // A rule this build could not run is neither earned nor lost: its weight
    // leaves the denominator. Awarding the points would flatter an image for a
    // check that never happened, and deducting them would condemn it for one.
    // Either way two builds would disagree about the same image and pack.
    let unavailable: u32 = evaluation
        .skipped
        .iter()
        .filter(|s| s.component == component)
        .map(|s| s.weight)
        .sum();
    let max = declared.saturating_sub(unavailable);

    let mut lost = 0u32;
    let mut failed_rules = Vec::new();
    for r in evaluation
        .results
        .iter()
        .filter(|r| r.component == component && !r.passed)
    {
        lost += r.weight;
        failed_rules.push(r.rule_id.clone());
    }
    ComponentScore {
        earned: max.saturating_sub(lost),
        max,
        failed_rules,
    }
}

/// Findings reduced to one number, before the curve.
///
/// Public so the calibration test can print the distribution it is calibrating
/// against — the saturation constant below was chosen from measured values, not
/// from taste, and the next person to change it needs the same numbers.
pub fn weighted_findings(scan: &ScanResult) -> f64 {
    scan.findings.iter().map(|f| severity_weight(f.severity)).sum()
}

fn score_vulnerabilities(scan: &ScanResult) -> ComponentScore {
    let weighted = weighted_findings(scan);
    let ratio = if weighted <= 0.0 {
        0.0
    } else {
        ((1.0 + weighted).ln() / (1.0 + VULNERABILITY_SATURATION).ln()).min(1.0)
    };
    let earned = (VULNERABILITY_MAX * (1.0 - ratio)).round() as u32;
    ComponentScore {
        earned,
        max: VULNERABILITY_MAX as u32,
        // Findings are not rules; they are listed by the scan itself.
        failed_rules: Vec::new(),
    }
}

/// The whole score.
///
/// Pure with respect to its arguments **and the rule pack this build carries** —
/// the pack is a hidden input, which is why `inputs.pack_version` and
/// `inputs.engine_version` travel with the result. Re-scoring a stored
/// evaluation under a different pack legitimately produces a different number;
/// what must never happen is that difference being invisible.
pub fn score(scan: &ScanResult, evaluation: &Evaluation, level: Level) -> ScoreBreakdown {
    let vulnerabilities = score_vulnerabilities(scan);
    let hardening = score_component(Component::Hardening, evaluation);
    let provenance = score_component(Component::Provenance, evaluation);
    let freshness = score_component(Component::Freshness, evaluation);

    let total = vulnerabilities.earned + hardening.earned + provenance.earned + freshness.earned;

    ScoreBreakdown {
        vulnerabilities,
        hardening,
        provenance,
        freshness,
        total,
        inputs: ScoreInputs {
            pack_version: rule_pack().pack_version.clone(),
            engine_version: env!("CARGO_PKG_VERSION").to_string(),
            scanner: format!("{:?}", scan.scanner).to_lowercase(),
            scanner_version: scan.scanner_version.clone(),
            db_snapshot_date: scan.db_snapshot_date.clone(),
            level,
            skipped_rules: evaluation.skipped.iter().map(|s| s.rule_id.clone()).collect(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::security_rules::{evaluate, Evaluation};
    use crate::commands::security_scan::{Finding, ScannerKind};

    fn scan_with(findings: Vec<(Severity, &str)>) -> ScanResult {
        ScanResult {
            image_ref: "app:1.0".into(),
            image_digest: "sha256:aaa".into(),
            findings: findings
                .into_iter()
                .map(|(severity, id)| Finding {
                    id: id.into(),
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
            db_snapshot_date: Some("2026-08-12T01:10:20Z".into()),
            scanned_at: 1,
        }
    }

    const NOW_MS: i64 = 1_786_492_800_000;

    fn clean_facts() -> crate::commands::security_rules::ImageFacts {
        crate::commands::security_rules::ImageFacts {
            image_ref: "example.test/app:1.2.3".into(),
            user: Some("app".into()),
            has_healthcheck: true,
            env: vec!["PATH=/usr/bin".into()],
            exposed_ports: vec![8080],
            labels: std::collections::HashMap::from([(
                "org.opencontainers.image.source".to_string(),
                "https://example.test".to_string(),
            )]),
            created: Some("2026-08-01T00:00:00Z".into()),
            repo_digests: vec!["example.test/app@sha256:abc".into()],
            layer_count: 6,
            history: vec!["COPY app /app".into()],
        }
    }

    fn awful_facts() -> crate::commands::security_rules::ImageFacts {
        crate::commands::security_rules::ImageFacts {
            image_ref: "app".into(),
            user: None,
            has_healthcheck: false,
            env: vec!["AWS_SECRET_ACCESS_KEY=abc123".into()],
            exposed_ports: vec![80],
            labels: std::collections::HashMap::new(),
            created: Some("2019-01-01T00:00:00Z".into()),
            repo_digests: vec![],
            layer_count: 90,
            history: vec!["ADD payload.tar /".into()],
        }
    }

    #[test]
    fn a_clean_image_with_no_findings_scores_one_hundred() {
        let evaluation = evaluate(&clean_facts(), Level::L3, NOW_MS);
        let s = score(&scan_with(vec![]), &evaluation, Level::L3);
        assert_eq!(s.total, 100);
        assert_eq!(s.vulnerabilities.earned, 40);
        assert_eq!(s.hardening.earned, 25);
        assert_eq!(s.provenance.earned, 20);
        assert_eq!(s.freshness.earned, 15);
    }

    #[test]
    fn an_image_that_fails_everything_scores_near_zero() {
        let evaluation = evaluate(&awful_facts(), Level::L3, NOW_MS);
        // Past the saturation point measured on real images, so this is the
        // floor rather than a point on the curve.
        let findings = (0..150)
            .map(|i| (Severity::Critical, Box::leak(format!("CVE-{}", i).into_boxed_str()) as &str))
            .collect();
        let s = score(&scan_with(findings), &evaluation, Level::L3);
        assert_eq!(s.hardening.earned, 0);
        assert_eq!(s.provenance.earned, 0);
        assert_eq!(s.freshness.earned, 0);
        assert!(s.total <= 5, "total was {}", s.total);
    }

    #[test]
    fn the_same_inputs_always_produce_the_same_score() {
        let evaluation = evaluate(&awful_facts(), Level::L2, NOW_MS);
        let scan = scan_with(vec![(Severity::High, "CVE-1"), (Severity::Low, "CVE-2")]);
        let first = score(&scan, &evaluation, Level::L2);
        for _ in 0..100 {
            let again = score(&scan, &evaluation, Level::L2);
            assert_eq!(first.total, again.total);
            assert_eq!(first.hardening.earned, again.hardening.earned);
        }
    }

    #[test]
    fn a_stricter_level_can_only_lower_the_score() {
        // The rules L2 and L3 add are ones this image fails, and a level that
        // enables more rules must never be able to award more points.
        let facts = awful_facts();
        let l1 = score(&scan_with(vec![]), &evaluate(&facts, Level::L1, NOW_MS), Level::L1);
        let l2 = score(&scan_with(vec![]), &evaluate(&facts, Level::L2, NOW_MS), Level::L2);
        let l3 = score(&scan_with(vec![]), &evaluate(&facts, Level::L3, NOW_MS), Level::L3);
        assert!(l2.total <= l1.total, "{} !<= {}", l2.total, l1.total);
        assert!(l3.total <= l2.total, "{} !<= {}", l3.total, l2.total);
        assert!(l3.hardening.earned < l1.hardening.earned);
    }

    #[test]
    fn the_inputs_that_make_a_score_comparable_are_always_present() {
        // Contract test: the phase 3 UI shows these next to every score, and a
        // score displayed bare invites a comparison that is not valid.
        let s = score(&scan_with(vec![]), &Evaluation::default(), Level::L1);
        assert!(!s.inputs.pack_version.is_empty());
        assert_eq!(s.inputs.scanner, "trivy");
        assert!(!s.inputs.scanner_version.is_empty());
        assert!(s.inputs.db_snapshot_date.is_some());
    }

    #[test]
    fn severity_mix_orders_images_the_way_a_human_would() {
        let evaluation = evaluate(&clean_facts(), Level::L1, NOW_MS);
        let of = |f: Vec<(Severity, &str)>| score(&scan_with(f), &evaluation, Level::L1).vulnerabilities.earned;

        let none = of(vec![]);
        let one_low = of(vec![(Severity::Low, "a")]);
        let one_high = of(vec![(Severity::High, "a")]);
        let one_critical = of(vec![(Severity::Critical, "a")]);
        let many_medium = of((0..30).map(|i| (Severity::Medium, Box::leak(format!("m{}", i).into_boxed_str()) as &str)).collect());

        assert!(none > one_low, "{} !> {}", none, one_low);
        assert!(one_low > one_high);
        assert!(one_high > one_critical);
        assert!(one_critical > many_medium, "thirty mediums should outweigh one critical");
        // And the scale still separates the bad from the very bad rather than
        // flattening everything to zero.
        assert!(many_medium > 0, "the curve must not saturate this early");
    }

    #[test]
    fn the_pack_can_award_exactly_one_hundred_points_and_no_more() {
        // If the pack's declared maxima plus the vulnerability budget do not add
        // up to 100, a perfect image cannot reach 100 and a hopeless one cannot
        // reach 0 — and every threshold anyone sets later is wrong.
        let declared: u32 = rule_pack().component_max.values().sum();
        assert_eq!(declared + VULNERABILITY_MAX as u32, 100);

        // Every component a rule belongs to must be declared, or its rules score
        // against a maximum of zero and silently stop counting.
        for rule in &rule_pack().rules {
            let key = format!("{:?}", rule.component).to_lowercase();
            assert!(
                rule_pack().component_max.contains_key(&key),
                "rule {} is in undeclared component {}",
                rule.id,
                key
            );
        }
    }

    #[test]
    fn a_rule_this_build_cannot_run_leaves_the_denominator() {
        use crate::commands::security_rules::SkippedRule;

        let mut evaluation = evaluate(&clean_facts(), Level::L1, NOW_MS);
        let full = score(&scan_with(vec![]), &evaluation, Level::L1);
        assert_eq!(full.hardening.max, 25);

        evaluation.skipped.push(SkippedRule {
            rule_id: "from-a-newer-pack".into(),
            component: Component::Hardening,
            weight: 5,
        });
        let partial = score(&scan_with(vec![]), &evaluation, Level::L1);

        // Not awarded as a pass — that would flatter the image for a check that
        // never ran — and not deducted either. The maximum shrinks instead, so
        // the number itself says something was not evaluated.
        assert_eq!(partial.hardening.max, 20);
        assert_eq!(partial.hardening.earned, 20);
        assert_eq!(partial.total, full.total - 5);
        assert!(partial.inputs.skipped_rules.contains(&"from-a-newer-pack".to_string()));
        assert!(!partial.inputs.engine_version.is_empty());
    }

    #[test]
    fn a_failed_rule_names_itself_in_its_component() {
        let evaluation = evaluate(&awful_facts(), Level::L1, NOW_MS);
        let s = score(&scan_with(vec![]), &evaluation, Level::L1);
        assert!(s.hardening.failed_rules.contains(&"runs-as-root".to_string()));
        assert!(s.provenance.failed_rules.contains(&"mutable-tag".to_string()));
        assert!(s.freshness.failed_rules.contains(&"built-over-365-days-ago".to_string()));
    }
}
