//! Live CPU / memory / disk figures for whichever container engine is active.
//!
//! The dashboard used to derive every resource number from `~/.colima/<profile>/colima.yaml`,
//! which is the *allocated* VM config and only exists when Colima manages the engine.
//! With Docker Desktop, OrbStack, Rancher, or a Colima profile whose config was never
//! written, that file is missing and every figure collapsed to zero.
//!
//! These numbers come from the engine itself instead:
//!   - `docker info`      -> cores and memory the engine can use
//!   - `docker stats`     -> what running containers currently consume
//!   - `docker system df` -> what images/containers/volumes/build cache occupy on disk

use serde::Serialize;

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineResources {
    /// False when the engine is not reachable — the UI falls back to VM config numbers.
    pub available: bool,
    /// Engine host name reported by `docker info` (e.g. "colima", "orbstack").
    pub engine_name: String,
    pub server_version: String,
    pub operating_system: String,
    /// Cores the engine can schedule onto.
    pub cpu_cores: u32,
    /// Sum of per-container CPU%, relative to a single core (200% = two cores busy).
    pub cpu_percent: f64,
    pub memory_total_bytes: u64,
    pub memory_used_bytes: u64,
    pub disk_used_bytes: u64,
    pub disk_reclaimable_bytes: u64,
    pub containers_running: u32,
}

/// Parse the human-readable sizes docker CLI prints ("28.43GB", "16.18MiB", "0B").
///
/// Docker mixes SI and binary units in the same output, so both are honoured
/// literally: `GB` is 1000^3, `GiB` is 1024^3. Trailing annotations such as
/// `"22.31GB (78%)"` from `system df` are ignored.
pub fn parse_size_to_bytes(input: &str) -> Option<u64> {
    let text = input.trim();
    let text = text.split('(').next()?.trim();
    if text.is_empty() || text == "N/A" || text == "--" {
        return None;
    }

    let split_at = text
        .find(|c: char| !(c.is_ascii_digit() || c == '.' || c == ','))
        .unwrap_or(text.len());
    let (number, unit) = text.split_at(split_at);
    let value: f64 = number.replace(',', "").parse().ok()?;

    let multiplier: f64 = match unit.trim().to_ascii_lowercase().as_str() {
        "" | "b" => 1.0,
        "kb" | "k" => 1e3,
        "kib" => 1024.0,
        "mb" | "m" => 1e6,
        "mib" => 1024f64.powi(2),
        "gb" | "g" => 1e9,
        "gib" => 1024f64.powi(3),
        "tb" | "t" => 1e12,
        "tib" => 1024f64.powi(4),
        "pb" => 1e15,
        "pib" => 1024f64.powi(5),
        _ => return None,
    };

    Some((value * multiplier) as u64)
}

/// Parse the "used / limit" pair `docker stats` puts in MemUsage.
fn parse_mem_usage(input: &str) -> (u64, u64) {
    let mut parts = input.split('/');
    let used = parts.next().and_then(parse_size_to_bytes).unwrap_or(0);
    let limit = parts.next().and_then(parse_size_to_bytes).unwrap_or(0);
    (used, limit)
}

fn parse_percent(input: &str) -> f64 {
    input.trim().trim_end_matches('%').parse().unwrap_or(0.0)
}

/// Run an engine command, returning stdout only when it exited successfully.
fn run(args: &[&str]) -> Option<String> {
    let output = crate::commands::runtime::get_runtime_cmd()
        .args(args)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).to_string())
}

/// `docker info` — cores, memory ceiling, engine identity.
fn collect_info(res: &mut EngineResources) -> bool {
    // One templated line instead of `--format json`: far less output to parse and
    // it works the same on docker, podman, and nerdctl.
    let raw = match run(&[
        "info",
        "--format",
        "{{.NCPU}}\t{{.MemTotal}}\t{{.ServerVersion}}\t{{.Name}}\t{{.OperatingSystem}}",
    ]) {
        Some(v) => v,
        None => return false,
    };

    let line = raw.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
    let fields: Vec<&str> = line.split('\t').collect();
    if fields.len() < 2 {
        return false;
    }

    res.cpu_cores = fields[0].trim().parse().unwrap_or(0);
    res.memory_total_bytes = fields[1].trim().parse().unwrap_or(0);
    res.server_version = fields.get(2).unwrap_or(&"").trim().to_string();
    res.engine_name = fields.get(3).unwrap_or(&"").trim().to_string();
    res.operating_system = fields.get(4).unwrap_or(&"").trim().to_string();
    true
}

/// `docker stats` — live CPU and memory consumption across running containers.
fn collect_stats(res: &mut EngineResources) {
    let Some(raw) = run(&["stats", "--no-stream", "--format", "json"]) else {
        return;
    };

    for line in raw.lines().filter(|l| !l.trim().is_empty()) {
        let Ok(entry) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        res.containers_running += 1;
        res.cpu_percent += parse_percent(entry["CPUPerc"].as_str().unwrap_or("0%"));

        let (used, limit) = parse_mem_usage(entry["MemUsage"].as_str().unwrap_or(""));
        res.memory_used_bytes += used;
        // Docker Desktop and OrbStack occasionally report MemTotal as 0 in `info`;
        // the per-container limit is the same ceiling, so use it as a fallback.
        if res.memory_total_bytes == 0 && limit > 0 {
            res.memory_total_bytes = limit;
        }
    }
}

/// `docker system df` — disk occupied by images, containers, volumes, build cache.
fn collect_disk(res: &mut EngineResources) {
    let Some(raw) = run(&["system", "df", "--format", "json"]) else {
        return;
    };

    for line in raw.lines().filter(|l| !l.trim().is_empty()) {
        let Ok(entry) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        res.disk_used_bytes += entry["Size"].as_str().and_then(parse_size_to_bytes).unwrap_or(0);
        res.disk_reclaimable_bytes += entry["Reclaimable"]
            .as_str()
            .and_then(parse_size_to_bytes)
            .unwrap_or(0);
    }
}

fn detect_engine_resources() -> EngineResources {
    let mut res = EngineResources::default();
    if !collect_info(&mut res) {
        return res;
    }
    res.available = true;
    collect_stats(&mut res);
    collect_disk(&mut res);
    res
}

/// Resource figures for the active container engine.
///
/// Never errors on an unreachable engine: it returns `available: false` so the
/// dashboard can fall back to VM-config numbers instead of showing an error card.
#[tauri::command]
pub async fn engine_resources() -> Result<EngineResources, crate::error::ColimaError> {
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(15),
        tokio::task::spawn_blocking(detect_engine_resources),
    )
    .await;

    match result {
        Ok(Ok(res)) => Ok(res),
        // A join failure or timeout is indistinguishable from "engine is wedged"
        // as far as the UI is concerned — both mean: use the fallback.
        _ => Ok(EngineResources::default()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_si_and_binary_units() {
        assert_eq!(parse_size_to_bytes("0B"), Some(0));
        assert_eq!(parse_size_to_bytes("28.43GB"), Some(28_430_000_000));
        assert_eq!(parse_size_to_bytes("1KiB"), Some(1024));
        assert_eq!(parse_size_to_bytes("2MiB"), Some(2 * 1024 * 1024));
        assert_eq!(parse_size_to_bytes("1.315MB"), Some(1_315_000));
    }

    #[test]
    fn ignores_reclaimable_percentage_suffix() {
        assert_eq!(parse_size_to_bytes("22.31GB (78%)"), Some(22_310_000_000));
    }

    #[test]
    fn rejects_unparseable_sizes() {
        assert_eq!(parse_size_to_bytes(""), None);
        assert_eq!(parse_size_to_bytes("N/A"), None);
        assert_eq!(parse_size_to_bytes("12quux"), None);
    }

    #[test]
    fn splits_mem_usage_pair() {
        let (used, limit) = parse_mem_usage("16.18MiB / 7.818GiB");
        assert_eq!(used, 16_965_959);
        assert_eq!(limit, 8_394_513_580);
    }

    #[test]
    fn parses_cpu_percentage() {
        assert_eq!(parse_percent("12.34%"), 12.34);
        assert_eq!(parse_percent("0.00%"), 0.0);
        assert_eq!(parse_percent("--"), 0.0);
    }
}
