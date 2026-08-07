use serde::{Deserialize, Serialize};
use std::process::Command;

/// Represents a Colima instance from `colima list --json`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColimaInstance {
    pub name: String,
    pub status: String,
    #[serde(default)]
    pub arch: String,
    #[serde(default)]
    pub cpus: u32,
    #[serde(default)]
    pub memory: u64,
    #[serde(default)]
    pub disk: u64,
    #[serde(default)]
    pub runtime: String,
    #[serde(default)]
    pub address: String,
    #[serde(default)]
    pub kubernetes: bool,
}

/// Extended status info from `colima status --json --extended`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstanceStatus {
    pub profile: String,
    pub status: String,
    pub arch: String,
    pub runtime: String,
    pub port_forwarding: String,
    #[serde(default)]
    pub cpu_usage: String,
    #[serde(default)]
    pub memory_usage: String,
    #[serde(default)]
    pub disk_usage: String,
    #[serde(default)]
    pub address: String,
}

/// Start instance configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartConfig {
    pub profile: String,
    pub runtime: String,
    pub cpus: u32,
    pub memory: u32,
    pub disk: u32,
    pub vm_type: String,
    #[serde(default)]
    pub kubernetes: bool,
    #[serde(default)]
    pub kubernetes_version: String,
    #[serde(default)]
    pub arch: String,
    #[serde(default)]
    pub mount_type: String,
    #[serde(default)]
    pub mounts: Vec<String>,
    #[serde(default)]
    pub dns: Vec<String>,
    #[serde(default)]
    pub network_address: bool,
}

fn colima_cmd() -> Command {
    let resolved = crate::path_util::resolve_binary("colima");
    let mut cmd = Command::new(&resolved);
    crate::path_util::apply_path_to_cmd(&mut cmd);
    cmd
}

/// List all Colima instances
/// Uses the fast filesystem reader (shared with API server) for consistency.
#[tauri::command]
pub async fn list_instances() -> Result<Vec<ColimaInstance>, String> {
    Ok(crate::instance_reader::list_instances_fast())
}

/// Start a Colima instance with given configuration
#[tauri::command]
pub async fn start_instance(config: StartConfig) -> Result<String, String> {
    // `colima start` blocks for 60-120s — run on thread pool to avoid starving tokio
    tokio::task::spawn_blocking(move || {
        let mut args = vec!["start".to_string()];

        if config.profile != "default" && !config.profile.is_empty() {
            args.push("--profile".to_string());
            args.push(config.profile.clone());
        }

        args.push("--runtime".to_string());
        args.push(config.runtime);

        args.push("--cpu".to_string());
        args.push(config.cpus.to_string());

        args.push("--memory".to_string());
        args.push(config.memory.to_string());

        args.push("--disk".to_string());
        args.push(config.disk.to_string());

        if !config.vm_type.is_empty() {
            args.push("--vm-type".to_string());
            args.push(config.vm_type);
        }

        if !config.arch.is_empty() {
            args.push("--arch".to_string());
            args.push(config.arch);
        }

        if !config.mount_type.is_empty() {
            args.push("--mount-type".to_string());
            args.push(config.mount_type);
        }

        for mount in &config.mounts {
            args.push("--mount".to_string());
            args.push(mount.clone());
        }

        for dns in &config.dns {
            args.push("--dns".to_string());
            args.push(dns.clone());
        }

        if config.network_address {
            args.push("--network-address".to_string());
        }

        if config.kubernetes {
            args.push("--kubernetes".to_string());
            if !config.kubernetes_version.is_empty() {
                args.push("--kubernetes-version".to_string());
                args.push(config.kubernetes_version);
            }
        }

        let args_ref: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let output = colima_cmd()
            .args(&args_ref)
            .output()
            .map_err(|e| format!("Failed to start colima: {}", e))?;

        if !output.status.success() {
            return Err(format!(
                "colima start failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        Ok(format!(
            "Instance '{}' started successfully",
            config.profile
        ))
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}

/// Stop a Colima instance (CLI-only, no Tauri state).
/// Used by HTTP route handlers which don't have access to Tauri managed state.
pub async fn stop_instance_cli(profile: String, force: bool) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        let mut args = vec!["stop".to_string()];
        if profile != "default" && !profile.is_empty() {
            args.push("--profile".to_string());
            args.push(profile.clone());
        }
        if force {
            args.push("--force".to_string());
        }

        let args_ref: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let output = colima_cmd()
            .args(&args_ref)
            .output()
            .map_err(|e| format!("Failed to stop colima: {}", e))?;

        if !output.status.success() {
            return Err(format!(
                "colima stop failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        Ok(format!("Instance '{}' stopped", profile))
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}

/// Delete a Colima instance (CLI-only, no Tauri state).
/// Used by HTTP route handlers which don't have access to Tauri managed state.
pub async fn delete_instance_cli(profile: String, force: bool) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        let mut args = vec!["delete".to_string()];
        if profile != "default" && !profile.is_empty() {
            args.push("--profile".to_string());
            args.push(profile.clone());
        }
        if force {
            args.push("--force".to_string());
        }

        let args_ref: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let output = colima_cmd()
            .args(&args_ref)
            .output()
            .map_err(|e| format!("Failed to delete colima: {}", e))?;

        if !output.status.success() {
            return Err(format!(
                "colima delete failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        Ok(format!("Instance '{}' deleted", profile))
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}

/// Stop a Colima instance
#[tauri::command]
pub async fn stop_instance(
    app: tauri::AppHandle,
    docker_state: tauri::State<'_, std::sync::Arc<tokio::sync::RwLock<crate::docker_state::DockerState>>>,
    profile: String,
    force: bool,
) -> Result<String, String> {
    // Proactively clear Docker state BEFORE stopping — user may navigate to Docker
    // tabs while colima stop is still running (fire-and-forget pattern in the UI).
    // If we clear after, Bollard queries succeed during the shutdown window.
    {
        let mut lock = docker_state.write().await;
        lock.docker = None;
        lock.containers_cache = vec![];
        lock.images_cache = vec![];
        lock.suppressed = true;
    }
    use tauri::Emitter;
    let _ = app.emit("docker-connection-lost", serde_json::json!({}));
    let _ = app.emit("docker-state-updated", serde_json::json!({
        "containers": [],
        "images": []
    }));

    // `colima stop` blocks for 30-60s — run on thread pool to avoid starving tokio
    tokio::task::spawn_blocking(move || {
        let mut args = vec!["stop".to_string()];

        if profile != "default" && !profile.is_empty() {
            args.push("--profile".to_string());
            args.push(profile.clone());
        }

        if force {
            args.push("--force".to_string());
        }

        let args_ref: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let output = colima_cmd()
            .args(&args_ref)
            .output()
            .map_err(|e| format!("Failed to stop colima: {}", e))?;

        if !output.status.success() {
            return Err(format!(
                "colima stop failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        Ok(format!("Instance '{}' stopped", profile))
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}

/// Delete a Colima instance
#[tauri::command]
pub async fn delete_instance(
    app: tauri::AppHandle,
    docker_state: tauri::State<'_, std::sync::Arc<tokio::sync::RwLock<crate::docker_state::DockerState>>>,
    profile: String,
    force: bool,
) -> Result<String, String> {
    {
        let mut lock = docker_state.write().await;
        lock.docker = None;
        lock.containers_cache = vec![];
        lock.images_cache = vec![];
        lock.suppressed = true;
    }
    use tauri::Emitter;
    let _ = app.emit("docker-connection-lost", serde_json::json!({}));
    let _ = app.emit("docker-state-updated", serde_json::json!({
        "containers": [],
        "images": []
    }));

    // `colima delete` can block — run on thread pool to avoid starving tokio
    tokio::task::spawn_blocking(move || {
        let mut args = vec!["delete".to_string()];

        if profile != "default" && !profile.is_empty() {
            args.push("--profile".to_string());
            args.push(profile.clone());
        }

        if force {
            args.push("--force".to_string());
        }

        let args_ref: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let output = colima_cmd()
            .args(&args_ref)
            .output()
            .map_err(|e| format!("Failed to delete colima: {}", e))?;

        if !output.status.success() {
            return Err(format!(
                "colima delete failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        Ok(format!("Instance '{}' deleted", profile))
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}

/// Get extended status of an instance
#[tauri::command]
pub async fn instance_status(profile: String) -> Result<InstanceStatus, String> {
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        tokio::task::spawn_blocking(move || {
            let mut args = vec!["status", "--json", "--extended"];

            let profile_flag;
            if profile != "default" && !profile.is_empty() {
                profile_flag = profile.clone();
                args.push("--profile");
                args.push(&profile_flag);
            }

            let output = colima_cmd()
                .args(&args)
                .output()
                .map_err(|e| format!("Failed to get status: {}", e))?;

            if !output.status.success() {
                return Err(format!(
                    "colima status failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                ));
            }

            let stdout = String::from_utf8_lossy(&output.stdout);
            let status: InstanceStatus =
                serde_json::from_str(&stdout).map_err(|e| format!("Failed to parse status: {}", e))?;

            Ok(status)
        }),
    )
    .await;

    match result {
        Ok(Ok(status)) => status,
        Ok(Err(e)) => Err(format!("Task join error: {}", e)),
        Err(_) => Err("colima status timed out (daemon may be unresponsive)".to_string()),
    }
}

/// SSH into a Colima instance (returns the command to execute)
#[tauri::command]
pub async fn get_ssh_command(profile: String) -> Result<Vec<String>, String> {
    let mut args = vec!["ssh".to_string()];
    if profile != "default" && !profile.is_empty() {
        args.push("--profile".to_string());
        args.push(profile);
    }
    Ok(args)
}

/// Kubernetes operations
#[tauri::command]
pub async fn kubernetes_action(profile: String, action: String) -> Result<String, String> {
    let valid_actions = ["start", "stop", "delete", "reset"];
    if !valid_actions.contains(&action.as_str()) {
        return Err(format!("Invalid kubernetes action: {}", action));
    }

    // `colima kubernetes` can block for a long time — run on thread pool
    tokio::task::spawn_blocking(move || {
        let mut args = vec!["kubernetes".to_string(), action.clone()];

        if profile != "default" && !profile.is_empty() {
            args.push("--profile".to_string());
            args.push(profile.clone());
        }

        let args_ref: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let output = colima_cmd()
            .args(&args_ref)
            .output()
            .map_err(|e| format!("Failed to execute kubernetes {}: {}", action, e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            // Treat "not enabled" / "not running" as success for delete/stop
            if (action == "delete" || action == "stop")
                && (stderr.contains("not enabled") || stderr.contains("not running"))
            {
                return Ok(format!(
                    "Kubernetes {} completed (already disabled)",
                    action
                ));
            }
            return Err(format!("kubernetes {} failed: {}", action, stderr));
        }

        Ok(format!("Kubernetes {} completed", action))
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}

// ===== Diagnostic Log Collection for AI Agent =====
// Reads Colima/Lima log files, checks for zombie processes,
// and inspects lock/pid/socket files to enable deep diagnostics.

#[tauri::command]
pub async fn collect_diagnostic_logs(profile: String) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/Users/unknown".to_string());
        let profile_dir = if profile.is_empty() || profile == "default" {
            "colima".to_string()
        } else {
            format!("colima-{}", profile)
        };
        let lima_dir = format!("{}/.colima/_lima/{}", home, profile_dir);
        let mut report = String::new();

        // Section 1: Lima VM log files
        report.push_str("## Lima VM Logs\n\n");
        for log_file in &["ha.stderr.log", "ha.stdout.log", "serial.log", "serialv.log"] {
            let path = format!("{}/{}", lima_dir, log_file);
            match std::fs::read_to_string(&path) {
                Ok(content) => {
                    // Take last 30 lines (most recent errors)
                    let lines: Vec<&str> = content.lines().collect();
                    let start = lines.len().saturating_sub(30);
                    let tail: String = lines[start..].join("\n");
                    if !tail.trim().is_empty() {
                        report.push_str(&format!("### {}\n```\n{}\n```\n\n", log_file, tail));
                    }
                }
                Err(_) => {}
            }
        }

        // Section 2: Check for zombie/stale processes
        report.push_str("## Running Processes\n\n");
        if let Ok(output) = std::process::Command::new("ps")
            .args(["aux"])
            .output()
        {
            let ps_output = String::from_utf8_lossy(&output.stdout);
            let relevant: Vec<&str> = ps_output
                .lines()
                .filter(|line| {
                    (line.contains("lima") || line.contains("colima") || line.contains("qemu") || line.contains("vz"))
                        && !line.contains("grep")
                        && !line.contains("ColimaUI")
                })
                .collect();
            if relevant.is_empty() {
                report.push_str("No Colima/Lima processes running.\n\n");
            } else {
                report.push_str(&format!("```\n{}\n```\n\n", relevant.join("\n")));
            }
        }

        // Section 3: Check for lock/pid/socket files
        report.push_str("## Lock/PID/Socket Files\n\n");
        if let Ok(entries) = std::fs::read_dir(&lima_dir) {
            let lock_files: Vec<String> = entries
                .filter_map(|e| e.ok())
                .filter(|e| {
                    let name = e.file_name().to_string_lossy().to_string();
                    name.ends_with(".pid") || name.ends_with(".sock") || name.ends_with(".lock")
                })
                .map(|e| {
                    let name = e.file_name().to_string_lossy().to_string();
                    let meta = e.metadata().ok();
                    let size = meta.as_ref().map(|m| m.len()).unwrap_or(0);
                    // For .pid files, read the content (it's a process ID)
                    let content = if name.ends_with(".pid") {
                        std::fs::read_to_string(e.path()).unwrap_or_default().trim().to_string()
                    } else {
                        String::new()
                    };
                    if content.is_empty() {
                        format!("  {} ({}B)", name, size)
                    } else {
                        format!("  {} (PID: {})", name, content)
                    }
                })
                .collect();
            if lock_files.is_empty() {
                report.push_str("No lock/pid/socket files found.\n\n");
            } else {
                report.push_str(&format!("{}\n\n", lock_files.join("\n")));
            }
        }

        // Section 4: Colima version info
        report.push_str("## System Info\n\n");
        if let Ok(output) = std::process::Command::new("colima").args(["version"]).output() {
            let ver = String::from_utf8_lossy(&output.stdout);
            report.push_str(&format!("```\n{}\n```\n", ver.trim()));
        }

        Ok(report)
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}

#[tauri::command]
pub async fn create_worker_node(master_profile: String, worker_profile: String, cpu: u32, memory: u32) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        // 1. Get Master IP
        let list_output = std::process::Command::new(crate::path_util::resolve_binary("colima"))
            .args(["list", "-j"])
            .output()
            .map_err(|e| format!("Failed to list instances: {}", e))?;
        let stdout = String::from_utf8_lossy(&list_output.stdout);
        let mut master_ip = String::new();
        for line in stdout.lines() {
            if let Ok(instance) = serde_json::from_str::<ColimaInstance>(line) {
                if instance.name == master_profile {
                    master_ip = instance.address;
                    break;
                }
            }
        }
        if master_ip.is_empty() {
            return Err(format!("Could not find master node '{}' or its IP address", master_profile));
        }

        // 2. Get Master Token
        let token_output = std::process::Command::new(crate::path_util::resolve_binary("colima"))
            .args(["ssh", "-p", &master_profile, "--", "sudo", "cat", "/var/lib/rancher/k3s/server/node-token"])
            .output()
            .map_err(|e| format!("Failed to get node token: {}", e))?;
        let token = String::from_utf8_lossy(&token_output.stdout).trim().to_string();
        if token.is_empty() {
            return Err("Failed to retrieve node token from master. Is kubernetes enabled?".to_string());
        }

        // 3. Start Worker Node
        let args = vec![
            "start".to_string(),
            "-p".to_string(),
            worker_profile,
            "--kubernetes".to_string(),
            "--cpu".to_string(),
            cpu.to_string(),
            "--memory".to_string(),
            memory.to_string(),
            "--network-address".to_string(),
            "--k3s-arg".to_string(),
            format!("--server=https://{}:6443", master_ip),
            "--k3s-arg".to_string(),
            format!("--token={}", token),
        ];

        let output = std::process::Command::new(crate::path_util::resolve_binary("colima"))
            .args(args)
            .output()
            .map_err(|e| format!("Failed to start worker node: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("Worker node start failed: {}", stderr));
        }

        Ok("Worker node created and joined successfully".to_string())
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}
