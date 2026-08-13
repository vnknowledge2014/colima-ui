//! End-to-end checks for image scanning against a real Trivy and a real runtime.
//!
//! Ignored by default: these need Trivy installed, a running container runtime,
//! and a vulnerability database already on disk (~1.2 GB). Run them
//! deliberately:
//!
//! ```text
//! cargo test --test security_scan_against_trivy -- --ignored --test-threads=1
//! ```
//!
//! What they exist for: the unit tests parse frozen fixtures, which proves the
//! parser and nothing about the command line. These prove the arguments this app
//! actually passes still mean what they meant when they were written — a
//! scanner flag that is renamed upstream fails here and nowhere else.

use colima_ui_lib::commands::security_scan::{scan_image_blocking, Severity};

/// Small, present on most development machines, and reliably clean.
const CLEAN_IMAGE: &str = "nginx:alpine";

fn have(program: &str, args: &[&str]) -> bool {
    std::process::Command::new(program)
        .args(args)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn prerequisites() -> bool {
    if !have("trivy", &["--version"]) {
        eprintln!("skipping: trivy is not installed");
        return false;
    }
    if !have("docker", &["info"]) {
        eprintln!("skipping: no container runtime");
        return false;
    }
    true
}

#[test]
#[ignore = "needs Trivy, a container runtime, and a downloaded vulnerability DB"]
fn scanning_a_real_image_produces_a_result_with_provenance() {
    if !prerequisites() {
        return;
    }

    let result = scan_image_blocking("e2e-scan-1", CLEAN_IMAGE, true).expect("scan");

    assert_eq!(result.image_ref, CLEAN_IMAGE);
    assert!(
        result.image_digest.starts_with("sha256:"),
        "the digest is the cache key; a tag would return a stale image's findings"
    );
    assert_ne!(result.scanner_version, "unknown", "version must be readable");
    assert!(
        result.db_snapshot_date.is_some(),
        "a finding list without a database date cannot be explained later"
    );
    for f in &result.findings {
        assert!(!f.id.is_empty());
        assert_ne!(
            f.severity,
            Severity::Unknown,
            "a real Trivy report should not produce unknown severities: {}",
            f.id
        );
    }
}

#[test]
#[ignore = "needs Trivy and a container runtime"]
fn a_second_scan_of_the_same_image_is_served_from_cache() {
    if !prerequisites() {
        return;
    }

    let first = scan_image_blocking("e2e-scan-2", CLEAN_IMAGE, true).expect("scan");
    let started = std::time::Instant::now();
    let second = scan_image_blocking("e2e-scan-3", CLEAN_IMAGE, false).expect("cached scan");

    assert_eq!(first.scanned_at, second.scanned_at, "same result, not a re-run");
    assert!(
        started.elapsed() < std::time::Duration::from_secs(1),
        "a cache hit must not spawn the scanner"
    );
}

#[test]
#[ignore = "needs a container runtime"]
fn an_absent_image_says_so_instead_of_pulling_it() {
    if !prerequisites() {
        return;
    }

    let err = scan_image_blocking("e2e-scan-4", "example.invalid/nope:404", false)
        .expect_err("must not pull");
    assert!(
        err.contains("not present locally"),
        "the error has to name the cause, got: {}",
        err
    );
}
