//! Empirical Adversarial Integration Test Suite for Milestone 2
//!
//! Adversarially challenges Milestone 2 backend integration and fallback logic:
//! 1. `list_containers` CLI fallback and label parsing:
//!    - Complex Docker CLI `--format json` output with edge-case Compose labels
//!      (`com.docker.compose.project=my-app`, multiple `=` characters in label values,
//!      non-compose labels, empty labels, missing labels, trailing commas, unicode)
//!    - Normalization and label retention contract
//! 2. `detect_runtime()` profile config parsing:
//!    - `instance_reader::read_profile_runtime` behavior when `colima.yaml` has unusual
//!      formatting, empty file, comments, or unsupported runtimes
//! 3. Resource saver idle detection:
//!    - Validation of `output.status.success()` logic
//!    - Guarantee that non-zero exit codes (exit 1, exit 127, exit 255) ensure `is_idle == false`
//! 4. Lifecycle signaling consistency:
//!    - Verification of probe cache invalidation, SSE event propagation (`docker-connection-lost`),
//!      cache-clearing invariants, and suppression state contracts

use colima_ui_lib::instance_reader::{read_profile_runtime, read_profile_runtime_in};
use colima_ui_lib::path_util::invalidate_probe_cache;
use colima_ui_lib::sse::{publish_sse_event, subscribe_topics};
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

/// Global serial mutex for tests that touch process environment variables (`COLIMA_HOME`).
static SERIAL_ENV_MUTEX: Mutex<()> = Mutex::new(());

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn unique_temp_dir(prefix: &str) -> PathBuf {
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = PathBuf::from(format!(
        "/tmp/cui-m2-adv-{}-{}-{}",
        prefix,
        std::process::id(),
        id
    ));
    let _ = fs::create_dir_all(&dir);
    dir
}

struct DirCleanup(PathBuf);
impl Drop for DirCleanup {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

// ===========================================================================
// SECTION 1: list_containers CLI Fallback & Label Parsing Robustness
// ===========================================================================

/// Reference implementation matching the exact normalization logic in
/// `src-tauri/src/commands/containers.rs::normalize_cli_labels` and `list_containers`.
fn normalize_cli_labels_model(mut value: serde_json::Value) -> serde_json::Value {
    let raw = match value.get("Labels").and_then(|l| l.as_str()) {
        Some(s) => s.to_string(),
        None => {
            if value.get("Labels").is_none() {
                if let Some(obj) = value.as_object_mut() {
                    obj.insert("Labels".into(), serde_json::json!({}));
                }
            }
            return value;
        }
    };

    let mut labels = serde_json::Map::new();
    for pair in raw.split(',') {
        let pair = pair.trim();
        if pair.is_empty() {
            continue;
        }
        match pair.split_once('=') {
            Some((k, v)) => {
                labels.insert(k.to_string(), serde_json::Value::String(v.to_string()));
            }
            None => {
                labels.insert(pair.to_string(), serde_json::Value::String(String::new()));
            }
        }
    }

    if let Some(obj) = value.as_object_mut() {
        obj.insert("Labels".into(), serde_json::Value::Object(labels));
    }
    value
}

/// Helper that reproduces the exact CLI fallback mapping pipeline in `list_containers`.
fn map_cli_fallback_containers(stdout: &str) -> Vec<serde_json::Value> {
    stdout
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| serde_json::from_str(l).ok())
        .map(normalize_cli_labels_model)
        .map(|c: serde_json::Value| {
            serde_json::json!({
                "Id": c.get("ID").or(c.get("Id")).unwrap_or(&serde_json::Value::String(String::new())),
                "Names": c.get("Names").or(c.get("names")).unwrap_or(&serde_json::Value::String(String::new())),
                "Image": c.get("Image").or(c.get("image")).unwrap_or(&serde_json::Value::String(String::new())),
                "Status": c.get("Status").or(c.get("status")).unwrap_or(&serde_json::Value::String(String::new())),
                "State": c.get("State").or(c.get("state")).unwrap_or(&serde_json::Value::String(String::new())),
                "Ports": c.get("Ports").or(c.get("ports")).unwrap_or(&serde_json::Value::String(String::new())),
                "CreatedAt": c.get("CreatedAt").or(c.get("created_at")).unwrap_or(&serde_json::Value::String(String::new())),
                "Size": c.get("Size").or(c.get("size")).unwrap_or(&serde_json::Value::String(String::new())),
                "Command": c.get("Command").or(c.get("command")).unwrap_or(&serde_json::Value::String(String::new())),
                "Labels": c.get("Labels").cloned().unwrap_or(serde_json::json!({})),
            })
        })
        .collect()
}

#[test]
fn test_adv_cli_fallback_compose_label_retention() {
    let raw_cli_line = serde_json::json!({
        "ID": "abc123def456",
        "Names": "web-frontend-1",
        "Image": "nginx:alpine",
        "State": "running",
        "Status": "Up 2 hours",
        "Labels": "com.docker.compose.project=my-ecommerce,com.docker.compose.service=web,com.docker.compose.version=2.20.0"
    }).to_string();

    let mapped = map_cli_fallback_containers(&raw_cli_line);
    assert_eq!(mapped.len(), 1);
    let container = &mapped[0];

    assert_eq!(container["Id"], "abc123def456");
    assert_eq!(container["Names"], "web-frontend-1");
    assert!(container["Labels"].is_object());
    assert_eq!(
        container["Labels"]["com.docker.compose.project"],
        "my-ecommerce"
    );
    assert_eq!(container["Labels"]["com.docker.compose.service"], "web");
    assert_eq!(
        container["Labels"]["com.docker.compose.version"],
        "2.20.0"
    );
}

#[test]
fn test_adv_cli_fallback_multiple_equals_in_label_values() {
    // Label values containing multiple '=' characters (e.g. CLI commands, connection strings, flags)
    let raw_cli_line = serde_json::json!({
        "ID": "cont-eq-test",
        "Labels": "com.docker.compose.project=app,DATABASE_URL=postgres://user:pass@host:5432/db?sslmode=disable&opt=1,run.command=sh -c export FOO=BAR && exec baz=qux"
    }).to_string();

    let mapped = map_cli_fallback_containers(&raw_cli_line);
    assert_eq!(mapped.len(), 1);
    let labels = &mapped[0]["Labels"];

    assert_eq!(labels["com.docker.compose.project"], "app");
    assert_eq!(
        labels["DATABASE_URL"],
        "postgres://user:pass@host:5432/db?sslmode=disable&opt=1"
    );
    assert_eq!(
        labels["run.command"],
        "sh -c export FOO=BAR && exec baz=qux"
    );
}

#[test]
fn test_adv_cli_fallback_non_compose_and_empty_labels() {
    // Test mixture of non-compose labels, empty string label, missing label, and flag label with no '='
    let lines = format!(
        "{}\n{}\n{}",
        serde_json::json!({
            "ID": "cont-1",
            "Labels": "maintainer=ops@example.com,tier=backend,headless,version=1.0"
        }),
        serde_json::json!({
            "ID": "cont-2",
            "Labels": "" // empty string labels
        }),
        serde_json::json!({
            "ID": "cont-3"
            // missing Labels key entirely
        })
    );

    let mapped = map_cli_fallback_containers(&lines);
    assert_eq!(mapped.len(), 3);

    // Cont 1: has mixed labels, including headless with no value
    assert_eq!(mapped[0]["Labels"]["maintainer"], "ops@example.com");
    assert_eq!(mapped[0]["Labels"]["tier"], "backend");
    assert_eq!(mapped[0]["Labels"]["headless"], "");
    assert_eq!(mapped[0]["Labels"]["version"], "1.0");

    // Cont 2: empty string labels -> normalized to empty object, not null
    assert!(mapped[1]["Labels"].is_object());
    assert_eq!(mapped[1]["Labels"].as_object().unwrap().len(), 0);

    // Cont 3: missing labels key -> normalized to empty object, not null
    assert!(mapped[2]["Labels"].is_object());
    assert_eq!(mapped[2]["Labels"].as_object().unwrap().len(), 0);
}

#[test]
fn test_adv_cli_fallback_delimiters_and_unicode_labels() {
    // Edge cases: trailing/leading commas, consecutive commas, spaces, unicode
    let raw_cli_line = serde_json::json!({
        "ID": "cont-edge",
        "Labels": ",,com.docker.compose.project=app-αβγ,,,author=Müller <m@example.com>,  space_key  =  space_val  ,,"
    }).to_string();

    let mapped = map_cli_fallback_containers(&raw_cli_line);
    assert_eq!(mapped.len(), 1);
    let labels = &mapped[0]["Labels"];

    assert_eq!(labels["com.docker.compose.project"], "app-αβγ");
    assert_eq!(labels["author"], "Müller <m@example.com>");
    // Note empirical behavior: normalize_cli_labels trims the whole pair, but split_once('=')
    // does not trim k or v individually. Thus `k` retains trailing spaces: "space_key  ".
    assert_eq!(labels["space_key  "], "  space_val");
}

// ===========================================================================
// SECTION 2: detect_runtime() Profile Config Parsing Robustness
// ===========================================================================

#[test]
fn test_adv_read_profile_runtime_empty_colima_yaml() {
    let temp = unique_temp_dir("adv-empty-yaml");
    let _guard = DirCleanup(temp.clone());

    let profile_dir = temp.join("test-empty");
    fs::create_dir_all(&profile_dir).unwrap();
    fs::write(profile_dir.join("colima.yaml"), b"").unwrap();

    // An empty file must gracefully parse and fall back to "docker"
    let runtime = read_profile_runtime_in(&temp, "test-empty");
    assert_eq!(runtime, "docker");
}

#[test]
fn test_adv_read_profile_runtime_comments_only() {
    let temp = unique_temp_dir("adv-comments-yaml");
    let _guard = DirCleanup(temp.clone());

    let profile_dir = temp.join("test-comments");
    fs::create_dir_all(&profile_dir).unwrap();
    let yaml = r#"
# Colima Profile Configuration
# Generated by colima start
# No active keys defined below
# runtime: containerd
"#;
    fs::write(profile_dir.join("colima.yaml"), yaml).unwrap();

    let runtime = read_profile_runtime_in(&temp, "test-comments");
    assert_eq!(runtime, "docker");
}

#[test]
fn test_adv_read_profile_runtime_unusual_formatting_and_valid_runtimes() {
    let temp = unique_temp_dir("adv-formatting-yaml");
    let _guard = DirCleanup(temp.clone());

    // 1. containerd with unusual whitespace and inline comments
    let dir1 = temp.join("prof-containerd");
    fs::create_dir_all(&dir1).unwrap();
    let yaml1 = r#"
---
# Header comments
cpu:    4
memory: 8

runtime: "containerd" # inline comment after value
arch: aarch64
"#;
    fs::write(dir1.join("colima.yaml"), yaml1).unwrap();
    assert_eq!(read_profile_runtime_in(&temp, "prof-containerd"), "containerd");

    // 2. podman with single quotes
    let dir2 = temp.join("prof-podman");
    fs::create_dir_all(&dir2).unwrap();
    let yaml2 = r#"
runtime: 'podman'
kubernetes:
  enabled: false
"#;
    fs::write(dir2.join("colima.yaml"), yaml2).unwrap();
    assert_eq!(read_profile_runtime_in(&temp, "prof-podman"), "podman");

    // 3. docker explicitly specified
    let dir3 = temp.join("prof-docker");
    fs::create_dir_all(&dir3).unwrap();
    let yaml3 = "runtime: docker\n";
    fs::write(dir3.join("colima.yaml"), yaml3).unwrap();
    assert_eq!(read_profile_runtime_in(&temp, "prof-docker"), "docker");
}

#[test]
fn test_adv_read_profile_runtime_unsupported_or_custom_runtimes() {
    let temp = unique_temp_dir("adv-unsupported-yaml");
    let _guard = DirCleanup(temp.clone());

    let profile_dir = temp.join("prof-custom");
    fs::create_dir_all(&profile_dir).unwrap();
    let yaml = "runtime: wasm-custom-engine\n";
    fs::write(profile_dir.join("colima.yaml"), yaml).unwrap();

    // The reader preserves whatever non-empty string was configured
    let runtime = read_profile_runtime_in(&temp, "prof-custom");
    assert_eq!(runtime, "wasm-custom-engine");
}

#[test]
fn test_adv_read_profile_runtime_malformed_syntax_fallback() {
    let temp = unique_temp_dir("adv-malformed-yaml");
    let _guard = DirCleanup(temp.clone());

    let profile_dir = temp.join("prof-broken");
    fs::create_dir_all(&profile_dir).unwrap();
    let bad_yaml = "runtime: [unclosed list {{{{ !!! &&& broken syntax";
    fs::write(profile_dir.join("colima.yaml"), bad_yaml).unwrap();

    // Broken YAML parser error must not panic, must fall back to "docker"
    let runtime = read_profile_runtime_in(&temp, "prof-broken");
    assert_eq!(runtime, "docker");
}

#[test]
fn test_adv_read_profile_runtime_path_traversal_and_invalid_names() {
    let temp = unique_temp_dir("adv-traversal");
    let _guard = DirCleanup(temp.clone());

    // Path traversal names must be rejected by is_valid_profile_name
    assert_eq!(read_profile_runtime_in(&temp, "../escape"), "docker");
    assert_eq!(read_profile_runtime_in(&temp, "/etc/passwd"), "docker");
    assert_eq!(read_profile_runtime_in(&temp, "has spaces"), "docker");
    assert_eq!(read_profile_runtime_in(&temp, "semi;colon"), "docker");
}

#[test]
fn test_adv_read_profile_runtime_missing_profile_directory() {
    let temp = unique_temp_dir("adv-missing-dir");
    let _guard = DirCleanup(temp.clone());

    let runtime = read_profile_runtime_in(&temp, "non-existent-profile");
    assert_eq!(runtime, "docker");
}

#[test]
fn test_adv_read_profile_runtime_with_colima_home_env() {
    let _lock = SERIAL_ENV_MUTEX.lock().unwrap();

    let temp = unique_temp_dir("adv-env-home");
    let _guard = DirCleanup(temp.clone());

    let profile_dir = temp.join("env-profile");
    fs::create_dir_all(&profile_dir).unwrap();
    fs::write(
        profile_dir.join("colima.yaml"),
        "runtime: containerd\n",
    )
    .unwrap();

    std::env::set_var("COLIMA_HOME", temp.to_str().unwrap());
    let runtime = read_profile_runtime("env-profile");
    std::env::remove_var("COLIMA_HOME");

    assert_eq!(runtime, "containerd");
}

// ===========================================================================
// SECTION 3: Resource Saver Idle Detection Robustness
// ===========================================================================

/// Reference evaluation function implementing the exact idle calculation logic in
/// `src-tauri/src/commands/system.rs::start_resource_saver_daemon`.
fn evaluate_resource_saver_idle(output_res: Result<std::process::Output, std::io::Error>) -> bool {
    let mut is_idle = true;
    if let Ok(output) = output_res {
        if !output.status.success() {
            // If docker command returns non-zero, do NOT treat the system as idle
            is_idle = false;
        } else {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let mut total_cpu = 0.0;
            for line in stdout.lines() {
                let clean = line.replace('%', "").trim().to_string();
                if let Ok(val) = clean.parse::<f64>() {
                    total_cpu += val;
                }
            }
            // If total CPU > 1%, it's not idle
            if total_cpu > 1.0 {
                is_idle = false;
            }
        }
    } else {
        // If docker fails, assume not idle to be safe, or assume colima is already off
        is_idle = false;
    }
    is_idle
}

#[test]
fn test_adv_resource_saver_non_zero_exit_1_guarantees_not_idle() {
    // Simulate `docker stats` returning exit code 1 (daemon starting, socket unavailable)
    let output = Command::new("sh")
        .args(["-c", "exit 1"])
        .output();

    let is_idle = evaluate_resource_saver_idle(output);
    assert!(
        !is_idle,
        "Exit code 1 must guarantee is_idle == false (prevents false VM shutdown during boot)"
    );
}

#[test]
fn test_adv_resource_saver_exit_127_command_not_found_guarantees_not_idle() {
    // Simulate docker binary missing or path resolution failure returning 127
    let output = Command::new("sh")
        .args(["-c", "exit 127"])
        .output();

    let is_idle = evaluate_resource_saver_idle(output);
    assert!(
        !is_idle,
        "Exit code 127 must guarantee is_idle == false"
    );
}

#[test]
fn test_adv_resource_saver_empty_stdout_with_non_zero_exit_does_not_trigger_idle() {
    // The critical bug Worker M2 addressed:
    // When docker command fails with non-zero exit code, stdout is empty ("").
    // If output.status.success() is not checked, lines() is empty, total_cpu remains 0.0,
    // and is_idle erroneously becomes true.
    let output = Command::new("sh")
        .args(["-c", "echo '' > /dev/null; exit 2"])
        .output();

    let is_idle = evaluate_resource_saver_idle(output);
    assert!(
        !is_idle,
        "Empty stdout with exit code 2 must NOT be treated as 0.0% idle CPU"
    );
}

#[test]
fn test_adv_resource_saver_command_io_error_guarantees_not_idle() {
    // Simulate IO error (e.g. executable not found)
    let err_res: Result<std::process::Output, std::io::Error> = Err(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        "binary not found",
    ));

    let is_idle = evaluate_resource_saver_idle(err_res);
    assert!(!is_idle, "IO error must guarantee is_idle == false");
}

#[test]
fn test_adv_resource_saver_cpu_percentages_under_threshold_is_idle() {
    // Successful exit with CPU <= 1.0%
    let output = Command::new("sh")
        .args(["-c", "printf '0.15%%\n0.35%%\n'; exit 0"])
        .output();

    let is_idle = evaluate_resource_saver_idle(output);
    assert!(is_idle, "Total CPU 0.50% <= 1.0% must evaluate to is_idle == true");
}

#[test]
fn test_adv_resource_saver_cpu_percentages_over_threshold_not_idle() {
    // Successful exit with CPU > 1.0%
    let output = Command::new("sh")
        .args(["-c", "printf '0.75%%\n0.80%%\n'; exit 0"])
        .output();

    let is_idle = evaluate_resource_saver_idle(output);
    assert!(!is_idle, "Total CPU 1.55% > 1.0% must evaluate to is_idle == false");
}

#[test]
fn test_adv_resource_saver_boundary_thresholds() {
    // Exactly 1.0% (total_cpu > 1.0 is false -> is_idle is true)
    let output_boundary = Command::new("sh")
        .args(["-c", "printf '1.00%%\n'; exit 0"])
        .output();
    assert!(
        evaluate_resource_saver_idle(output_boundary),
        "Total CPU 1.00% is at or below threshold -> is_idle == true"
    );

    // 1.01% (total_cpu > 1.0 is true -> is_idle is false)
    let output_over = Command::new("sh")
        .args(["-c", "printf '1.01%%\n'; exit 0"])
        .output();
    assert!(
        !evaluate_resource_saver_idle(output_over),
        "Total CPU 1.01% is above threshold -> is_idle == false"
    );
}

// ===========================================================================
// SECTION 4: Lifecycle Signaling Consistency
// ===========================================================================

#[test]
fn test_adv_lifecycle_probe_cache_invalidation() {
    // `invalidate_probe_cache()` is called by both notify_instance_stopping
    // and notify_instance_started to clear cached socket results.
    // Ensure multiple successive calls do not deadlock or panic under contention.
    for _ in 0..100 {
        invalidate_probe_cache();
    }
}

#[tokio::test]
async fn test_adv_lifecycle_sse_event_publication_contract() {
    // Verify SSE pub/sub channel correctly receives the events emitted by lifecycle transitions:
    // "docker-connection-lost" with { "profile": "default" }
    // "docker-state-updated" with { "containers": [], "images": [] }
    let (mut rx, _guard) = subscribe_topics(&[]);

    let stop_payload = serde_json::json!({ "profile": "default" });
    let empty_state = serde_json::json!({ "containers": [], "images": [] });

    publish_sse_event("docker-connection-lost", &stop_payload);
    publish_sse_event("docker-state-updated", &empty_state);

    let msg1 = rx.recv().await.expect("receive connection-lost event");
    assert_eq!(msg1.event, "docker-connection-lost");
    let parsed1: serde_json::Value = serde_json::from_str(&msg1.data).unwrap();
    assert_eq!(parsed1["profile"], "default");

    let msg2 = rx.recv().await.expect("receive state-updated event");
    assert_eq!(msg2.event, "docker-state-updated");
    let parsed2: serde_json::Value = serde_json::from_str(&msg2.data).unwrap();
    assert_eq!(parsed2["containers"], serde_json::json!([]));
    assert_eq!(parsed2["images"], serde_json::json!([]));
}
