//! What does live monitoring actually cost?
//!
//! The phase sets a number — under 2% average CPU with 20 containers at a 2 s
//! period — and a number is only closed by measuring it.
//!
//! **Children count.** `all_container_stats` spawns `docker stats`, whose CPU
//! lands in a child process. `ps` on our own pid would therefore report a
//! flattering fraction of the true cost. `getrusage(RUSAGE_CHILDREN)` includes
//! reaped children, so self + children is the honest figure for "what the app
//! costs while this page is open".
//!
//! Ignored by default: it starts 20 throwaway containers and runs for a minute.
//!
//! ```text
//! cargo test --release --test metrics_collector_cost -- --ignored --nocapture
//! ```
//!
//! `--release` matters: a debug build measures the debug build, and nobody ships
//! that.

use colima_ui_lib::commands::metrics_collector::{set_interval_ms, spawn_collector, ticks_taken, TOPIC};
use colima_ui_lib::sse::subscribe_topics;
use std::process::Command;
use std::time::{Duration, Instant};

const IMAGE: &str = "curlimages/curl:latest";
const CONTAINERS: usize = 20;
const SAMPLE_SECONDS: u64 = 60;
const INTERVAL_MS: u64 = 2000;
const BUDGET_PERCENT: f64 = 2.0;

fn docker(args: &[&str]) -> std::process::Output {
    Command::new("docker").args(args).output().expect("docker must be on PATH")
}

/// Seconds of CPU burned by this process and every child it has reaped.
fn cpu_seconds() -> f64 {
    // SAFETY: `rusage` is a plain data struct fully written by getrusage on
    // success; both calls are checked.
    unsafe {
        let mut total = 0.0;
        for who in [libc::RUSAGE_SELF, libc::RUSAGE_CHILDREN] {
            let mut usage: libc::rusage = std::mem::zeroed();
            assert_eq!(libc::getrusage(who, &mut usage), 0, "getrusage failed");
            total += usage.ru_utime.tv_sec as f64
                + usage.ru_utime.tv_usec as f64 / 1e6
                + usage.ru_stime.tv_sec as f64
                + usage.ru_stime.tv_usec as f64 / 1e6;
        }
        total
    }
}

struct Fleet(Vec<String>);

impl Fleet {
    fn start(count: usize) -> Self {
        let mut names = Vec::new();
        for i in 0..count {
            let name = format!("colimaui-cost-{}-{}", std::process::id(), i);
            let out = docker(&["run", "-d", "--name", &name, IMAGE, "sleep", "600"]);
            if out.status.success() {
                names.push(name);
            } else {
                eprintln!(
                    "could not start {}: {}",
                    name,
                    String::from_utf8_lossy(&out.stderr)
                );
            }
        }
        Self(names)
    }
}

impl Drop for Fleet {
    fn drop(&mut self) {
        for name in &self.0 {
            let _ = docker(&["rm", "-f", name]);
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "starts 20 containers and runs for a minute"]
async fn sampling_twenty_containers_stays_within_the_cpu_budget() {
    let fleet = Fleet::start(CONTAINERS);
    assert!(
        fleet.0.len() >= CONTAINERS,
        "only started {} of {} containers",
        fleet.0.len(),
        CONTAINERS
    );

    let running = String::from_utf8_lossy(&docker(&["ps", "-q"]).stdout)
        .lines()
        .count();
    println!("containers running on this machine: {running}");

    set_interval_ms(INTERVAL_MS);
    spawn_collector();

    // Idle baseline first: whatever the harness costs while nothing is sampling
    // is not attributable to monitoring.
    tokio::time::sleep(Duration::from_secs(3)).await;
    let idle_start_cpu = cpu_seconds();
    let idle_start = Instant::now();
    tokio::time::sleep(Duration::from_secs(5)).await;
    let idle_percent =
        (cpu_seconds() - idle_start_cpu) / idle_start.elapsed().as_secs_f64() * 100.0;

    // Now with a watcher, which is what opening the Activity page does.
    let (_rx, guard) = subscribe_topics(&[TOPIC.to_string()]);
    let ticks_before = ticks_taken();
    let cpu_before = cpu_seconds();
    let started = Instant::now();
    tokio::time::sleep(Duration::from_secs(SAMPLE_SECONDS)).await;
    let elapsed = started.elapsed().as_secs_f64();
    let cpu_used = cpu_seconds() - cpu_before;
    let ticks = ticks_taken() - ticks_before;
    drop(guard);

    let percent = cpu_used / elapsed * 100.0;
    let per_tick_ms = if ticks > 0 { cpu_used / ticks as f64 * 1000.0 } else { 0.0 };

    println!("--- live metrics cost ---");
    println!("containers sampled : {CONTAINERS} (of {running} running)");
    println!("period             : {INTERVAL_MS} ms");
    println!("ticks              : {ticks} over {elapsed:.1}s");
    println!("idle baseline      : {idle_percent:.2}% CPU");
    println!("while sampling     : {percent:.2}% CPU (self + children)");
    println!("cost per sample    : {per_tick_ms:.1} ms CPU");
    println!("budget             : {BUDGET_PERCENT:.1}%");

    assert!(ticks > 0, "the collector never sampled");
    // Roughly the expected number of ticks: a wildly low count would mean the
    // loop stalled and the CPU figure describes nothing.
    let expected = SAMPLE_SECONDS * 1000 / INTERVAL_MS;
    assert!(
        ticks >= expected / 2,
        "expected about {expected} ticks, got {ticks}"
    );

    assert!(
        percent < BUDGET_PERCENT,
        "live monitoring costs {percent:.2}% CPU, over the {BUDGET_PERCENT:.1}% budget \
         ({per_tick_ms:.1} ms per sample across {ticks} samples)"
    );
}
