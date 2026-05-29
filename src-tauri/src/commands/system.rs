use serde::Serialize;
use std::process::Command;
use crate::path_util;


#[derive(Debug, Clone, Serialize)]
pub struct SystemInfo {
    pub colima_installed: bool,
    pub colima_version: String,
    pub docker_installed: bool,
    pub docker_version: String,
    pub lima_installed: bool,
    pub lima_version: String,
}

fn get_version(cmd: &str, args: &[&str]) -> Option<String> {
    let resolved = path_util::resolve_binary(cmd);
    let mut command = Command::new(&resolved);
    command.args(args);
    path_util::apply_path_to_cmd(&mut command);
    command
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

/// Check system prerequisites
/// Runs on a blocking thread with a 5s timeout to prevent app startup hangs
/// when Docker/Colima daemons are unresponsive.
#[tauri::command]
pub async fn check_system() -> Result<SystemInfo, String> {
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        tokio::task::spawn_blocking(|| {
            let colima_version = get_version("colima", &["version"]);
            // Use --version (not context-dependent) to avoid Docker daemon connection
            let docker_version = get_version("docker", &["--version"]);
            let lima_version = get_version("limactl", &["--version"]);

            SystemInfo {
                colima_installed: colima_version.is_some(),
                colima_version: colima_version.unwrap_or_default(),
                docker_installed: docker_version.is_some(),
                docker_version: docker_version.unwrap_or_default(),
                lima_installed: lima_version.is_some(),
                lima_version: lima_version.unwrap_or_default(),
            }
        }),
    )
    .await;

    match result {
        Ok(Ok(info)) => Ok(info),
        Ok(Err(e)) => Err(format!("Task join error: {}", e)),
        Err(_) => {
            // Timeout — return partial info so the app doesn't freeze
            Ok(SystemInfo {
                colima_installed: false,
                colima_version: String::new(),
                docker_installed: false,
                docker_version: String::new(),
                lima_installed: false,
                lima_version: String::new(),
            })
        }
    }
}

/// Get Colima version string
#[tauri::command]
pub async fn get_colima_version() -> Result<String, String> {
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        tokio::task::spawn_blocking(|| {
            Command::new("colima")
                .args(["version"])
                .output()
                .map_err(|e| format!("Failed to get colima version: {}", e))
        }),
    )
    .await;

    match result {
        Ok(Ok(Ok(output))) => Ok(String::from_utf8_lossy(&output.stdout).trim().to_string()),
        Ok(Ok(Err(e))) => Err(e),
        Ok(Err(e)) => Err(format!("Task join error: {}", e)),
        Err(_) => Err("Colima version check timed out".to_string()),
    }
}

/// Check if an optional tool is installed
#[tauri::command]
pub async fn check_tool(name: String) -> Result<serde_json::Value, String> {
    let allowed = ["kubectl", "kind", "helm", "krunkit", "nerdctl"];
    if !allowed.contains(&name.as_str()) {
        return Err(format!("Unknown tool: {}", name));
    }
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        tokio::task::spawn_blocking(move || {
            let version = get_version(&name, &["version"])
                .or_else(|| get_version(&name, &["--version"]));
            serde_json::json!({
                "installed": version.is_some(),
                "version": version.unwrap_or_default()
            })
        }),
    )
    .await;

    match result {
        Ok(Ok(json)) => Ok(json),
        Ok(Err(e)) => Err(format!("Task join error: {}", e)),
        Err(_) => Ok(serde_json::json!({ "installed": false, "version": "" })),
    }
}

// ===== Host Hardware Specs =====

#[derive(Debug, Clone, Serialize)]
pub struct HostSpecs {
    pub cpu_cores: u32,
    pub memory_gib: u32,
    pub disk_free_gib: u32,
    pub disk_total_gib: u32,
    pub arch: String,
    pub model: String,
}

fn parse_sysctl_u64(key: &str) -> Option<u64> {
    Command::new("sysctl")
        .args(["-n", key])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| {
            String::from_utf8_lossy(&o.stdout)
                .trim()
                .parse::<u64>()
                .ok()
        })
}

pub fn detect_host_specs() -> HostSpecs {
    let arch = std::env::consts::ARCH.to_string();

    // CPU cores
    let cpu_cores = if cfg!(target_os = "macos") {
        parse_sysctl_u64("hw.ncpu").unwrap_or(1) as u32
    } else {
        // Linux: nproc or /proc/cpuinfo
        Command::new("nproc")
            .output()
            .ok()
            .filter(|o| o.status.success())
            .and_then(|o| String::from_utf8_lossy(&o.stdout).trim().parse::<u32>().ok())
            .unwrap_or(1)
    };

    // Total memory in GiB
    let memory_gib = if cfg!(target_os = "macos") {
        parse_sysctl_u64("hw.memsize")
            .map(|bytes| (bytes / 1073741824) as u32)
            .unwrap_or(8)
    } else {
        // Linux: /proc/meminfo
        std::fs::read_to_string("/proc/meminfo")
            .ok()
            .and_then(|content| {
                content
                    .lines()
                    .find(|l| l.starts_with("MemTotal:"))
                    .and_then(|l| {
                        l.split_whitespace()
                            .nth(1)
                            .and_then(|v| v.parse::<u64>().ok())
                            .map(|kb| (kb / 1048576) as u32) // kB -> GiB
                    })
            })
            .unwrap_or(8)
    };

    // Disk: free and total on root volume (GiB)
    let (disk_free_gib, disk_total_gib) = {
        let output = Command::new("df")
            .args(["-g", "/"])
            .output()
            .ok()
            .filter(|o| o.status.success());
        if let Some(o) = output {
            let stdout = String::from_utf8_lossy(&o.stdout);
            // Parse second line: Filesystem 1G-blocks Used Available ...
            if let Some(line) = stdout.lines().nth(1) {
                let cols: Vec<&str> = line.split_whitespace().collect();
                let total = cols.get(1).and_then(|v| v.parse::<u32>().ok()).unwrap_or(0);
                let free = cols.get(3).and_then(|v| v.parse::<u32>().ok()).unwrap_or(0);
                (free, total)
            } else {
                (0, 0)
            }
        } else {
            (0, 0)
        }
    };

    // Model name
    let model = if cfg!(target_os = "macos") {
        Command::new("sysctl")
            .args(["-n", "machdep.cpu.brand_string"])
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            // Apple Silicon doesn't have brand_string — fallback to hw.model
            .or_else(|| {
                Command::new("sysctl")
                    .args(["-n", "hw.model"])
                    .output()
                    .ok()
                    .filter(|o| o.status.success())
                    .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            })
            .unwrap_or_default()
    } else {
        // Linux: /proc/cpuinfo model name
        std::fs::read_to_string("/proc/cpuinfo")
            .ok()
            .and_then(|c| {
                c.lines()
                    .find(|l| l.starts_with("model name"))
                    .map(|l| l.split(':').nth(1).unwrap_or("").trim().to_string())
            })
            .unwrap_or_default()
    };

    HostSpecs {
        cpu_cores,
        memory_gib,
        disk_free_gib,
        disk_total_gib,
        arch,
        model,
    }
}

/// Get host machine hardware specifications
#[tauri::command]
pub async fn host_specs() -> Result<HostSpecs, String> {
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        tokio::task::spawn_blocking(detect_host_specs),
    )
    .await;

    match result {
        Ok(Ok(specs)) => Ok(specs),
        Ok(Err(e)) => Err(format!("Task join error: {}", e)),
        Err(_) => Err("Host specs detection timed out".to_string()),
    }

}

/// Read the bundled CONTEXT.md file and return its contents.
/// Used by the AI agent to inject authoritative ColimaUI + Colima knowledge
/// into the chat context (Tier 1 injection).
#[tauri::command]
pub async fn get_app_context(app: tauri::AppHandle) -> Result<String, String> {
    use tauri::Manager;
    // Tauri's resource_dir() resolves to Contents/Resources/ on macOS.
    // Files from "resources/*" in tauri.conf.json are bundled inside
    // Contents/Resources/resources/ (preserving the source subdirectory).
    let resource_path = app
        .path()
        .resource_dir()
        .map_err(|e| format!("Cannot resolve resource dir: {e}"))?
        .join("resources")
        .join("CONTEXT.md");

    std::fs::read_to_string(&resource_path)
        .map_err(|e| format!("Failed to read CONTEXT.md from {:?}: {e}", resource_path))
}
