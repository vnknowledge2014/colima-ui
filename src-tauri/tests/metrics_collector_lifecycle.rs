//! Does the sampling loop actually stop when nobody is watching?
//!
//! The phase's headline requirement is that an idle app makes no Docker calls.
//! That is a claim about behaviour over time, so it is asserted on the collector's
//! own tick counter rather than inferred from a CPU graph.
//!
//! Ignored by default: it spends several seconds waiting, and touches the real
//! container runtime when a subscriber exists. Run it deliberately:
//!
//! ```text
//! cargo test --test metrics_collector_lifecycle -- --ignored --test-threads=1
//! ```

use colima_ui_lib::commands::metrics_collector::{
    parse_stats, sample_now, set_interval_ms, spawn_collector, ticks_taken, TOPIC,
};
use colima_ui_lib::sse::{subscribe_topics, subscriber_count};
use std::time::Duration;

async fn settle(ms: u64) {
    tokio::time::sleep(Duration::from_millis(ms)).await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "takes several seconds and touches the container runtime"]
async fn sampling_follows_the_subscriber_count() {
    set_interval_ms(1000);
    spawn_collector();

    // 1. Idle. Nothing is watching, so nothing should be sampled.
    assert_eq!(subscriber_count(TOPIC), 0, "no watchers expected at start");
    let idle_before = ticks_taken();
    settle(2500).await;
    assert_eq!(
        ticks_taken(),
        idle_before,
        "the collector sampled while nobody was subscribed"
    );

    // 2. A watcher appears — exactly what opening the Activity page does.
    let (_rx, guard) = subscribe_topics(&[TOPIC.to_string()]);
    assert_eq!(subscriber_count(TOPIC), 1);
    settle(2500).await;
    let while_watching = ticks_taken();
    assert!(
        while_watching > idle_before,
        "the collector did not start when a watcher subscribed"
    );

    // 3. The watcher goes away, as it does when the page is closed, the tab is
    //    reloaded, or the client crashes. The guard's Drop is the only signal.
    drop(guard);
    assert_eq!(subscriber_count(TOPIC), 0);
    // Allow an in-flight sample to finish before taking the baseline.
    settle(1500).await;
    let after_close = ticks_taken();
    settle(2500).await;
    assert_eq!(
        ticks_taken(),
        after_close,
        "the collector kept sampling after the last watcher left"
    );
}

/// The figures must agree with what `docker stats` itself reports.
#[tokio::test]
#[ignore = "needs a real Docker daemon"]
async fn parsed_values_agree_with_the_docker_cli() {
    let out = std::process::Command::new("docker")
        .args(["stats", "--no-stream", "--format", "json"])
        .output()
        .expect("docker must be on PATH");
    assert!(out.status.success(), "docker stats failed");
    let raw = String::from_utf8_lossy(&out.stdout);

    let samples = parse_stats(&raw, 0, "test");
    let lines = raw.lines().filter(|l| !l.trim().is_empty()).count();
    assert_eq!(
        samples.len(),
        lines,
        "every line docker printed should become a sample"
    );

    for s in &samples {
        assert!(!s.container_id.is_empty());
        assert!(!s.name.is_empty(), "{} has no name", s.container_id);
        assert!(s.cpu_pct >= 0.0, "{} has negative CPU", s.name);
        assert!(
            s.mem_limit_bytes == 0 || s.mem_bytes <= s.mem_limit_bytes,
            "{} reports {} bytes used of a {} byte limit",
            s.name,
            s.mem_bytes,
            s.mem_limit_bytes
        );
        // The percentage docker printed and the one implied by the byte figures
        // must agree; a unit-parsing slip shows up here as a large divergence.
        if s.mem_limit_bytes > 0 && s.mem_pct > 0.0 {
            let implied = (s.mem_bytes as f64 / s.mem_limit_bytes as f64) * 100.0;
            assert!(
                (implied - s.mem_pct).abs() < 1.0,
                "{}: docker said {:.2}% but the parsed bytes imply {:.2}%",
                s.name,
                s.mem_pct,
                implied
            );
        }
    }
}

/// The sampler now reads the engine API instead of shelling out to `docker
/// stats`. That is a change of data source, so the figures have to be shown to
/// still agree with the CLI — a units or formula slip would otherwise be
/// invisible until someone compared two screens.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs a real Docker daemon"]
async fn api_figures_agree_with_the_docker_cli() {
    // Two readings: CPU percent is a delta, so the first sample of a container
    // has nothing to difference against.
    let _ = sample_now().await.expect("first sample");
    tokio::time::sleep(Duration::from_secs(2)).await;
    let samples = sample_now().await.expect("second sample");
    assert!(!samples.is_empty(), "no running containers to compare against");

    let out = std::process::Command::new("docker")
        .args(["stats", "--no-stream", "--format", "json"])
        .output()
        .expect("docker must be on PATH");
    assert!(out.status.success(), "docker stats failed");
    let cli = parse_stats(&String::from_utf8_lossy(&out.stdout), 0, "cli");

    let mut compared = 0;
    for ours in &samples {
        let Some(theirs) = cli.iter().find(|c| c.container_id == ours.container_id) else {
            // Containers start and stop between the two calls; that is not a
            // disagreement.
            continue;
        };
        compared += 1;

        assert_eq!(ours.name, theirs.name, "name mismatch for {}", ours.container_id);
        assert_eq!(ours.pids, theirs.pids, "PID count differs for {}", ours.name);

        // Memory is a gauge read at two nearby instants, so allow drift but not a
        // unit error: 5% apart is noise, 2.4% or 1024x apart is a bug.
        let close = |a: u64, b: u64, tolerance: f64, what: &str| {
            let (a, b) = (a as f64, b as f64);
            let bound = b * tolerance + 8.0 * 1024.0 * 1024.0;
            assert!(
                (a - b).abs() <= bound,
                "{} for {}: api {:.0} vs cli {:.0}",
                what,
                ours.name,
                a,
                b
            );
        };
        close(ours.mem_bytes, theirs.mem_bytes, 0.10, "memory used");
        close(ours.mem_limit_bytes, theirs.mem_limit_bytes, 0.01, "memory limit");

        // Counters only ever grow, and the CLI read after us.
        assert!(
            ours.net_rx_bytes <= theirs.net_rx_bytes + 1_000_000,
            "network rx for {}: api {} vs cli {}",
            ours.name,
            ours.net_rx_bytes,
            theirs.net_rx_bytes
        );
        assert!(ours.cpu_pct >= 0.0 && ours.cpu_pct.is_finite());
    }

    assert!(compared > 0, "no container appeared in both readings");
    println!("compared {compared} containers against the CLI");
}
