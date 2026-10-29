//! Direct filesystem reader for Colima instance state.
//!
//! Reads instance state directly from `~/.colima/` instead of shelling out
//! to `colima list --json` (which triggers slow macOS system_profiler calls).
//!
//! Structure:
//! ```text
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

/// Read the configured runtime from `~/.colima/<profile>/colima.yaml` directly without spawning `colima status`.
pub fn read_profile_runtime(profile: &str) -> String {
    read_profile_runtime_in(&colima_home(), profile)
}

/// Helper that reads profile runtime from a specified colima home directory (used in tests and internal callers).
pub fn read_profile_runtime_in(colima_home: &Path, profile: &str) -> String {
    if !crate::validation::is_valid_profile_name(profile) {
        return "docker".to_string();
    }
    let config_path = colima_home.join(profile).join("colima.yaml");
    if let Some(runtime) = std::fs::read_to_string(&config_path)
        .ok()
        .and_then(|content| serde_yml::from_str::<ColimaConfig>(&content).ok())
        .map(|cfg| cfg.runtime)
        .filter(|r| !r.is_empty())
    {
        return runtime;
    }
    "docker".to_string()
}

/// Resolve the accurate 4-tier status of a Colima instance:
/// - `"Stopped"`: Lima process is not alive
/// - `"Running"`: Lima process is alive, and either runtime is non-docker or Docker socket responds with HTTP 200 OK
/// - `"Starting"`: Lima process is alive, Docker socket not yet responding, and instance uptime < 90s
/// - `"Unhealthy"`: Lima process is alive, Docker socket not responding, and instance uptime >= 90s
pub fn resolve_instance_status(
    colima_home: &Path,
    profile: &str,
    lima_dir: &Path,
    runtime: &str,
) -> &'static str {
    if !crate::path_util::is_lima_instance_alive(lima_dir) {
        return "Stopped";
    }

    let is_docker = runtime.is_empty() || runtime == "docker";
    if is_docker {
        let sock = colima_home.join(profile).join("docker.sock");
        let healthy = crate::path_util::probe_docker_socket_cached(&sock)
            || (profile == "default"
                && crate::path_util::probe_docker_socket_cached(&colima_home.join("docker.sock")));
        if healthy {
            return "Running";
        }

        // Socket is not healthy. Determine if Starting (< 90s) or Unhealthy (>= 90s)
        let uptime_secs = std::fs::metadata(lima_dir.join("ha.pid"))
            .or_else(|_| std::fs::metadata(lima_dir.join("ha.sock")))
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.elapsed().ok())
            .map_or(0, |d| d.as_secs());

        if uptime_secs < 90 {
            "Starting"
        } else {
            "Unhealthy"
        }
    } else {
        "Running"
    }
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

    let runtime = if config.runtime.is_empty() {
        "docker".to_string()
    } else {
        config.runtime
    };

    let status = resolve_instance_status(colima_home, profile, &lima_dir, &runtime);

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
        status: status.to_string(),
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
        runtime,
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

    // Sort: active/starting/unhealthy instances above stopped instances, then by profile name
    instances.sort_by(|a, b| {
        let a_active = a.status != "Stopped";
        let b_active = b.status != "Stopped";
        b_active.cmp(&a_active).then(a.name.cmp(&b.name))
    });

    instances
}

/// Return the names of any *running* Colima profiles that use the containerd runtime.
///
/// The Docker state watcher uses this to explain why the Docker socket is missing
/// even though a Colima VM is running. ColimaUI currently communicates through the
/// Docker REST API only, so a profile started with `--runtime containerd` leaves the
/// UI without a usable socket.
pub fn running_containerd_profiles() -> Vec<String> {
    let home = colima_home();
    let mut profiles = Vec::new();
    if !home.exists() {
        return profiles;
    }

    if let Ok(entries) = std::fs::read_dir(&home) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('_') || name.starts_with('.') {
                continue;
            }
            if !entry.path().is_dir() {
                continue;
            }
            let lima_name = lima_instance_name(&name);
            let lima_dir = home.join("_lima").join(&lima_name);
            if !crate::path_util::is_lima_instance_alive(&lima_dir) {
                continue;
            }
            if let Some(inst) = read_instance(&home, &name) {
                if inst.runtime == "containerd" {
                    profiles.push(name);
                }
            }
        }
    }
    profiles
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    fn temp_test_dir(name: &str) -> PathBuf {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = PathBuf::from(format!("/tmp/cui-ir-test-{}-{}-{}", name, std::process::id(), id));
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::create_dir_all(&dir);
        dir
    }

    struct DirCleanup(PathBuf);
    impl Drop for DirCleanup {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn test_read_profile_runtime() {
        let home = temp_test_dir("runtime");
        let _guard = DirCleanup(home.clone());

        // Profile 1: runtime containerd
        let prof1_dir = home.join("containerd_prof");
        std::fs::create_dir_all(&prof1_dir).unwrap();
        std::fs::write(prof1_dir.join("colima.yaml"), "cpu: 2\nruntime: containerd\n").unwrap();

        // Profile 2: default docker when empty or missing
        let prof2_dir = home.join("docker_prof");
        std::fs::create_dir_all(&prof2_dir).unwrap();
        std::fs::write(prof2_dir.join("colima.yaml"), "cpu: 4\n").unwrap();

        assert_eq!(read_profile_runtime_in(&home, "containerd_prof"), "containerd");
        assert_eq!(read_profile_runtime_in(&home, "docker_prof"), "docker");
        assert_eq!(read_profile_runtime_in(&home, "nonexistent"), "docker");
    }

    #[test]
    #[cfg(unix)]
    fn test_resolve_instance_status_transitions() {
        let _guard = crate::path_util::test_probe_exclusive();
        crate::path_util::invalidate_probe_cache();

        let home = temp_test_dir("status");
        let _dir_guard = DirCleanup(home.clone());

        let profile = "testprof";
        let profile_dir = home.join(profile);
        std::fs::create_dir_all(&profile_dir).unwrap();

        let lima_dir = home.join("_lima").join("colima-testprof");
        std::fs::create_dir_all(&lima_dir).unwrap();

        // 1. Stopped: no ha.pid or ha.sock
        let status = resolve_instance_status(&home, profile, &lima_dir, "docker");
        assert_eq!(status, "Stopped");

        // 2. Starting: ha.pid with live PID, recent mtime (< 90s), but socket not responding
        let ha_pid = lima_dir.join("ha.pid");
        std::fs::write(&ha_pid, format!("{}\n", std::process::id())).unwrap();
        crate::path_util::invalidate_probe_cache();
        let status = resolve_instance_status(&home, profile, &lima_dir, "docker");
        assert_eq!(status, "Starting");

        // 3. Unhealthy: ha.pid with live PID, mtime >= 90s ago, socket not responding
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let old_time = (now - 120) as libc::time_t;
        let c_path = std::ffi::CString::new(ha_pid.to_str().unwrap()).unwrap();
        let times = [
            libc::timeval { tv_sec: old_time, tv_usec: 0 },
            libc::timeval { tv_sec: old_time, tv_usec: 0 },
        ];
        unsafe {
            libc::utimes(c_path.as_ptr(), times.as_ptr());
        }

        crate::path_util::invalidate_probe_cache();
        let status = resolve_instance_status(&home, profile, &lima_dir, "docker");
        assert_eq!(status, "Unhealthy");

        // 4. Running: Docker socket responds 200 OK.
        // Use distinct profile (testprof_running) so socket paths do not collide.
        let run_profile = "testprof_running";
        let run_profile_dir = home.join(run_profile);
        std::fs::create_dir_all(&run_profile_dir).unwrap();

        let run_lima_dir = home.join("_lima").join("colima-testprof_running");
        std::fs::create_dir_all(&run_lima_dir).unwrap();
        let run_ha_pid = run_lima_dir.join("ha.pid");
        std::fs::write(&run_ha_pid, format!("{}\n", std::process::id())).unwrap();

        let sock_path = run_profile_dir.join("docker.sock");
        let listener = std::os::unix::net::UnixListener::bind(&sock_path).unwrap();
        let handle = std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                use std::io::{Read, Write};
                let mut buf = [0u8; 128];
                let _ = stream.read(&mut buf);
                let resp = b"HTTP/1.1 200 OK\r\n\r\nOK";
                let _ = stream.write_all(resp);
            }
        });

        crate::path_util::invalidate_probe_cache();
        let status = resolve_instance_status(&home, run_profile, &run_lima_dir, "docker");
        assert_eq!(status, "Running");
        let _ = handle.join();

        // 5. Non-docker runtime (e.g. containerd) returns Running when Lima is alive
        let status_containerd = resolve_instance_status(&home, profile, &lima_dir, "containerd");
        assert_eq!(status_containerd, "Running");
    }

    #[test]
    fn test_list_instances_sort_order() {
        let mut instances = [
            ColimaInstance {
                name: "zebra".to_string(),
                status: "Stopped".to_string(),
                arch: "aarch64".to_string(),
                cpus: 2,
                memory: 1024,
                disk: 1024,
                runtime: "docker".to_string(),
                address: String::new(),
                kubernetes: false,
            },
            ColimaInstance {
                name: "alpha".to_string(),
                status: "Stopped".to_string(),
                arch: "aarch64".to_string(),
                cpus: 2,
                memory: 1024,
                disk: 1024,
                runtime: "docker".to_string(),
                address: String::new(),
                kubernetes: false,
            },
            ColimaInstance {
                name: "beta".to_string(),
                status: "Starting".to_string(),
                arch: "aarch64".to_string(),
                cpus: 2,
                memory: 1024,
                disk: 1024,
                runtime: "docker".to_string(),
                address: String::new(),
                kubernetes: false,
            },
            ColimaInstance {
                name: "gamma".to_string(),
                status: "Running".to_string(),
                arch: "aarch64".to_string(),
                cpus: 2,
                memory: 1024,
                disk: 1024,
                runtime: "docker".to_string(),
                address: String::new(),
                kubernetes: false,
            },
        ];

        instances.sort_by(|a, b| {
            let a_active = a.status != "Stopped";
            let b_active = b.status != "Stopped";
            b_active.cmp(&a_active).then(a.name.cmp(&b.name))
        });

        let names: Vec<&str> = instances.iter().map(|i| i.name.as_str()).collect();
        assert_eq!(names, vec!["beta", "gamma", "alpha", "zebra"]);
    }
}
