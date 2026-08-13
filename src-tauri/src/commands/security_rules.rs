//! Configuration rules for local images: what the image *is*, as opposed to what
//! is *in* it.
//!
//! Phase 1 answers "which known vulnerabilities are in this image". This answers
//! the other half — running as root, no digest to pin to, a credential baked
//! into the environment — which is where most of the fixable risk actually is.
//!
//! ## The rule pack is data, and the checks are not
//!
//! A rule pack says *which* compiled check to run and how to describe the
//! result. It cannot describe a new behaviour. That matters because packs will
//! be downloadable later: a pack containing an expression or a script, fetched
//! over the network and evaluated on a user's machine, is remote code execution
//! with a friendly name. A closed `Check` enum means the worst a hostile pack
//! can do is describe a check wrongly.
//!
//! A pack naming a check this build does not know is skipped, not fatal — an
//! older app meeting a newer pack loses one rule rather than every rule.
//!
//! ## Where the text comes from
//!
//! Every `title`, `rationale` and `remediation` in `resources/rules/v1.json` is
//! written by this project. `standardRefs` carries identifiers only, and the
//! schema has no field for external text, so there is nowhere to paste any. See
//! `NOTICE` for why that boundary exists: CIS and the OWASP Docker Top 10 are
//! both CC BY-NC-SA, and ColimaUI is a commercial product.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::LazyLock;

/// Embedded at build time, so rules work with no network and no first-run fetch.
const RULE_PACK_JSON: &str = include_str!("../../resources/rules/v1.json");

/// How strict to be. The same pack; a wider set of rules.
///
/// Higher levels only ever *add* rules, which is what makes the score at L3
/// less than or equal to the score at L1 for the same image.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Level {
    #[default]
    L1,
    L2,
    L3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Component {
    Hardening,
    Provenance,
    Freshness,
}

/// A pointer into a published standard. Deliberately has **no text field**:
/// identifiers are facts, the prose around them is somebody else's work.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StandardRef {
    pub standard: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// Section or item identifier, e.g. `4.1`.
    pub id: String,
}

/// What a rule actually inspects.
///
/// Internally tagged so a pack from the future carrying an unknown `kind`
/// deserializes into `Unknown` instead of failing the whole pack.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Check {
    RunsAsRoot,
    SecretLikeEnv,
    NoHealthcheck,
    PrivilegedPort,
    AddInsteadOfCopy,
    ExcessiveLayers { max: usize },
    NoDigestPin,
    MutableTag,
    MissingSourceLabel,
    OlderThanDays { days: i64 },
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rule {
    pub id: String,
    pub title: String,
    pub rationale: String,
    pub remediation: String,
    pub severity: crate::commands::security_scan::Severity,
    pub component: Component,
    /// Points this rule removes from its component when it fails.
    pub weight: u32,
    pub min_level: Level,
    #[serde(default)]
    pub standard_refs: Vec<StandardRef>,
    pub check: Check,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RulePack {
    pub pack_version: String,
    /// The denominator each component is scored against. Fixed by the pack so a
    /// score means the same thing at every strictness level.
    pub component_max: HashMap<String, u32>,
    pub rules: Vec<Rule>,
}

static PACK: LazyLock<RulePack> = LazyLock::new(|| {
    serde_json::from_str(RULE_PACK_JSON).expect("the embedded rule pack must parse")
});

/// The pack compiled into this build.
pub fn rule_pack() -> &'static RulePack {
    &PACK
}

/// The pack, for the UI: a result carries rule ids, and this is what turns an id
/// into something a person can read and act on.
#[tauri::command]
pub async fn security_rule_pack() -> Result<&'static RulePack, crate::error::ColimaError> {
    Ok(rule_pack())
}

// ===== Facts about an image =====

/// Everything the rules read, gathered once.
///
/// A plain struct rather than a handle to the runtime: rules are then a pure
/// function of it, which is what makes them testable without a daemon and what
/// keeps the score reproducible from a stored result.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageFacts {
    /// What the user typed, used for the tag rules.
    pub image_ref: String,
    /// `Config.User`; absent or empty means root.
    pub user: Option<String>,
    pub has_healthcheck: bool,
    /// `KEY=value` as the image declares them.
    pub env: Vec<String>,
    pub exposed_ports: Vec<u32>,
    pub labels: HashMap<String, String>,
    /// RFC 3339, from `Created`.
    pub created: Option<String>,
    pub repo_digests: Vec<String>,
    pub layer_count: usize,
    /// The `CreatedBy` line of each history entry.
    pub history: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleResult {
    pub rule_id: String,
    pub passed: bool,
    /// Copied from the rule so a caller can render or score without the pack.
    pub severity: crate::commands::security_scan::Severity,
    pub component: Component,
    pub weight: u32,
    /// What was found. Short, and never a secret's value — see `evaluate`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evidence: Option<String>,
}

/// Environment variable names that usually hold a credential.
///
/// Matched on the **name**, and the value is never reported: a rule that prints
/// the secret it found has published it into a notification list, a diagnostic
/// bundle and a screenshot.
const SECRET_NAME_HINTS: &[&str] = &[
    "SECRET", "PASSWORD", "PASSWD", "TOKEN", "APIKEY", "API_KEY", "PRIVATE_KEY",
    // Not a bare "AUTH": that also matches AUTHOR, AUTHORS and MAINTAINER_AUTHOR,
    // and a rule that fires on an author's name is a rule people learn to ignore.
    "ACCESS_KEY", "CREDENTIAL", "AUTH_TOKEN", "AUTHORIZATION",
];

/// Names that match a hint but are routinely not credentials.
///
/// Compared **whole**, not as substrings: an exception matched by substring
/// would suppress `AUTH_METHOD_SECRET`, which is the opposite of the intent.
const SECRET_NAME_EXCEPTIONS: &[&str] = &["TOKENIZERS_PARALLELISM", "AUTH_METHOD", "TOKEN_TYPE"];

fn looks_like_secret(entry: &str) -> Option<String> {
    let (name, value) = entry.split_once('=')?;
    if value.trim().is_empty() {
        // A declared-but-empty variable is a placeholder for a run-time value,
        // which is exactly the thing this rule asks people to do.
        return None;
    }
    let upper = name.to_ascii_uppercase();
    if SECRET_NAME_EXCEPTIONS.iter().any(|e| upper == *e) {
        return None;
    }
    SECRET_NAME_HINTS
        .iter()
        .any(|h| upper.contains(h))
        .then(|| name.to_string())
}

/// True when the reference names a tag that can be overwritten.
fn is_mutable_tag(image_ref: &str) -> bool {
    // A digest reference is immutable whatever the tag says.
    if image_ref.contains("@sha256:") {
        return false;
    }
    // Strip a registry host with a port before looking for the tag separator.
    let last = image_ref.rsplit('/').next().unwrap_or(image_ref);
    let Some((_, tag)) = last.split_once(':') else {
        return true; // no tag at all means `latest`
    };
    if tag.is_empty() {
        return true;
    }
    // Names that conventionally track something rather than name a release.
    const ROLLING: &[&str] = &[
        "latest", "stable", "edge", "main", "master", "dev", "devel", "nightly",
        "current", "unstable", "rolling", "beta", "alpha",
    ];
    if ROLLING.contains(&tag) {
        return true;
    }
    // A major- or minor-only version (`1`, `1.27`) is republished as patches
    // land, so it moves as surely as `latest` does.
    let parts: Vec<&str> = tag.split('.').collect();
    parts.len() <= 2 && parts.iter().all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
}

fn days_since(created: &str, now_ms: i64) -> Option<i64> {
    let created_ms = parse_rfc3339_ms(created)?;
    Some((now_ms - created_ms) / 86_400_000)
}

/// Days in a month, so an impossible date is rejected rather than folded into a
/// plausible-looking epoch.
fn days_in_month(year: i64, month: i64) -> i64 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 => 29,
        2 => 28,
        _ => 0,
    }
}

/// Offset in minutes from a trailing `Z`, `+hh:mm` or `-hhmm`.
///
/// `docker image inspect` returns `Z` today, but `image history` returns local
/// offsets, and a timestamp read as UTC when it is not is up to fourteen hours
/// wrong — enough to move an image across a 90-day boundary.
fn zone_offset_minutes(s: &str) -> Option<i64> {
    let tail = s.trim_end();
    if tail.ends_with('Z') || tail.ends_with('z') {
        return Some(0);
    }
    // The sign must come after the time, not be the year's own minus sign.
    let idx = tail.rfind(['+', '-'])?;
    if idx < 11 {
        return None;
    }
    let sign = if tail.as_bytes()[idx] == b'-' { -1 } else { 1 };
    let digits: String = tail[idx + 1..].chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.len() != 4 {
        return None;
    }
    let hours: i64 = digits[0..2].parse().ok()?;
    let minutes: i64 = digits[2..4].parse().ok()?;
    Some(sign * (hours * 60 + minutes))
}

/// Milliseconds since the epoch for an RFC 3339 timestamp.
///
/// Hand-rolled rather than pulling in a date crate for one field: the format
/// here is produced by the container runtime and is always `YYYY-MM-DDThh:mm:ss`
/// with an optional fraction and zone. Anything else returns `None`, and a rule
/// with no answer passes rather than accusing an image on a parse failure.
fn parse_rfc3339_ms(s: &str) -> Option<i64> {
    let bytes = s.as_bytes();
    if bytes.len() < 19 {
        return None;
    }
    let num = |a: usize, b: usize| s.get(a..b)?.parse::<i64>().ok();
    let (y, mo, d) = (num(0, 4)?, num(5, 7)?, num(8, 10)?);
    let (h, mi, sec) = (num(11, 13)?, num(14, 16)?, num(17, 19)?);
    if !(1..=12).contains(&mo) || d < 1 || d > days_in_month(y, mo) {
        return None;
    }
    if !(0..=23).contains(&h) || !(0..=59).contains(&mi) || !(0..=60).contains(&sec) {
        return None;
    }
    // No recognisable zone means the timestamp is not one this code should be
    // interpreting: guessing UTC is how an image lands on the wrong side of an
    // age threshold.
    let offset_minutes = zone_offset_minutes(s)?;
    // Days from the civil epoch — Howard Hinnant's days_from_civil, which is
    // exact for the whole range and needs no lookup table.
    let y_adj = if mo <= 2 { y - 1 } else { y };
    let era = if y_adj >= 0 { y_adj } else { y_adj - 399 } / 400;
    let yoe = y_adj - era * 400;
    let mp = (mo + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    let local_seconds = (days * 86_400) + h * 3600 + mi * 60 + sec;
    Some((local_seconds - offset_minutes * 60) * 1000)
}

/// Whether one history line uses `ADD` for something `COPY` would have done.
///
/// `ADD file:<hash> in /` is how a base image's root filesystem appears in every
/// history, so matching `ADD ` alone would flag every image ever built.
fn is_avoidable_add(line: &str) -> bool {
    let body = line.trim().trim_start_matches("/bin/sh -c #(nop) ").trim();
    body.starts_with("ADD ") && !body.starts_with("ADD file:")
}

/// What one evaluation produced.
///
/// `skipped` exists because a rule this build cannot run is neither a pass nor a
/// failure, and scoring it as either is a lie. The scorer removes its weight
/// from the available points instead, so the component reads `22/22` rather than
/// `25/25` — the number itself says a check did not run.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Evaluation {
    pub results: Vec<RuleResult>,
    /// Rules named by the pack whose `check` this build does not implement,
    /// with the weight each would have carried.
    pub skipped: Vec<SkippedRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkippedRule {
    pub rule_id: String,
    pub component: Component,
    pub weight: u32,
}

/// Run the pack against one image's facts.
///
/// `now_ms` is a parameter rather than a call to the clock so the result is
/// reproducible: a stored evaluation must not change meaning when re-read.
pub fn evaluate(facts: &ImageFacts, level: Level, now_ms: i64) -> Evaluation {
    let mut out = Vec::new();
    let mut skipped = Vec::new();
    for rule in &rule_pack().rules {
        if rule.min_level > level {
            continue;
        }
        let (passed, evidence) = match &rule.check {
            Check::RunsAsRoot => {
                let user = facts.user.as_deref().unwrap_or("").trim().to_string();
                let is_root = user.is_empty() || user == "root" || user.starts_with("0:") || user == "0";
                (!is_root, is_root.then(|| {
                    if user.is_empty() { "no USER instruction".to_string() } else { format!("USER {}", user) }
                }))
            }
            Check::SecretLikeEnv => {
                let names: Vec<String> = facts.env.iter().filter_map(|e| looks_like_secret(e)).collect();
                let hit = !names.is_empty();
                // Names only. The value is the thing we are warning about.
                (!hit, hit.then(|| names.join(", ")))
            }
            Check::NoHealthcheck => (facts.has_healthcheck, (!facts.has_healthcheck).then(|| "no HEALTHCHECK".to_string())),
            Check::PrivilegedPort => {
                let low: Vec<String> = facts.exposed_ports.iter().filter(|p| **p < 1024).map(|p| p.to_string()).collect();
                let hit = !low.is_empty();
                (!hit, hit.then(|| low.join(", ")))
            }
            Check::AddInsteadOfCopy => {
                let hit = facts.history.iter().any(|l| is_avoidable_add(l));
                (!hit, hit.then(|| "ADD used for a local file".to_string()))
            }
            Check::ExcessiveLayers { max } => {
                let hit = facts.layer_count > *max;
                (!hit, hit.then(|| format!("{} layers", facts.layer_count)))
            }
            Check::NoDigestPin => {
                let hit = facts.repo_digests.is_empty();
                (!hit, hit.then(|| "no repository digest".to_string()))
            }
            Check::MutableTag => {
                let hit = is_mutable_tag(&facts.image_ref);
                (!hit, hit.then(|| facts.image_ref.clone()))
            }
            Check::MissingSourceLabel => {
                let hit = !facts.labels.contains_key("org.opencontainers.image.source")
                    && !facts.labels.contains_key("org.opencontainers.image.revision");
                (!hit, hit.then(|| "no source or revision label".to_string()))
            }
            Check::OlderThanDays { days } => {
                match facts.created.as_deref().and_then(|c| days_since(c, now_ms)) {
                    // An unreadable build date is not evidence of staleness.
                    None => (true, None),
                    Some(age) => {
                        let hit = age > *days;
                        (!hit, hit.then(|| format!("built {} days ago", age)))
                    }
                }
            }
            // A check this build does not implement. Recorded rather than
            // guessed at — a rule reported as passing would be a lie, and one
            // reported as failing would be a different lie.
            Check::Unknown => {
                skipped.push(SkippedRule {
                    rule_id: rule.id.clone(),
                    component: rule.component,
                    weight: rule.weight,
                });
                continue;
            }
        };

        out.push(RuleResult {
            rule_id: rule.id.clone(),
            passed,
            severity: rule.severity,
            component: rule.component,
            weight: rule.weight,
            evidence,
        });
    }
    Evaluation { results: out, skipped }
}

// ===== Gathering facts from the runtime =====

#[derive(Debug, Deserialize)]
struct InspectConfig {
    #[serde(rename = "User")]
    user: Option<String>,
    #[serde(rename = "Healthcheck")]
    healthcheck: Option<serde_json::Value>,
    #[serde(rename = "Env")]
    env: Option<Vec<String>>,
    #[serde(rename = "ExposedPorts")]
    exposed_ports: Option<HashMap<String, serde_json::Value>>,
    #[serde(rename = "Labels")]
    labels: Option<HashMap<String, String>>,
}

#[derive(Debug, Deserialize)]
struct Inspect {
    #[serde(rename = "Config")]
    config: Option<InspectConfig>,
    #[serde(rename = "Created")]
    created: Option<String>,
    #[serde(rename = "RepoDigests")]
    repo_digests: Option<Vec<String>>,
    #[serde(rename = "RootFS")]
    root_fs: Option<RootFs>,
}

#[derive(Debug, Deserialize)]
struct RootFs {
    #[serde(rename = "Layers")]
    layers: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct HistoryLine {
    #[serde(rename = "CreatedBy")]
    created_by: Option<String>,
}

/// Parse `docker image inspect --format {{json .}}` output into facts.
pub fn facts_from_inspect(image_ref: &str, inspect_json: &str, history_json: &str) -> Result<ImageFacts, String> {
    let inspect: Inspect =
        serde_json::from_str(inspect_json).map_err(|e| format!("Cannot read image config: {}", e))?;
    let config = inspect.config.unwrap_or(InspectConfig {
        user: None,
        healthcheck: None,
        env: None,
        exposed_ports: None,
        labels: None,
    });

    let exposed_ports = config
        .exposed_ports
        .unwrap_or_default()
        .keys()
        // "80/tcp" — the protocol is not what the rule is about.
        .filter_map(|k| k.split('/').next()?.parse::<u32>().ok())
        .collect();

    // One JSON object per line. A line that does not parse is skipped: history
    // is advisory here, and losing one entry must not fail the whole evaluation.
    let history = history_json
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| serde_json::from_str::<HistoryLine>(l).ok())
        .filter_map(|h| h.created_by)
        .collect();

    Ok(ImageFacts {
        image_ref: image_ref.to_string(),
        user: config.user,
        has_healthcheck: config.healthcheck.is_some_and(|h| !h.is_null()),
        env: config.env.unwrap_or_default(),
        exposed_ports,
        labels: config.labels.unwrap_or_default(),
        created: inspect.created,
        repo_digests: inspect.repo_digests.unwrap_or_default(),
        layer_count: inspect.root_fs.and_then(|r| r.layers).map_or(0, |l| l.len()),
        history,
    })
}

/// Ask the container runtime about one image.
///
/// Blocking; call it from `helpers::run_blocking`.
pub fn collect_facts_blocking(image_ref: &str) -> Result<ImageFacts, String> {
    let mut inspect_cmd = crate::commands::runtime::get_runtime_cmd();
    inspect_cmd.args(["image", "inspect", "--format", "{{json .}}", "--", image_ref]);
    let inspect = inspect_cmd
        .output()
        .map_err(|e| crate::redact::redact_err("Cannot inspect image", e))?;
    if !inspect.status.success() {
        // "No such image" and "the daemon is not running" are different problems
        // with different fixes, and telling someone to pull an image when the
        // runtime is down sends them the wrong way.
        let stderr = String::from_utf8_lossy(&inspect.stderr);
        if stderr.to_lowercase().contains("no such image") {
            return Err(format!(
                "Image {} is not present locally. Pull it first, then scan.",
                image_ref
            ));
        }
        return Err(crate::redact::redact(stderr.trim()));
    }

    let mut history_cmd = crate::commands::runtime::get_runtime_cmd();
    history_cmd.args(["image", "history", "--no-trunc", "--format", "{{json .}}", "--", image_ref]);
    // History is best-effort: without it two rules lose their input, which is a
    // better outcome than refusing to evaluate the other ten.
    let history = history_cmd.output().map(|o| String::from_utf8_lossy(&o.stdout).to_string()).unwrap_or_default();

    facts_from_inspect(image_ref, &String::from_utf8_lossy(&inspect.stdout), &history)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts() -> ImageFacts {
        ImageFacts {
            image_ref: "example.test/app:1.2.3".into(),
            user: Some("app".into()),
            has_healthcheck: true,
            env: vec!["PATH=/usr/bin".into()],
            exposed_ports: vec![8080],
            labels: HashMap::from([(
                "org.opencontainers.image.source".to_string(),
                "https://example.test/app".to_string(),
            )]),
            created: Some("2026-08-01T00:00:00Z".into()),
            repo_digests: vec!["example.test/app@sha256:abc".into()],
            layer_count: 6,
            history: vec!["COPY app /app # buildkit".into()],
        }
    }

    /// 2026-08-12T00:00:00Z, so "days ago" in these tests is stable forever.
    const NOW_MS: i64 = 1_786_492_800_000;

    #[test]
    fn the_embedded_pack_parses_and_its_weights_add_up() {
        let pack = rule_pack();
        assert!(!pack.rules.is_empty());
        for (component, max) in &pack.component_max {
            let sum: u32 = pack
                .rules
                .iter()
                .filter(|r| format!("{:?}", r.component).to_lowercase() == *component)
                .map(|r| r.weight)
                .sum();
            // If the weights do not add up to the declared maximum, an image
            // that fails everything cannot reach zero and one that passes
            // everything cannot reach full marks.
            assert_eq!(sum, *max, "component {} weights", component);
        }
    }

    #[test]
    fn a_well_built_image_passes_everything() {
        let results = evaluate(&facts(), Level::L3, NOW_MS).results;
        let failed: Vec<&str> = results.iter().filter(|r| !r.passed).map(|r| r.rule_id.as_str()).collect();
        assert!(failed.is_empty(), "unexpected failures: {:?}", failed);
    }

    #[test]
    fn root_is_detected_however_it_is_spelled() {
        for user in [None, Some("".to_string()), Some("root".to_string()), Some("0".to_string()), Some("0:0".to_string())] {
            let f = ImageFacts { user, ..facts() };
            let r = evaluate(&f, Level::L1, NOW_MS).results;
            let root = r.iter().find(|r| r.rule_id == "runs-as-root").expect("rule");
            assert!(!root.passed, "root must be detected");
        }
    }

    #[test]
    fn a_secret_is_reported_by_name_and_never_by_value() {
        let f = ImageFacts {
            env: vec![
                "PATH=/usr/bin".into(),
                "DB_PASSWORD=hunter2".into(),
                "EMPTY_TOKEN=".into(),
            ],
            ..facts()
        };
        let r = evaluate(&f, Level::L1, NOW_MS).results;
        let secret = r.iter().find(|r| r.rule_id == "secret-like-env").expect("rule");
        assert!(!secret.passed);
        let evidence = secret.evidence.clone().unwrap_or_default();
        assert!(evidence.contains("DB_PASSWORD"));
        // Printing the value would copy the secret into every notification,
        // diagnostic bundle and screenshot that carries this result.
        assert!(!evidence.contains("hunter2"), "evidence leaked the value");
        // A declared-but-empty variable is the recommended pattern, not a fault.
        assert!(!evidence.contains("EMPTY_TOKEN"));
    }

    #[test]
    fn mutable_tags_are_recognised_but_digests_are_not_mutable() {
        // No tag, or a tag that names a position rather than a release.
        assert!(is_mutable_tag("nginx"));
        assert!(is_mutable_tag("nginx:latest"));
        assert!(is_mutable_tag("registry.test:5000/team/app"));
        assert!(is_mutable_tag("alpine:edge"));
        assert!(is_mutable_tag("app:main"));
        assert!(is_mutable_tag("app:nightly"));
        // Major- and minor-only versions are republished as patches land, so
        // they move as surely as `latest` does.
        assert!(is_mutable_tag("node:20"));
        assert!(is_mutable_tag("registry.test:5000/team/app:2.0"));

        // A full version, and any digest reference, name one image forever.
        assert!(!is_mutable_tag("nginx:1.27.4"));
        assert!(!is_mutable_tag("node:20-alpine"));
        assert!(!is_mutable_tag("registry.test:5000/team/app:2.0.1"));
        assert!(!is_mutable_tag("nginx@sha256:abc"));
        assert!(!is_mutable_tag("nginx:latest@sha256:abc"));
    }

    #[test]
    fn the_base_rootfs_layer_is_not_an_avoidable_add() {
        // Every image ever built has this line; flagging it would make the rule
        // fire on everything and mean nothing.
        assert!(!is_avoidable_add("/bin/sh -c #(nop) ADD file:abc123 in / "));
        assert!(is_avoidable_add("/bin/sh -c #(nop) ADD https://example.test/x.tar /opt/"));
        assert!(is_avoidable_add("ADD app.tar /srv"));
    }

    #[test]
    fn age_rules_stack_so_an_old_image_loses_more_than_a_stale_one() {
        let old = ImageFacts { created: Some("2020-01-01T00:00:00Z".into()), ..facts() };
        let stale = ImageFacts { created: Some("2026-01-01T00:00:00Z".into()), ..facts() };

        let failed = |f: &ImageFacts| -> usize {
            evaluate(f, Level::L1, NOW_MS).results.iter().filter(|r| !r.passed).count()
        };
        assert_eq!(failed(&old), 3, "90, 180 and 365 day rules all fail");
        assert_eq!(failed(&stale), 2, "over 180 days, under a year");
    }

    #[test]
    fn an_unreadable_build_date_does_not_accuse_the_image() {
        let f = ImageFacts { created: Some("not a date".into()), ..facts() };
        let r = evaluate(&f, Level::L1, NOW_MS).results;
        assert!(r.iter().filter(|r| r.rule_id.starts_with("built-over")).all(|r| r.passed));
    }

    #[test]
    fn a_stricter_level_only_adds_rules() {
        let l1: Vec<String> = evaluate(&facts(), Level::L1, NOW_MS).results.iter().map(|r| r.rule_id.clone()).collect();
        let l3: Vec<String> = evaluate(&facts(), Level::L3, NOW_MS).results.iter().map(|r| r.rule_id.clone()).collect();
        assert!(l3.len() > l1.len());
        for id in &l1 {
            assert!(l3.contains(id), "L3 dropped {}", id);
        }
    }

    #[test]
    fn a_pack_with_an_unknown_check_loses_that_rule_and_nothing_else() {
        let json = r#"{
            "packVersion": "9.9.9",
            "componentMax": { "hardening": 8 },
            "rules": [
                { "id": "from-the-future", "title": "t", "rationale": "r", "remediation": "m",
                  "severity": "high", "component": "hardening", "weight": 4, "minLevel": "l1",
                  "check": { "kind": "reads_your_mind", "depth": 3 } },
                { "id": "runs-as-root", "title": "t", "rationale": "r", "remediation": "m",
                  "severity": "high", "component": "hardening", "weight": 4, "minLevel": "l1",
                  "newFieldNobodyKnows": true,
                  "check": { "kind": "runs_as_root" } }
            ]
        }"#;
        let pack: RulePack = serde_json::from_str(json).expect("a future pack must still parse");
        assert_eq!(pack.rules.len(), 2);
        assert!(matches!(pack.rules[0].check, Check::Unknown));
        assert!(matches!(pack.rules[1].check, Check::RunsAsRoot));
    }

    #[test]
    fn timestamps_are_read_in_their_own_zone() {
        // `docker image inspect` returns Z; `image history` returns a local
        // offset. Reading an offset timestamp as UTC is up to fourteen hours
        // wrong, which moves an image across an age threshold.
        let utc = parse_rfc3339_ms("2026-08-12T07:00:00Z").expect("utc");
        let plus7 = parse_rfc3339_ms("2026-08-12T14:00:00+07:00").expect("+07:00");
        let minus5 = parse_rfc3339_ms("2026-08-12T02:00:00-0500").expect("-0500");
        assert_eq!(utc, plus7);
        assert_eq!(utc, minus5);

        // Fractional seconds are ignored, not fatal.
        assert_eq!(
            parse_rfc3339_ms("2026-07-15T23:57:22.253361067Z"),
            parse_rfc3339_ms("2026-07-15T23:57:22Z")
        );
        // Leap day is real; the day after it in a non-leap year is not.
        assert!(parse_rfc3339_ms("2024-02-29T00:00:00Z").is_some());
        assert!(parse_rfc3339_ms("2023-02-29T00:00:00Z").is_none());
        assert!(parse_rfc3339_ms("2026-04-31T00:00:00Z").is_none());
        // No zone at all: refused rather than assumed to be UTC.
        assert!(parse_rfc3339_ms("2026-08-12T07:00:00").is_none());
    }

    #[test]
    fn secret_names_are_matched_precisely_enough_to_be_believed() {
        assert!(looks_like_secret("DB_PASSWORD=hunter2").is_some());
        assert!(looks_like_secret("AWS_SECRET_ACCESS_KEY=abc").is_some());
        // An exception is the whole name, so a longer name containing it is
        // still reported.
        assert!(looks_like_secret("AUTH_METHOD=oauth").is_none());
        assert!(looks_like_secret("AUTH_METHOD_SECRET=abc").is_some());
        // And a rule that fires on an author's name is one people learn to
        // ignore, which costs more than the false negative.
        assert!(looks_like_secret("MAINTAINER_AUTHOR=someone").is_none());
    }

    #[test]
    fn an_unrunnable_rule_is_recorded_rather_than_assumed() {
        // The pack here is the embedded one, so nothing is skipped; the shape of
        // the answer is what this pins down. `security_score` proves the weight
        // leaves the denominator.
        let e = evaluate(&facts(), Level::L3, NOW_MS);
        assert!(e.skipped.is_empty());
        assert_eq!(e.results.len(), rule_pack().rules.len());
    }

    #[test]
    fn facts_are_read_from_real_runtime_output() {
        // Shapes taken verbatim from `docker image inspect` / `image history`.
        let inspect = r#"{
            "Created": "2026-07-15T23:57:22.253361067Z",
            "RepoDigests": ["nginx@sha256:4a73"],
            "RootFS": { "Type": "layers", "Layers": ["a","b","c"] },
            "Config": {
                "Env": ["PATH=/usr/bin", "NGINX_VERSION=1.31.3"],
                "ExposedPorts": { "80/tcp": {} },
                "Labels": { "maintainer": "someone" }
            }
        }"#;
        let history = "{\"CreatedBy\":\"RUN apk add nginx # buildkit\"}\n{\"CreatedBy\":\"ENV NJS_RELEASE=1\"}\n";
        let f = facts_from_inspect("nginx:alpine", inspect, history).expect("parse");

        assert_eq!(f.exposed_ports, vec![80]);
        assert_eq!(f.layer_count, 3);
        assert_eq!(f.history.len(), 2);
        assert!(f.user.is_none(), "no User key means root");
        assert!(!f.has_healthcheck);
    }
}
