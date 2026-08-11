//! Direct filesystem reader for Colima instance state.
//!
//! Reads instance state directly from `~/.colima/` instead of shelling out
//! to `colima list --json` (which triggers slow macOS system_profiler calls).
//!
//! Structure:
//! ```
//! ~/.colima/
//! ├── default/colima.yaml          # profile config
//! ├── myprofile/colima.yaml        # another profile
//! ├── _lima/
//! │   ├── colima/                  # lima instance for "default"
//! │   │   ├── ha.sock             # exists = VM running
//! │   │   └── colima.yaml
//! │   └── colima-myprofile/       # lima instance for other profiles
//! └── _store/, _templates/        # internal dirs (ignored)
//! ```

use serde::Deserialize;
use std::path::{Path, PathBuf};

use crate::commands::colima::ColimaInstance;

/// Partial deserialize of colima.yaml — only the fields we need.
#[derive(Deserialize, Default)]
struct ColimaConfig {
    #[serde(default)]
    cpu: u32,
    #[serde(default)]
    memory: u32,
    #[serde(default)]
    disk: u32,
    #[serde(default)]
    arch: String,
    #[serde(default)]
    runtime: String,
    #[serde(default)]
    kubernetes: KubernetesConfig,
    #[serde(default)]
    #[allow(dead_code)]
    hostname: String,
}

#[derive(Deserialize, Default)]
struct KubernetesConfig {
    #[serde(default)]
    enabled: bool,
}

/// Get the colima home directory (~/.colima)
pub fn colima_home() -> PathBuf {
    if let Ok(home) = std::env::var("COLIMA_HOME") {
        return PathBuf::from(home);
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    PathBuf::from(home).join(".colima")
}

/// Map a profile name to its lima instance directory name.
/// "default" -> "colima", anything else -> "colima-{name}"
fn lima_instance_name(profile: &str) -> String {
    if profile == "default" {
        "colima".to_string()
    } else {
        format!("colima-{}", profile)
    }
}

/// Check if a lima instance is running by looking for ha.sock or ha.pid.
fn is_instance_running(lima_dir: &Path) -> bool {
    lima_dir.join("ha.sock").exists() || lima_dir.join("ha.pid").exists()
}

/// Resource fields of the lima instance colima generates from colima.yaml.
/// Lima writes sizes as strings ("4GiB"), cpus as a plain count.
#[derive(Deserialize, Default)]
struct LimaResources {
    #[serde(default)]
    cpus: u32,
    #[serde(default)]
    memory: String,
    #[serde(default)]
    disk: String,
}

/// Read cpus / memory / disk from `_lima/<instance>/lima.yaml`.
/// Returns `(cpus, memory_bytes, disk_bytes)`; any field lima omits comes back 0.
fn read_lima_resources(lima_dir: &Path) -> Option<(u32, u64, u64)> {
    let content = std::fs::read_to_string(lima_dir.join("lima.yaml")).ok()?;
    let res: LimaResources = serde_yml::from_str(&content).ok()?;
    Some((
        res.cpus,
        crate::commands::engine_resources::parse_size_to_bytes(&res.memory).unwrap_or(0),
        crate::commands::engine_resources::parse_size_to_bytes(&res.disk).unwrap_or(0),
    ))
}

/// Read a single instance's state from the filesystem.
fn read_instance(colima_home: &Path, profile: &str) -> Option<ColimaInstance> {
    // The profile name becomes a path component. Reject anything that could
    // walk out of ~/.colima before touching the filesystem — this function is
    // also reached from HTTP routes.
    //
    // Log rather than dropping silently: colima permits some names this
    // validator rejects, and a profile vanishing from the dashboard with no
    // explanation reads as "my VM was deleted".
    if !crate::validation::is_valid_profile_name(profile) {
        eprintln!(
            "[instance_reader] skipping profile with unsupported name: {:?}",
            profile
        );
        return None;
    }
    let config_path = colima_home.join(profile).join("colima.yaml");

    // Defence in depth behind the name check: prove the resolved path really is
    // inside ~/.colima before reading it.
    if let Err(e) = crate::validation::assert_path_within(colima_home, &config_path) {
        eprintln!("[instance_reader] refusing to read {:?}: {}", profile, e);
        return None;
    }
    let lima_name = lima_instance_name(profile);
    let lima_dir = colima_home.join("_lima").join(&lima_name);

    // Read and parse the colima.yaml config
    let config: ColimaConfig = if config_path.exists() {
        let content = std::fs::read_to_string(&config_path).ok()?;
        serde_yml::from_str(&content).unwrap_or_default()
    } else {
        // No config file — if lima dir also doesn't exist, this is a stale/deleted profile
        if !lima_dir.exists() {
            return None;
        }
        ColimaConfig::default()
    };

    let running = is_instance_running(&lima_dir);

    // Determine the display name (matches colima list output)
    // Colima lowercases profile names internally, so normalize to lowercase
    let name = if profile == "default" {
        "default".to_string()
    } else {
        profile.to_lowercase()
    };

    // Memory and disk are stored in GiB in colima.yaml but our struct uses bytes
    // to match what `colima list --json` returns
    let mut cpus = config.cpu;
    let mut memory_bytes = (config.memory as u64) * 1024 * 1024 * 1024;
    let mut disk_bytes = (config.disk as u64) * 1024 * 1024 * 1024;

    // colima.yaml is not always complete — profiles created by older colima
    // versions, or edited by hand, can omit these. The lima instance colima
    // generated from it always carries the real allocation, so prefer that over
    // inventing a number.
    if cpus == 0 || memory_bytes == 0 || disk_bytes == 0 {
        if let Some(lima) = read_lima_resources(&lima_dir) {
            if cpus == 0 {
                cpus = lima.0;
            }
            if memory_bytes == 0 {
                memory_bytes = lima.1;
            }
            if disk_bytes == 0 {
                disk_bytes = lima.2;
            }
        }
    }

    Some(ColimaInstance {
        name,
        status: if running {
            "Running".to_string()
        } else {
            "Stopped".to_string()
        },
        arch: if config.arch.is_empty() {
            std::env::consts::ARCH.to_string()
        } else {
            config.arch
        },
        // 0 means "unknown" and the UI renders it as such — a hardcoded 2 CPU /
        // 2 GiB / 60 GiB placeholder here used to be indistinguishable from a
        // real allocation.
        cpus,
        memory: memory_bytes,
        disk: disk_bytes,
        runtime: if config.runtime.is_empty() && running {
            "docker".to_string()
        } else {
            config.runtime
        },
        address: String::new(),
        kubernetes: config.kubernetes.enabled,
    })
}

/// Check if k3s is actually running for a profile via kubectl context check.
/// This is the fallback when colima.yaml says `kubernetes.enabled: false`
/// but K3s might actually be running (config out of sync).
/// Fix #11: Uses --request-timeout=3s to avoid blocking instance listing.
fn check_k3s_via_kubectl(profile: &str) -> bool {
    let context_name = if profile == "default" {
        "colima".to_string()
    } else {
        format!("colima-{}", profile)
    };

    if let Ok(output) = std::process::Command::new("kubectl")
        .args([
            "--context",
            &context_name,
            "--request-timeout=3s",
            "get",
            "nodes",
            "-o",
            "jsonpath={.items[0].status.nodeInfo.kubeletVersion}",
        ])
        .stderr(std::process::Stdio::null())
        .output()
    {
        if output.status.success() {
            let version = String::from_utf8_lossy(&output.stdout);
            return version.contains("k3s");
        }
    }
    false
}

/// List all Colima instances by reading the filesystem directly.
/// This is ~60,000x faster than `colima list --json` because it avoids
/// spawning a subprocess that triggers macOS system_profiler.
pub fn list_instances_fast() -> Vec<ColimaInstance> {
    let home = colima_home();

    if !home.exists() {
        return vec![];
    }

    let mut instances = Vec::new();

    // Scan profile directories (skip internal dirs starting with "_")
    if let Ok(entries) = std::fs::read_dir(&home) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();

            // Skip internal dirs, files, and hidden entries
            if name.starts_with('_') || name.starts_with('.') {
                continue;
            }

            // Skip non-directories (docker.sock, ssh_config, etc.)
            if !entry.path().is_dir() {
                continue;
            }

            if let Some(instance) = read_instance(&home, &name) {
                instances.push(instance);
            }
        }
    }

    // Enrich: for running instances where config says k8s disabled,
    // check kubectl to see if k3s is actually running (config out of sync)
    for inst in &mut instances {
        if inst.status == "Running" && !inst.kubernetes {
            let profile = if inst.name == "default" {
                "default"
            } else {
                &inst.name
            };
            inst.kubernetes = check_k3s_via_kubectl(profile);
        }
    }

    // Sort: running first, then alphabetical
    instances.sort_by(|a, b| {
        let a_running = a.status == "Running";
        let b_running = b.status == "Running";
        b_running.cmp(&a_running).then(a.name.cmp(&b.name))
    });

    instances
}
