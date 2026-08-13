//! Does a real crash loop actually reach us as separate events?
//!
//! Ignored by default: needs a working Docker daemon and creates a throwaway
//! container that deliberately fails several times. Run it deliberately:
//!
//! ```text
//! cargo test --test docker_events_against_docker -- --ignored --test-threads=1
//! ```
//!
//! The unit tests in `docker_events` prove the *conversion* keeps each event.
//! They cannot prove the premise underneath it — that Docker emits one `die` per
//! restart rather than coalescing them, and that a restart loop is therefore
//! observable at all. That premise is the reason this module exists, so it is
//! checked against the daemon rather than assumed.

use colima_ui_lib::docker_events::from_bollard;
use std::process::Command;
use std::time::Duration;

const CONTAINER: &str = "colimaui-crashloop-probe";

/// `--restart on-failure:N` means "retry up to N times", so the container dies
/// **N+1** times: once on its own, then once per retry. Measured against Docker
/// 29.5.2 rather than assumed — the obvious reading of the flag is off by one,
/// and a test asserting 5 would pass while the daemon emitted 6.
const RETRIES: usize = 5;
const EXPECTED_DIES: usize = RETRIES + 1;

fn docker(args: &[&str]) -> std::process::Output {
    Command::new("docker")
        .args(args)
        .output()
        .expect("docker CLI is on PATH")
}

/// Removes the probe container however the test ends.
///
/// Cleanup placed after an assertion never runs when that assertion fails, which
/// leaves a restarting container behind on exactly the runs where someone is
/// about to investigate.
struct Probe;

impl Drop for Probe {
    fn drop(&mut self) {
        let _ = docker(&["rm", "-f", CONTAINER]);
    }
}

/// Connect the same way the app does, so the test exercises the real socket
/// discovery rather than a convenient shortcut.
fn connect() -> bollard::Docker {
    let (host, _) = colima_ui_lib::path_util::detect_docker_host()
        .expect("a running colima instance is required");
    bollard::Docker::connect_with_local(
        host.trim_start_matches("unix://"),
        120,
        bollard::API_DEFAULT_VERSION,
    )
    .expect("docker socket is reachable")
}

#[tokio::test]
#[ignore = "requires a running Docker daemon"]
async fn a_crash_loop_arrives_as_one_event_per_restart() {
    use futures_util::StreamExt;

    let _probe = Probe; // removes the container on every exit path, panic included
    drop(docker(&["rm", "-f", CONTAINER])); // clear a leftover from a previous run

    let docker_client = connect();
    let mut stream = docker_client.events(Some(bollard::system::EventsOptions::<String>::default()));

    // Fails immediately, so Docker restarts it as fast as it is willing to.
    let started = docker(&[
        "run",
        "-d",
        "--name",
        CONTAINER,
        "--restart",
        &format!("on-failure:{RETRIES}"),
        "busybox",
        "sh",
        "-c",
        "exit 1",
    ]);
    assert!(
        started.status.success(),
        "could not start probe container: {}",
        String::from_utf8_lossy(&started.stderr)
    );

    // Docker backs off between restarts (100ms, 200ms, 400ms…), so five of them
    // take a few seconds. Generous ceiling; the assertion is on the count.
    let mut dies = 0usize;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);

    while dies < EXPECTED_DIES && tokio::time::Instant::now() < deadline {
        let next = tokio::time::timeout_at(deadline, stream.next()).await;
        let Ok(Some(Ok(raw))) = next else { break };

        let Some(event) = from_bollard(&raw) else {
            continue;
        };
        if event.container_name == CONTAINER && event.action == "die" {
            dies += 1;
            // The exit code is what separates "finished" from "was killed", and
            // a healing rule cannot decide anything without it.
            assert_eq!(
                event.exit_code,
                Some(1),
                "die event {dies} lost its exit code"
            );
        }
    }

    assert_eq!(
        dies, EXPECTED_DIES,
        "a crash loop must be countable: expected {EXPECTED_DIES} separate die events, saw {dies}. \
         If this is 1, the events are being coalesced somewhere and no crash-loop rule can work."
    );
}
