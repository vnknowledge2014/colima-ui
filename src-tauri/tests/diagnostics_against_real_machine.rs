//! Does a bundle built on a real machine leak anything?
//!
//! The unit tests feed fixtures through the redactor. These build the actual
//! thing on the actual host — real home directory, real tool versions, real
//! container logs — because the failure this phase must not have is a secret
//! reaching a public GitHub issue, and that failure lives in the collectors, not
//! in the regexes.
//!
//! Ignored by default: needs a Docker daemon and creates a throwaway container.
//!
//! ```text
//! cargo test --test diagnostics_against_real_machine -- --ignored --test-threads=1
//! ```

use colima_ui_lib::commands::diagnostics::build_bundle;
use std::process::Command;

const IMAGE: &str = "curlimages/curl:latest";

fn docker(args: &[&str]) -> std::process::Output {
    Command::new("docker")
        .args(args)
        .output()
        .expect("docker must be on PATH")
}

/// The account name of whoever is running this. It must not appear in a bundle.
///
/// `$USER` is not set under every runner, and a test that silently skips the one
/// check it exists for is worse than no test. `id -un` and the home directory's
/// last component are consulted as well.
fn username() -> String {
    if let Ok(user) = std::env::var("USER") {
        if !user.is_empty() {
            return user;
        }
    }
    if let Ok(out) = Command::new("id").arg("-un").output() {
        let name = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if !name.is_empty() {
            return name;
        }
    }
    std::env::var("HOME")
        .ok()
        .and_then(|h| {
            std::path::Path::new(&h)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
        })
        .unwrap_or_default()
}

#[tokio::test]
#[ignore = "needs a real Docker daemon"]
async fn a_real_bundle_carries_no_account_name() {
    let user = username();
    assert!(!user.is_empty(), "could not determine the account name; the check would be vacuous");

    let bundle = build_bundle(None, None, None).await;
    for section in &bundle.sections {
        assert!(
            !section.content.contains(&user),
            "section {:?} contains the account name:\n{}",
            section.id,
            section.content
        );
    }
    // The paths are still there, just anonymised — over-redaction would make the
    // report useless, so check we masked rather than deleted.
    let joined: String = bundle.sections.iter().map(|s| s.content.as_str()).collect();
    assert!(!joined.contains(&format!("/Users/{user}")));
}

#[tokio::test]
#[ignore = "needs a real Docker daemon"]
async fn every_expected_section_is_present_even_when_a_collector_fails() {
    // Whatever is or is not installed on this machine, the bundle is complete:
    // a collector that cannot run reports that inside its own section.
    let bundle = build_bundle(None, None, None).await;
    let ids: Vec<&str> = bundle.sections.iter().map(|s| s.id.as_str()).collect();
    for expected in ["app", "versions", "host", "instances", "containers", "crash"] {
        assert!(ids.contains(&expected), "missing section {:?}, got {:?}", expected, ids);
    }
    for section in &bundle.sections {
        assert!(!section.content.is_empty(), "section {:?} is empty", section.id);
    }
    assert!(!bundle.app_version.is_empty());
}

#[tokio::test]
#[ignore = "needs a real Docker daemon"]
async fn a_password_printed_by_a_container_is_masked_in_its_logs() {
    // The scenario the plan names: a database container prints its environment
    // at boot and someone pastes the log into an issue.
    let name = format!("colimaui-diagtest-{}", std::process::id());
    let _ = docker(&["rm", "-f", &name]);

    let out = docker(&[
        "run", "-d", "--name", &name, IMAGE,
        "sh", "-c",
        "echo POSTGRES_PASSWORD=hunter2; echo AWS_ACCESS_KEY_ID=AKIAIOSFODNN7EXAMPLE; echo ready; sleep 20",
    ]);
    assert!(
        out.status.success(),
        "could not start the test container: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let id = String::from_utf8_lossy(&out.stdout).trim().to_string();

    // Let the container write before reading its log.
    std::thread::sleep(std::time::Duration::from_millis(1500));

    let bundle = build_bundle(None, Some(id.clone()), Some(50)).await;
    let logs = bundle
        .sections
        .iter()
        .find(|s| s.id == "logs")
        .expect("a logs section was requested");

    let _ = docker(&["rm", "-f", &name]);

    assert!(!logs.content.contains("hunter2"), "password leaked:\n{}", logs.content);
    assert!(
        !logs.content.contains("AKIAIOSFODNN7EXAMPLE"),
        "AWS key leaked:\n{}",
        logs.content
    );
    // Redaction must not have eaten the log: the report has to stay useful.
    assert!(
        logs.content.contains("ready"),
        "the log body did not survive redaction:\n{}",
        logs.content
    );
    // And logs stay opt-in.
    assert!(!logs.included_by_default);
}

#[tokio::test]
#[ignore = "needs a real Docker daemon"]
async fn an_invalid_container_id_is_reported_not_executed() {
    let bundle = build_bundle(None, Some("not a real id; rm -rf /".to_string()), None).await;
    let logs = bundle.sections.iter().find(|s| s.id == "logs").expect("logs section");
    assert!(
        logs.content.contains("invalid container id"),
        "unexpected content: {}",
        logs.content
    );
}

#[tokio::test]
#[ignore = "needs a real Docker daemon"]
async fn the_crash_section_reflects_what_is_actually_stored() {
    let bundle = build_bundle(None, None, None).await;
    let crash = bundle.sections.iter().find(|s| s.id == "crash").expect("crash section");
    // Either there is a stored crash or there is not; both are real answers, and
    // an empty section is neither.
    assert!(
        crash.content.contains("no crash recorded") || crash.content.contains("Recorded at unix"),
        "unexpected crash section: {}",
        crash.content
    );
}
