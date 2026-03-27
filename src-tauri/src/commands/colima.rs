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
    Command::new("colima")
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
}

/// Stop a Colima instance
#[tauri::command]
pub async fn stop_instance(profile: String, force: bool) -> Result<String, String> {
    let mut args = vec!["stop"];

    let profile_flag;
    if profile != "default" && !profile.is_empty() {
        profile_flag = profile.clone();
        args.push("--profile");
        args.push(&profile_flag);
    }

    if force {
        args.push("--force");
    }

    let output = colima_cmd()
        .args(&args)
        .output()
        .map_err(|e| format!("Failed to stop colima: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "colima stop failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(format!("Instance '{}' stopped", profile))
}

/// Delete a Colima instance
#[tauri::command]
pub async fn delete_instance(profile: String, force: bool) -> Result<String, String> {
    let mut args = vec!["delete"];

    let profile_flag;
    if profile != "default" && !profile.is_empty() {
        profile_flag = profile.clone();
        args.push("--profile");
        args.push(&profile_flag);
    }

    if force {
        args.push("--force");
    }

    let output = colima_cmd()
        .args(&args)
        .output()
        .map_err(|e| format!("Failed to delete colima: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "colima delete failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(format!("Instance '{}' deleted", profile))
}

/// Get extended status of an instance
#[tauri::command]
pub async fn instance_status(profile: String) -> Result<InstanceStatus, String> {
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

    let mut args = vec!["kubernetes", action.as_str()];

    let profile_flag;
    if profile != "default" && !profile.is_empty() {
        profile_flag = profile.clone();
        args.push("--profile");
        args.push(&profile_flag);
    }

    let output = colima_cmd()
        .args(&args)
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
}
