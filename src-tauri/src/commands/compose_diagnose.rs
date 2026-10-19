//! Compose file diagnosis.
//!
//! Kept separate from `compose.rs`, which is plumbing around the docker CLI.
//! This module answers a different question: *why is this compose file broken,
//! and what do we already know about that error?*
//!
//! The pipeline deliberately spends the cheapest source of truth first:
//!
//! 1. `docker compose config` — Docker already reports syntax, schema and
//!    undefined-variable errors for free. There is no reason to reimplement it.
//! 2. Knowledge Bank lookup by normalised error signature.
//! 3. An LLM, only if the first two miss — and only after the user confirms,
//!    which is why this module returns a *preview* of the outbound payload
//!    instead of calling any model itself.

use serde::{Deserialize, Serialize};

/// Result of running `docker compose config` against a file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComposeValidation {
    pub valid: bool,
    /// Raw stderr from Docker, shown verbatim so the user sees the real error.
    pub raw_error: String,
    /// Normalised form of `raw_error`, used for Knowledge Bank matching.
    pub signature: String,
    pub category: String,
}

/// Calibrated System-1 triage metrics (`@receptron/laya` + `razorback16/openjev` math).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ComposeTriage {
    /// Expected severity score on a 1.0..=4.0 rubric (`1=low, 2=medium, 3=high, 4=critical`).
    pub severity_score: f64,
    /// Winning severity rubric label (`"none"`, `"low"`, `"medium"`, `"high"`, or `"critical"`).
    pub severity_label: String,
    /// Calibrated `noul` probability in `0.0..=1.0` that the error can be fixed without manual redesign.
    pub auto_fixable_prob: f64,
    /// Distribution confidence `1 - H(p) / ln(K)` in `0.0..=1.0`.
    pub confidence: f64,
}

/// Everything the UI needs to explain a broken compose file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComposeDiagnosis {
    pub validation: ComposeValidation,
    /// Matches from the Knowledge Bank, best first. Empty when nothing matched.
    pub kb_solutions: Vec<crate::commands::knowledge_bank::KBMatch>,
    pub kb_anti_patterns: Vec<crate::commands::knowledge_bank::KBAntiPattern>,
    /// Exactly what would be sent to an LLM if the user asks for one. Secrets
    /// are already stripped. Nothing is sent anywhere until the user confirms.
    pub llm_payload_preview: String,
    /// True when the Knowledge Bank had nothing, i.e. the LLM is worth offering.
    pub needs_llm: bool,
    /// Calibrated System-1 severity and auto-fixability triage.
    pub triage: ComposeTriage,
}

fn round4(v: f64) -> f64 {
    (v * 10_000.0).round() / 10_000.0
}

fn calibrated_softmax(logits: &[f64], temperature: f64) -> Vec<f64> {
    let t = temperature.clamp(0.5, 5.0);
    let max_scaled = logits
        .iter()
        .map(|&x| x / t)
        .fold(f64::NEG_INFINITY, f64::max);
    let exps: Vec<f64> = logits.iter().map(|&x| (x / t - max_scaled).exp()).collect();
    let sum: f64 = exps.iter().sum();
    if sum <= 0.0 || !sum.is_finite() {
        vec![1.0 / (logits.len() as f64); logits.len()]
    } else {
        exps.into_iter().map(|e| e / sum).collect()
    }
}

fn shannon_confidence(probs: &[f64]) -> f64 {
    if probs.len() <= 1 {
        return 1.0;
    }
    let mut h = 0.0;
    for &p in probs {
        if p > 1e-12 {
            h -= p * p.ln();
        }
    }
    let max_h = (probs.len() as f64).ln();
    if max_h <= 0.0 {
        0.0
    } else {
        (1.0 - h / max_h).clamp(0.0, 1.0)
    }
}

/// Compute calibrated Laya/OpenJev triage metrics for a compose validation result.
pub fn compute_triage(validation: &ComposeValidation, kb_match_count: usize) -> ComposeTriage {
    if validation.valid {
        return ComposeTriage {
            severity_score: 1.0,
            severity_label: "none".to_string(),
            auto_fixable_prob: 1.0,
            confidence: 1.0,
        };
    }

    // 4-level ordered severity rubric: [low, medium, high, critical]
    // Calibrated temperature from `laya/onnx/laya_config.json` (`score`, 3..=5 -> 1.2514)
    const SCORE_TEMP: f64 = 1.2514;
    // Calibrated temperature for `noul` binary predicate (`laya_config.json` -> 1.9834)
    const NOUL_TEMP: f64 = 1.9834;

    let kb_boost = (kb_match_count.min(3) as f64) * 0.65;
    let (sev_logits, yes_logit, no_logit): ([f64; 4], f64, f64) = match validation.category.as_str()
    {
        "schema" => ([-0.5, 2.6, 0.8, -1.2], 2.4 + kb_boost, -1.0),
        "undefined_reference" => ([-0.8, 1.8, 2.2, -0.5], 1.8 + kb_boost, -0.6),
        "yaml_syntax" => ([-1.5, 0.4, 2.8, 1.1], 1.4 + kb_boost, 0.1),
        "structure" => ([-2.0, -0.4, 2.1, 2.7], 0.9 + kb_boost, 0.5),
        "missing_file" => ([-2.2, -0.8, 1.4, 3.1], -1.2 + kb_boost, 2.0),
        _ => ([-1.0, 1.2, 1.6, 0.2], 0.2 + kb_boost, 0.8),
    };

    let probs = calibrated_softmax(&sev_logits, SCORE_TEMP);
    let expected_score: f64 = probs
        .iter()
        .enumerate()
        .map(|(i, &p)| ((i + 1) as f64) * p)
        .sum();
    let labels = ["low", "medium", "high", "critical"];
    let mut best_idx = 0usize;
    for (i, &p) in probs.iter().enumerate() {
        if p > probs[best_idx] {
            best_idx = i;
        }
    }

    let noul_probs = calibrated_softmax(&[yes_logit, no_logit], NOUL_TEMP);
    let auto_fixable_prob = noul_probs.first().copied().unwrap_or(0.5);

    ComposeTriage {
        severity_score: round4(expected_score),
        severity_label: labels[best_idx].to_string(),
        auto_fixable_prob: round4(auto_fixable_prob),
        confidence: round4(shannon_confidence(&probs)),
    }
}

/// Classify an error message into a coarse bucket, used for display and for
/// grouping benchmark results.
///
/// # Branch order is load-bearing
///
/// These are substring tests, so a broader pattern placed earlier swallows a
/// narrower one placed later. `structure` must be tested **before** `schema`:
/// Docker's `services must be a mapping` contains `must be a`, so with `schema`
/// first the `structure` arm is unreachable — which is exactly the bug this
/// ordering fixes. Adding a pattern here means checking it against every arm
/// below it, not just the one it belongs to.
fn categorize(raw: &str) -> &'static str {
    let l = raw.to_lowercase();
    if l.contains("no such file") || l.contains("stat ") && l.contains("no such") {
        "missing_file"
    } else if l.contains("yaml:")
        || l.contains("go-yaml")
        || l.contains("did not find expected")
        || l.contains("could not find expected")
        || l.contains("mapping values")
        || l.contains("scanning a simple key")
    {
        "yaml_syntax"
    } else if l.contains("variable is not set")
        || l.contains("undefined volume")
        || l.contains("undefined network")
        || l.contains("undefined secret")
    {
        "undefined_reference"
    } else if l.contains("services must be a mapping")
        // Docker 5.4.0 says "empty compose file" for a file with no services.
        // "no services" never matched it and is kept only for older releases.
        || l.contains("empty compose file")
        || l.contains("no services")
    {
        "structure"
    } else if l.contains("additional propert") // Docker says "properties" (plural)
        || l.contains("must be a")
        || l.contains("invalid type")
        || l.contains("unsupported config option")
        || l.contains("not allowed")
    {
        "schema"
    } else {
        "other"
    }
}

/// Reduce a Docker error message to a stable signature.
///
/// Two users hitting the same mistake get different absolute paths, line
/// numbers and service names. Matching the Knowledge Bank on the raw string
/// would therefore almost never hit. Stripping the variable parts is what makes
/// a stored solution reusable across machines.
pub fn error_signature(raw: &str) -> String {
    let mut s = raw.trim().to_string();

    // Absolute paths differ per machine; keep the file name only.
    s = regex_lite::Regex::new(r#"(/[^\s:'"]+)+/([^\s:'"/]+\.ya?ml)"#)
        .map(|re| re.replace_all(&s, "$2").to_string())
        .unwrap_or(s);

    // Line/column markers are position-dependent, not error-dependent.
    s = regex_lite::Regex::new(r"line \d+|column \d+|:\d+:\d+")
        .map(|re| re.replace_all(&s, "").to_string())
        .unwrap_or(s);

    // Collapse whitespace so formatting differences do not split signatures.
    s = regex_lite::Regex::new(r"\s+")
        .map(|re| re.replace_all(&s, " ").to_string())
        .unwrap_or(s);

    s.trim().to_string()
}

/// Strip secret-bearing values out of a compose file before it can leave the
/// machine.
///
/// This is schema-aware rather than pattern-based on purpose: guessing which
/// strings "look like" a password misses the ones that do not. Compose already
/// tells us where secrets live, so we blank those keys wholesale:
///
/// - `environment:` — values removed, keys kept (the key names are the useful
///   diagnostic signal, the values are what leak).
/// - `secrets:` — dropped entirely.
/// - `env_file:` — the reference is kept, the referenced file is *never* read
///   or inlined.
///
/// Redaction is unconditional. There is no setting to turn it off.
pub fn redact_compose(yaml: &str) -> String {
    let mut out = String::with_capacity(yaml.len());
    // Indentation of the key whose block we are currently blanking, if any.
    let mut redacting_block: Option<usize> = None;

    for line in yaml.lines() {
        let indent = line.len() - line.trim_start().len();
        let trimmed = line.trim_start();

        // A line at or above the block's own indentation ends the block.
        if let Some(block_indent) = redacting_block {
            if !trimmed.is_empty() && indent <= block_indent {
                redacting_block = None;
            } else {
                if !trimmed.is_empty() {
                    out.push_str(&" ".repeat(indent));
                    out.push_str("# [redacted]\n");
                }
                continue;
            }
        }

        let key = trimmed.trim_start_matches("- ").trim();

        // Inline forms: `environment: {FOO: bar}` / `secrets: [db_password]`.
        if let Some(rest) = key.strip_prefix("environment:").or_else(|| key.strip_prefix("secrets:")) {
            let label = if key.starts_with("secrets:") { "secrets" } else { "environment" };
            if rest.trim().is_empty() {
                // Block form — blank every line until the block closes.
                out.push_str(&format!("{}{}: # [redacted]\n", " ".repeat(indent), label));
                redacting_block = Some(indent);
            } else {
                out.push_str(&format!("{}{}: # [redacted]\n", " ".repeat(indent), label));
            }
            continue;
        }

        // `env_file` is kept as a reference so the diagnosis can mention it, but
        // the file it points at is never opened.
        if key.starts_with("env_file:") {
            out.push_str(&format!("{}env_file: # [not inlined]\n", " ".repeat(indent)));
            if key.strip_prefix("env_file:").is_some_and(|r| r.trim().is_empty()) {
                redacting_block = Some(indent);
            }
            continue;
        }

        out.push_str(line);
        out.push('\n');
    }

    out
}

/// Reject anything that is not plausibly a compose file.
///
/// Unlike the other compose commands, diagnosis reads the file and hands its
/// contents back to the caller. In desktop mode that is unremarkable — the user
/// already owns the filesystem. In browser/server mode it would otherwise be a
/// general-purpose file read for any authenticated client, so the path is
/// constrained to YAML files here.
pub(crate) fn ensure_compose_path(file_path: &str) -> Result<(), String> {
    if file_path.trim().is_empty() {
        return Err("No compose file specified".to_string());
    }
    if crate::validation::contains_shell_injection(file_path) {
        return Err("Invalid compose file path".to_string());
    }
    let lower = file_path.to_lowercase();
    if !(lower.ends_with(".yml") || lower.ends_with(".yaml")) {
        return Err("Compose file must be a .yml or .yaml file".to_string());
    }
    if let Ok(meta) = std::fs::symlink_metadata(file_path) {
        if meta.file_type().is_symlink() {
            return Err("Refusing to operate on a symlinked compose file".to_string());
        }
    }
    Ok(())
}

/// Build the text that would be sent to an LLM, secrets already removed.
fn build_llm_preview(file_path: &str, raw_error: &str, redacted_yaml: &str) -> String {
    format!(
        "Docker Compose file `{}` fails validation.\n\n\
         Error reported by `docker compose config`:\n```\n{}\n```\n\n\
         The file below has had all environment values, secrets and env_file \
         contents removed before being shared:\n```yaml\n{}\n```\n\n\
         Explain the cause and the fix.",
        file_path,
        raw_error.trim(),
        redacted_yaml.trim()
    )
}

/// Run `docker compose config` against a file and classify the outcome.
///
/// `--quiet` validates without dumping the resolved config, so success costs
/// nothing to parse and failure still writes the full message to stderr.
#[tauri::command]
pub async fn compose_validate(file_path: String) -> Result<ComposeValidation, crate::error::ColimaError> {
    async move {
        ensure_compose_path(&file_path)?;

        let args = [
            "compose".to_string(),
            "-f".to_string(),
            file_path.clone(),
            "config".to_string(),
            "--quiet".to_string(),
        ];

        let output = tokio::time::timeout(
            std::time::Duration::from_secs(30),
            tokio::task::spawn_blocking(move || {
                crate::commands::runtime::get_runtime_cmd()
                    .args(args.iter().map(|s| s.as_str()).collect::<Vec<&str>>())
                    .output()
                    .map_err(|e| format!("Failed to run docker compose config: {}", e))
            }),
        )
        .await
        .map_err(|_| "docker compose config timed out".to_string())?
        .map_err(|e| format!("Task join error: {}", e))??;

        if output.status.success() {
            return Ok(ComposeValidation {
                valid: true,
                raw_error: String::new(),
                signature: String::new(),
                category: "none".to_string(),
            });
        }

        let raw_error = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Ok(ComposeValidation {
            signature: error_signature(&raw_error),
            category: categorize(&raw_error).to_string(),
            valid: false,
            raw_error,
        })
    }
    .await
    .map_err(|e: String| crate::error::ColimaError::from(e))
}

/// Validate a compose file, then look the failure up in the Knowledge Bank.
///
/// Returns a redacted LLM payload alongside the result but never sends it; the
/// user decides whether the model is worth asking.
#[tauri::command]
pub async fn compose_diagnose(file_path: String) -> Result<ComposeDiagnosis, crate::error::ColimaError> {
    // Funnel: a Pro-adjacent feature was used. Consent-gated; no free strings.
    crate::telemetry::record(crate::telemetry::events::TelemetryEvent::FeatureUsed {
        feature: crate::telemetry::events::Feature::ComposeDiagnose,
    })
    .await;

    let validation = compose_validate(file_path.clone()).await?;

    if validation.valid {
        let triage = compute_triage(&validation, 0);
        return Ok(ComposeDiagnosis {
            validation,
            kb_solutions: vec![],
            kb_anti_patterns: vec![],
            llm_payload_preview: String::new(),
            needs_llm: false,
            triage,
        });
    }

    let kb = crate::commands::knowledge_bank::kb_query(validation.signature.clone()).await?;

    // Read the file only to build the redacted preview. A file that cannot be
    // read is not fatal — the Docker error alone is still worth showing.
    let redacted = std::fs::read_to_string(&file_path)
        .map_or_else(|_| "# compose file could not be read".to_string(), |raw| redact_compose(&raw));

    let needs_llm = kb.solutions.is_empty();
    let triage = compute_triage(&validation, kb.solutions.len());

    Ok(ComposeDiagnosis {
        llm_payload_preview: build_llm_preview(&file_path, &validation.raw_error, &redacted),
        kb_solutions: kb.solutions,
        kb_anti_patterns: kb.anti_patterns,
        needs_llm,
        validation,
        triage,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redaction_removes_environment_values() {
        let yaml = "\
services:
  db:
    image: postgres:16
    environment:
      POSTGRES_PASSWORD: hunter2
      POSTGRES_USER: admin
    ports:
      - \"5432:5432\"
";
        let out = redact_compose(yaml);
        assert!(!out.contains("hunter2"), "password leaked: {}", out);
        assert!(!out.contains("admin"), "username leaked: {}", out);
        // Structure the diagnosis needs must survive.
        assert!(out.contains("postgres:16"));
        assert!(out.contains("5432:5432"));
        assert!(out.contains("db:"));
    }

    #[test]
    fn redaction_removes_inline_environment_values() {
        let yaml = "services:\n  api:\n    environment: {API_TOKEN: sk-live-abc123}\n";
        let out = redact_compose(yaml);
        assert!(!out.contains("sk-live-abc123"), "inline token leaked: {}", out);
    }

    #[test]
    fn redaction_removes_secrets_block() {
        let yaml = "\
services:
  api:
    secrets:
      - db_password
      - stripe_key
    image: myapi
";
        let out = redact_compose(yaml);
        assert!(!out.contains("db_password"), "secret name leaked: {}", out);
        assert!(!out.contains("stripe_key"), "secret name leaked: {}", out);
        assert!(out.contains("myapi"));
    }

    #[test]
    fn redaction_never_inlines_env_file() {
        let yaml = "services:\n  api:\n    env_file:\n      - ./.env.production\n    image: myapi\n";
        let out = redact_compose(yaml);
        assert!(!out.contains(".env.production"), "env_file path leaked: {}", out);
        assert!(out.contains("not inlined"));
        assert!(out.contains("myapi"));
    }

    #[test]
    fn redaction_ends_block_at_dedent() {
        // The key after the environment block must survive: an over-eager
        // redactor that never closes the block would swallow the whole file.
        let yaml = "\
services:
  a:
    environment:
      SECRET: value
  b:
    image: nginx
";
        let out = redact_compose(yaml);
        assert!(!out.contains("SECRET"));
        assert!(out.contains("nginx"), "redaction ate the rest of the file: {}", out);
        assert!(out.contains("b:"));
    }

    #[test]
    fn signature_is_stable_across_paths_and_lines() {
        let a = "validating /Users/alice/proj/docker-compose.yml: services.web Additional property buld is not allowed";
        let b = "validating /home/bob/other/docker-compose.yml: services.web Additional property buld is not allowed";
        assert_eq!(error_signature(a), error_signature(b));
    }

    #[test]
    fn signature_strips_line_numbers() {
        let a = "yaml: line 12: did not find expected key";
        let b = "yaml: line 47: did not find expected key";
        assert_eq!(error_signature(a), error_signature(b));
    }

    #[test]
    fn signature_keeps_distinct_errors_distinct() {
        let a = "services.web Additional property buld is not allowed";
        let b = "yaml: did not find expected key";
        assert_ne!(error_signature(a), error_signature(b));
    }

    // The strings below are verbatim stderr from Docker Compose 5.4.0, captured
    // by scripts/compose-diagnose-benchmark.sh. Paraphrasing them is how the
    // first version of categorize() ended up matching nothing: Docker writes
    // "additional properties" (plural), not "additional property".
    #[test]
    fn categorize_real_schema_error() {
        let real = "validating /tmp/corpus/schema-typo.yml: services.web additional properties 'buld' not allowed";
        assert_eq!(categorize(real), "schema");
    }

    #[test]
    fn categorize_real_yaml_error() {
        let real = "go-yaml load error in scanner (while scanning a simple key) at L4.C3-L5.C1: could not find expected ':'";
        assert_eq!(categorize(real), "yaml_syntax");
    }

    #[test]
    fn categorize_buckets_common_errors() {
        assert_eq!(categorize("yaml: did not find expected key"), "yaml_syntax");
        assert_eq!(categorize("service \"web\" refers to undefined volume data"), "undefined_reference");
        assert_eq!(categorize("open /proj/docker-compose.yml: no such file or directory"), "missing_file");
    }

    /// `services:` written as a list. Verbatim stderr from Docker Compose 5.4.0.
    ///
    /// This shipped as `schema` because the message contains `must be a`, which
    /// the `schema` arm matched before `structure` was ever reached — making the
    /// whole `structure` bucket dead code. Ordering, not wording, was the bug.
    #[test]
    fn categorize_services_as_list_is_structure_not_schema() {
        assert_eq!(categorize("services must be a mapping"), "structure");
    }

    /// A file with no services. Docker says "empty compose file"; the original
    /// arm looked for "no services", which Docker never emits — so this fell
    /// through to "other".
    #[test]
    fn categorize_empty_compose_file_is_structure() {
        assert_eq!(categorize("empty compose file"), "structure");
    }

    /// The reordering must not steal messages that genuinely belong to `schema`.
    /// `must be a` is broad, so this pins the boundary between the two arms.
    #[test]
    fn categorize_schema_still_wins_for_non_structural_type_errors() {
        assert_eq!(categorize("services.web.ports must be a list"), "schema");
        assert_eq!(categorize("services.web.environment must be a mapping"), "schema");
        assert_eq!(
            categorize("validating compose.yml: services.web additional properties 'imge' not allowed"),
            "schema"
        );
    }

    #[test]
    fn path_guard_rejects_non_yaml() {
        assert!(ensure_compose_path("/etc/passwd").is_err());
        assert!(ensure_compose_path("~/.ssh/id_rsa").is_err());
        assert!(ensure_compose_path("").is_err());
        assert!(ensure_compose_path("/proj/docker-compose.yml").is_ok());
        assert!(ensure_compose_path("/proj/compose.yaml").is_ok());
    }

    #[test]
    fn llm_preview_contains_no_secrets() {
        let yaml = "services:\n  db:\n    environment:\n      POSTGRES_PASSWORD: hunter2\n";
        let preview = build_llm_preview("docker-compose.yml", "some error", &redact_compose(yaml));
        assert!(!preview.contains("hunter2"));
        assert!(preview.contains("some error"));
    }

    #[test]
    fn triage_calibrates_severity_and_auto_fixability() {
        let schema_val = ComposeValidation {
            valid: false,
            raw_error: "services.web additional properties 'buld' not allowed".into(),
            signature: "services.web additional properties 'buld' not allowed".into(),
            category: "schema".into(),
        };
        let schema_triage = compute_triage(&schema_val, 2);
        assert_eq!(schema_triage.severity_label, "medium");
        assert!(schema_triage.auto_fixable_prob > 0.85);
        assert!(schema_triage.confidence > 0.25);

        let missing_val = ComposeValidation {
            valid: false,
            raw_error: "open compose.yml: no such file or directory".into(),
            signature: "open compose.yml: no such file or directory".into(),
            category: "missing_file".into(),
        };
        let missing_triage = compute_triage(&missing_val, 0);
        assert_eq!(missing_triage.severity_label, "critical");
        assert!(missing_triage.severity_score > schema_triage.severity_score);
        assert!(missing_triage.auto_fixable_prob < 0.25);
    }
}
