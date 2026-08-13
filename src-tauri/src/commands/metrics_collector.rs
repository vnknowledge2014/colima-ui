//! The app's single sampling loop.
//!
//! Every live metric in the UI comes from here. The alternative — each component
//! polling `docker stats` for itself — multiplies daemon load by the number of
//! open views and produces figures that disagree with each other.
//!
//! Two output channels, deliberately separate:
//!
//! * **Display** — `publish_sse_event("metrics.sample", batch)`. Lossy by design:
//!   a client that falls behind gets a `stream-lagged` event and a gap in its
//!   chart, which is the honest rendering.
//! * **Durable** — the optional [`MetricWriter`]. Anything that must not lose
//!   samples registers here and receives every batch directly. Reading the SSE
//!   stream for that purpose would silently drop data under load.
//!
//! This module imports nothing from `pro` or `subscription`. The paid build
//! installs a writer at startup; that one call is the entire branch.
//!
//! ## Lifecycle
//!
//! Sampling runs only while something is watching, counted by
//! `sse::subscriber_count("metrics.sample")` — a count tied to the lifetime of
//! each HTTP stream, so a reloaded or crashed client stops being counted without
//! having to say so. With no watchers the loop makes no daemon calls at all.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, LazyLock, Mutex, RwLock};
use std::time::{Duration, Instant};

/// SSE topic. Also the key the subscriber registry counts against, so the UI must
/// subscribe with `?topics=metrics.sample` for sampling to start.
pub const TOPIC: &str = "metrics.sample";

/// How often the loop re-checks for watchers while idle.
const IDLE_POLL: Duration = Duration::from_secs(1);

const MIN_INTERVAL_MS: u64 = 1000;
const MAX_INTERVAL_MS: u64 = 60_000;

static INTERVAL_MS: AtomicU64 = AtomicU64::new(2000);

/// Samples taken since startup.
///
/// Exists so "the collector stops when nobody is watching" can be asserted
/// rather than asserted-about: the difference between idle and sampling is a
/// number here, not a judgement about CPU graphs.
static TICKS: AtomicU64 = AtomicU64::new(0);

/// How many samples the collector has taken.
pub fn ticks_taken() -> u64 {
    TICKS.load(Ordering::Relaxed)
}

/// One container, one instant.
///
/// Flat and self-describing on purpose: this becomes a table row wherever samples
/// are persisted, and a nested shape would have to be flattened there anyway.
///
/// `instance` is present from the first commit even though only one value exists
/// today. `detect_docker_host` picks the first running Colima profile, so the day
/// a second engine is supported, samples already written would otherwise be an
/// unattributable mix with no way to separate them after the fact.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricSample {
    /// Unix milliseconds.
    pub ts: i64,
    pub instance: String,
    pub container_id: String,
    pub name: String,
    pub cpu_pct: f64,
    pub mem_bytes: u64,
    pub mem_limit_bytes: u64,
    pub mem_pct: f64,
    pub net_rx_bytes: u64,
    pub net_tx_bytes: u64,
    pub block_read_bytes: u64,
    pub block_write_bytes: u64,
    pub pids: u32,
}

/// Receives every batch the collector produces.
///
/// A plain function rather than a bespoke trait: there is exactly one implementor
/// and a trait would only add a name for it. Pro installs one at startup with
/// [`set_metric_writer`].
pub type MetricWriter = Arc<dyn Fn(&[MetricSample]) + Send + Sync>;

static WRITER: RwLock<Option<MetricWriter>> = RwLock::new(None);

/// Install the durable sink. Called once at startup by builds that have one.
pub fn set_metric_writer(writer: MetricWriter) {
    if let Ok(mut slot) = WRITER.write() {
        *slot = Some(writer);
    }
}

/// Sampling period. Clamped: a zero would spin, and an hour would look broken.
pub fn set_interval_ms(ms: u64) {
    INTERVAL_MS.store(ms.clamp(MIN_INTERVAL_MS, MAX_INTERVAL_MS), Ordering::Relaxed);
}

pub fn interval_ms() -> u64 {
    INTERVAL_MS.load(Ordering::Relaxed)
}

#[tauri::command]
pub async fn set_metrics_interval(ms: u64) -> Result<u64, crate::error::ColimaError> {
    set_interval_ms(ms);
    Ok(interval_ms())
}

// ===== Parsing =====
//
// `docker stats --format json` reports pre-formatted strings, and mixes unit
// systems within a single row: memory uses binary units (`683MiB`) while network
// and block I/O use SI (`450kB`). Treating them alike is a 2.4% error on memory
// and a 7% error at gigabyte scale — enough to make the table disagree with
// `docker stats` for no visible reason.

/// Parse one Docker size token. `KiB`-style suffixes are 1024-based, `kB`-style
/// are 1000-based, matching what the Docker CLI prints.
fn parse_size(raw: &str) -> u64 {
    let s = raw.trim();
    if s.is_empty() || s == "--" || s == "N/A" {
        return 0;
    }
    let split = s
        .find(|c: char| c.is_ascii_alphabetic())
        .unwrap_or(s.len());
    let (number, unit) = s.split_at(split);
    let value: f64 = match number.trim().parse() {
        Ok(v) => v,
        Err(_) => return 0,
    };
    let unit = unit.trim();
    let binary = unit.contains('i') || unit.contains('I');
    let base: f64 = if binary { 1024.0 } else { 1000.0 };
    let power = match unit.chars().next().map(|c| c.to_ascii_lowercase()) {
        Some('k') => 1,
        Some('m') => 2,
        Some('g') => 3,
        Some('t') => 4,
        Some('p') => 5,
        // Bare "B", or no unit at all.
        _ => 0,
    };
    (value * base.powi(power)) as u64
}

/// Split `"683MiB / 1.913GiB"` into its two sides.
fn parse_pair(raw: &str) -> (u64, u64) {
    let mut parts = raw.split('/');
    let left = parts.next().unwrap_or("");
    let right = parts.next().unwrap_or("");
    (parse_size(left), parse_size(right))
}

fn parse_percent(raw: &str) -> f64 {
    raw.trim().trim_end_matches('%').trim().parse().unwrap_or(0.0)
}

fn field(v: &serde_json::Value, key: &str) -> String {
    v.get(key).and_then(|x| x.as_str()).unwrap_or("").to_string()
}

/// Turn one `docker stats --format json` line into a sample.
fn sample_from_stats(v: &serde_json::Value, ts: i64, instance: &str) -> Option<MetricSample> {
    let container_id = field(v, "ID");
    if container_id.is_empty() {
        return None;
    }
    let (mem_bytes, mem_limit_bytes) = parse_pair(&field(v, "MemUsage"));
    let (net_rx_bytes, net_tx_bytes) = parse_pair(&field(v, "NetIO"));
    let (block_read_bytes, block_write_bytes) = parse_pair(&field(v, "BlockIO"));

    Some(MetricSample {
        ts,
        instance: instance.to_string(),
        container_id,
        name: field(v, "Name"),
        cpu_pct: parse_percent(&field(v, "CPUPerc")),
        mem_bytes,
        mem_limit_bytes,
        mem_pct: parse_percent(&field(v, "MemPerc")),
        net_rx_bytes,
        net_tx_bytes,
        block_read_bytes,
        block_write_bytes,
        pids: field(v, "PIDs").trim().parse().unwrap_or(0),
    })
}

/// Parse a whole `docker stats` payload, which is one JSON object per line.
pub fn parse_stats(raw: &str, ts: i64, instance: &str) -> Vec<MetricSample> {
    raw.lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .filter_map(|v| sample_from_stats(&v, ts, instance))
        .collect()
}

// ===== Sampling through the engine API =====
//
// Measured, not assumed: `docker stats --no-stream` across 25 running containers
// cost **669 ms of CPU per sample and about six seconds of wall clock**, which is
// 11% average CPU at a 2 s period and makes that period unreachable — the loop
// spent its whole life inside one call.
//
// Two things cost that much. The CLI is a process spawned on every tick, and it
// collects *two* samples internally per container so it can difference them for a
// CPU percentage.
//
// Neither is necessary here. This sampler asks the engine API for one shot per
// container and differences it against the previous tick, which this loop already
// has. That is also better data: the delta spans the real sampling period rather
// than the CLI's internal one-second window.
//
// The CLI path remains for engines that do not speak the Docker API — `nerdctl`
// on containerd being the one that matters.

/// Previous CPU counters per container, for the delta the API does not give us.
type CpuCounters = (u64, u64);
static PREV_CPU: LazyLock<Mutex<HashMap<String, CpuCounters>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Docker's own CPU-percent formula.
///
/// Deliberately the same arithmetic the CLI performs, so the table agrees with
/// `docker stats` rather than being defensibly-different.
fn cpu_percent(cur: CpuCounters, prev: CpuCounters, online_cpus: u64) -> f64 {
    let (cur_total, cur_system) = cur;
    let (prev_total, prev_system) = prev;
    let cpu_delta = cur_total.saturating_sub(prev_total) as f64;
    let system_delta = cur_system.saturating_sub(prev_system) as f64;
    if system_delta <= 0.0 || cpu_delta <= 0.0 || online_cpus == 0 {
        return 0.0;
    }
    (cpu_delta / system_delta) * online_cpus as f64 * 100.0
}

/// Memory actually in use, as Docker reports it: page cache that can be reclaimed
/// does not count against the container.
fn memory_used(stats: &bollard::container::MemoryStats) -> u64 {
    use bollard::container::MemoryStatsStats;
    let usage = stats.usage.unwrap_or(0);
    let inactive = match stats.stats {
        Some(MemoryStatsStats::V1(v1)) => v1.total_inactive_file,
        Some(MemoryStatsStats::V2(v2)) => v2.inactive_file,
        None => 0,
    };
    usage.saturating_sub(inactive)
}

fn blkio_total(stats: &bollard::container::BlkioStats, op: &str) -> u64 {
    stats
        .io_service_bytes_recursive
        .as_ref()
        .map_or(0, |entries| {
            entries
                .iter()
                .filter(|e| e.op.eq_ignore_ascii_case(op))
                .map(|e| e.value)
                .sum()
        })
}

/// Turn one API reading into a sample, differencing CPU against the last tick.
fn sample_from_api(
    stats: &bollard::container::Stats,
    id: &str,
    name: &str,
    ts: i64,
    instance: &str,
) -> MetricSample {
    let cur: CpuCounters = (
        stats.cpu_stats.cpu_usage.total_usage,
        stats.cpu_stats.system_cpu_usage.unwrap_or(0),
    );
    let online = stats.cpu_stats.online_cpus.unwrap_or_else(|| {
        stats
            .cpu_stats
            .cpu_usage
            .percpu_usage
            .as_ref()
            .map_or(1, |v| v.len() as u64)
    });

    // No previous reading means no elapsed interval to divide by. Reporting 0%
    // matches what `docker stats` shows for a container's first sample.
    let prev = PREV_CPU.lock().ok().and_then(|m| m.get(id).copied());
    let cpu_pct = prev.map_or(0.0, |p| cpu_percent(cur, p, online));
    if let Ok(mut m) = PREV_CPU.lock() {
        m.insert(id.to_string(), cur);
    }

    let mem_bytes = memory_used(&stats.memory_stats);
    let mem_limit_bytes = stats.memory_stats.limit.unwrap_or(0);
    let (net_rx_bytes, net_tx_bytes) = stats
        .networks
        .as_ref()
        .map_or((0, 0), |nets| {
            nets.values()
                .fold((0u64, 0u64), |(rx, tx), n| (rx + n.rx_bytes, tx + n.tx_bytes))
        });

    MetricSample {
        ts,
        instance: instance.to_string(),
        container_id: id.chars().take(12).collect(),
        name: name.to_string(),
        cpu_pct,
        mem_bytes,
        mem_limit_bytes,
        mem_pct: if mem_limit_bytes > 0 {
            mem_bytes as f64 / mem_limit_bytes as f64 * 100.0
        } else {
            0.0
        },
        net_rx_bytes,
        net_tx_bytes,
        block_read_bytes: blkio_total(&stats.blkio_stats, "read"),
        block_write_bytes: blkio_total(&stats.blkio_stats, "write"),
        pids: stats.pids_stats.current.unwrap_or(0) as u32,
    }
}

/// Sample every running container through the engine API.
///
/// Returns `None` when the API is unreachable, so the caller can fall back to the
/// CLI rather than reporting an empty machine.
async fn sample_via_api(ts: i64, instance: &str) -> Option<Vec<MetricSample>> {
    use bollard::container::{ListContainersOptions, StatsOptions};
    use futures_util::StreamExt;

    let docker = crate::docker_state::connect_bollard()?;
    let containers = docker
        .list_containers(Some(ListContainersOptions::<String> {
            all: false,
            ..Default::default()
        }))
        .await
        .ok()?;

    // Concurrently: twenty sequential round-trips over a socket would reintroduce
    // the latency this exists to remove.
    let readings = futures_util::future::join_all(containers.into_iter().map(|c| {
        let docker = docker.clone();
        async move {
            let id = c.id?;
            let name = c
                .names
                .and_then(|n| n.first().cloned())
                .unwrap_or_default()
                .trim_start_matches('/')
                .to_string();
            let stats = docker
                .stats(
                    &id,
                    Some(StatsOptions {
                        stream: false,
                        // One reading, returned immediately. The previous tick is
                        // the other half of the difference.
                        one_shot: true,
                    }),
                )
                .next()
                .await?
                .ok()?;
            Some((id, name, stats))
        }
    }))
    .await;

    let samples: Vec<MetricSample> = readings
        .into_iter()
        .flatten()
        .map(|(id, name, stats)| sample_from_api(&stats, &id, &name, ts, instance))
        .collect();

    // Forget containers that are gone, so the counter map cannot grow forever on
    // a machine that churns containers.
    if let Ok(mut prev) = PREV_CPU.lock() {
        let live: std::collections::HashSet<String> = samples
            .iter()
            .map(|s| s.container_id.clone())
            .collect();
        prev.retain(|id, _| live.iter().any(|l| id.starts_with(l.as_str())));
    }

    Some(samples)
}

/// Which engine the samples came from.
fn current_instance() -> String {
    crate::path_util::detect_docker_host()
        .map_or_else(|| "unknown".to_string(), |(_, profile)| profile)
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

// ===== Collector =====

// Engine-wide figures are deliberately **not** collected here.
//
// `engine_resources` derives aggregate CPU by running `docker stats --no-stream`,
// measured at 2.04 s with 25 containers — the single most expensive call this app
// makes. The collector already holds per-container CPU and memory, so the
// aggregate is a sum it can do for free; repeating the expensive call to obtain a
// number already in hand was most of what live monitoring cost.
//
// The slow-changing facts — core count, total memory, versions, disk — come from
// `/api/system/engine-resources`, which the Activity page fetches once when it
// opens. They do not belong in a two-second sample.

/// Take one sample and publish it.
///
/// One `docker stats` call for every container and **one** event for the whole
/// batch: an event per container would multiply the per-tick overhead by the
/// container count on both sides of the wire.
/// Take one reading, without publishing it.
///
/// The API path is the fast one; the CLI is the fallback for engines that do not
/// speak the Docker API. See the note above [`sample_via_api`].
///
/// Public so the sampling path itself can be tested against `docker stats`
/// without having to stand up an SSE subscriber to observe it.
pub async fn sample_now() -> Result<Vec<MetricSample>, String> {
    let ts = now_ms();
    let instance = current_instance();

    if let Some(samples) = sample_via_api(ts, &instance).await {
        return Ok(samples);
    }
    crate::commands::containers::all_container_stats()
        .await
        .map(|raw| parse_stats(&raw, ts, &instance))
        .map_err(|e| e.to_string())
}

async fn collect_once() {
    TICKS.fetch_add(1, Ordering::Relaxed);

    let samples = match sample_now().await {
        Ok(samples) => samples,
        Err(e) => {
            crate::sse::publish_sse_event(
                TOPIC,
                &serde_json::json!({ "error": e, "samples": [] }),
            );
            return;
        }
    };

    // Durable channel first: a writer must not miss a batch because the display
    // channel was busy.
    let writer = WRITER.read().ok().and_then(|w| w.clone());
    if let Some(writer) = writer {
        writer(&samples);
    }

    crate::sse::publish_sse_event(
        TOPIC,
        &serde_json::json!({
            "samples": samples,
            "intervalMs": interval_ms(),
        }),
    );
}

/// Start the loop. Call once, at startup.
///
/// While nothing is watching this wakes once a second to check and does nothing
/// else — no daemon calls, no allocation. A condition variable would avoid even
/// that, but the signal would have to be raised inside the subscriber registry,
/// and a once-a-second integer read is not worth reaching into another module's
/// locking for.
pub fn spawn_collector() {
    tauri::async_runtime::spawn(async move {
        loop {
            if crate::sse::subscriber_count(TOPIC) == 0 {
                tokio::time::sleep(IDLE_POLL).await;
                continue;
            }

            let started = Instant::now();
            collect_once().await;

            // Subtract the time sampling took. Without this, a slow daemon turns
            // the period into "interval plus however long docker felt like",
            // and the UI's x-axis quietly stops meaning what it says.
            let period = Duration::from_millis(interval_ms());
            tokio::time::sleep(period.saturating_sub(started.elapsed())).await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binary_and_si_suffixes_are_not_confused() {
        // Docker mixes them within one row; treating MiB as MB is a 4.9% error.
        assert_eq!(parse_size("1KiB"), 1024);
        assert_eq!(parse_size("1kB"), 1000);
        assert_eq!(parse_size("1MiB"), 1024 * 1024);
        assert_eq!(parse_size("1MB"), 1_000_000);
        assert_eq!(parse_size("1GiB"), 1024 * 1024 * 1024);
        assert_eq!(parse_size("1GB"), 1_000_000_000);
        assert_eq!(parse_size("512B"), 512);
        assert_eq!(parse_size("0B"), 0);
    }

    #[test]
    fn fractional_and_spaced_sizes_parse() {
        assert_eq!(parse_size("1.913GiB"), 2_054_068_109);
        assert_eq!(parse_size("30.8MB"), 30_800_000);
        assert_eq!(parse_size(" 683MiB "), 716_177_408);
    }

    #[test]
    fn unavailable_values_read_as_zero_rather_than_failing() {
        // A stopped container reports these; a parse error here would drop the
        // whole row and make the container vanish from the table.
        for raw in ["--", "N/A", "", "  ", "garbage"] {
            assert_eq!(parse_size(raw), 0, "input: {:?}", raw);
        }
    }

    #[test]
    fn pairs_split_on_the_slash() {
        assert_eq!(parse_pair("683MiB / 1.913GiB"), (716_177_408, 2_054_068_109));
        assert_eq!(parse_pair("450kB / 234kB"), (450_000, 234_000));
        assert_eq!(parse_pair("0B / 0B"), (0, 0));
        // Malformed input yields zeros, not a panic.
        assert_eq!(parse_pair(""), (0, 0));
        assert_eq!(parse_pair("only-one-side"), (0, 0));
    }

    #[test]
    fn percentages_drop_the_sign() {
        assert_eq!(parse_percent("0.00%"), 0.0);
        assert_eq!(parse_percent("34.87%"), 34.87);
        assert_eq!(parse_percent("102.5%"), 102.5);
        assert_eq!(parse_percent("--"), 0.0);
    }

    /// A real line, captured from `docker stats --no-stream --format json`.
    #[test]
    fn a_real_stats_line_becomes_a_sample() {
        let line = r#"{"BlockIO":"583MB / 30.8MB","CPUPerc":"1.25%","Container":"e213daa57c95","ID":"e213daa57c95","MemPerc":"34.87%","MemUsage":"683MiB / 1.913GiB","Name":"recipe-web-1","NetIO":"450kB / 234kB","PIDs":"37"}"#;
        let samples = parse_stats(line, 1_700_000_000_000, "colima");
        assert_eq!(samples.len(), 1);
        let s = &samples[0];
        assert_eq!(s.container_id, "e213daa57c95");
        assert_eq!(s.name, "recipe-web-1");
        assert_eq!(s.cpu_pct, 1.25);
        assert_eq!(s.mem_bytes, 716_177_408);
        assert_eq!(s.mem_limit_bytes, 2_054_068_109);
        assert_eq!(s.mem_pct, 34.87);
        assert_eq!(s.net_rx_bytes, 450_000);
        assert_eq!(s.net_tx_bytes, 234_000);
        assert_eq!(s.block_read_bytes, 583_000_000);
        assert_eq!(s.block_write_bytes, 30_800_000);
        assert_eq!(s.pids, 37);
        assert_eq!(s.instance, "colima");
        assert_eq!(s.ts, 1_700_000_000_000);
    }

    #[test]
    fn a_multi_line_payload_yields_one_sample_per_container() {
        let raw = concat!(
            r#"{"ID":"aaa","Name":"one","CPUPerc":"1%","MemUsage":"1MiB / 2MiB","MemPerc":"50%","NetIO":"0B / 0B","BlockIO":"0B / 0B","PIDs":"1"}"#,
            "\n",
            r#"{"ID":"bbb","Name":"two","CPUPerc":"2%","MemUsage":"2MiB / 4MiB","MemPerc":"50%","NetIO":"0B / 0B","BlockIO":"0B / 0B","PIDs":"2"}"#,
            "\n\n",
        );
        let samples = parse_stats(raw, 1, "x");
        assert_eq!(samples.len(), 2);
        assert_eq!(samples[0].container_id, "aaa");
        assert_eq!(samples[1].container_id, "bbb");
    }

    #[test]
    fn rows_without_an_id_are_skipped_not_zero_filled() {
        // A row we cannot attribute is worse than no row: it would appear in the
        // table as a nameless container using no resources.
        let raw = r#"{"Name":"ghost","CPUPerc":"5%"}"#;
        assert!(parse_stats(raw, 1, "x").is_empty());
    }

    #[test]
    fn malformed_lines_do_not_take_the_batch_down() {
        let raw = concat!(
            "not json\n",
            r#"{"ID":"ok","Name":"fine","CPUPerc":"1%","MemUsage":"1MiB / 2MiB","MemPerc":"50%","NetIO":"0B / 0B","BlockIO":"0B / 0B","PIDs":"1"}"#,
            "\n",
        );
        assert_eq!(parse_stats(raw, 1, "x").len(), 1);
    }

    #[test]
    fn the_interval_is_clamped_to_something_sane() {
        let original = interval_ms();
        set_interval_ms(0);
        assert_eq!(interval_ms(), MIN_INTERVAL_MS);
        set_interval_ms(u64::MAX);
        assert_eq!(interval_ms(), MAX_INTERVAL_MS);
        set_interval_ms(2000);
        assert_eq!(interval_ms(), 2000);
        set_interval_ms(original);
    }

    #[test]
    fn a_writer_receives_the_batch_it_was_given() {
        use std::sync::atomic::AtomicUsize;
        static SEEN: AtomicUsize = AtomicUsize::new(0);

        set_metric_writer(Arc::new(|batch: &[MetricSample]| {
            SEEN.fetch_add(batch.len(), Ordering::SeqCst);
        }));
        let writer = WRITER.read().unwrap().clone().expect("writer installed");
        writer(&parse_stats(
            r#"{"ID":"a","Name":"n","CPUPerc":"0%","MemUsage":"0B / 0B","MemPerc":"0%","NetIO":"0B / 0B","BlockIO":"0B / 0B","PIDs":"0"}"#,
            1,
            "x",
        ));
        assert_eq!(SEEN.load(Ordering::SeqCst), 1);

        // Leave the slot empty: other tests must not inherit this writer.
        if let Ok(mut slot) = WRITER.write() {
            *slot = None;
        }
    }

    /// The collector is the boundary Pro plugs into; it must not depend on Pro.
    ///
    /// The needles are assembled at runtime: written as literals they would
    /// appear in this file and the check would fail on its own source.
    #[test]
    fn this_module_does_not_reference_the_paid_build() {
        let forbidden = [format!("crate::{}", "pro"), format!("crate::{}", "subscription")];
        let source = include_str!("metrics_collector.rs");
        for line in source.lines() {
            // Comments may name the paid build; only code may not reach into it.
            let code = match line.find("//") {
                Some(at) => &line[..at],
                None => line,
            };
            for needle in &forbidden {
                assert!(
                    !code.contains(needle.as_str()),
                    "collector must stay independent of the paid build: {}",
                    line
                );
            }
        }
    }
}
