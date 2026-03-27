use serde::{Deserialize, Serialize};
use std::process::Command;

fn limactl_cmd() -> Command {
    Command::new("limactl")
}

/// Lima VM instance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LimaInstance {
    pub name: String,
    pub status: String,
    pub arch: String,
    pub cpus: String,
    pub memory: String,
    pub disk: String,
    pub dir: String,
}

/// List Lima instances
#[tauri::command]
pub async fn lima_list() -> Result<Vec<LimaInstance>, String> {
    let output = limactl_cmd()
        .args(["list", "--json"])
        .output()
        .map_err(|e| format!("limactl not available: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "limactl list failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.trim().is_empty() {
        return Ok(vec![]);
    }

    // limactl list --json outputs one JSON object per line
    let instances: Vec<LimaInstance> = stdout
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|line| {
            let v: serde_json::Value = serde_json::from_str(line).ok()?;
            Some(LimaInstance {
                name: v["name"].as_str().unwrap_or("").to_string(),
                status: v["status"].as_str().unwrap_or("Unknown").to_string(),
                arch: v["arch"].as_str().unwrap_or("").to_string(),
                cpus: v["cpus"]
                    .as_i64()
                    .map(|n| n.to_string())
                    .unwrap_or_default(),
                memory: format_bytes_lima(v["memory"].as_i64().unwrap_or(0)),
                disk: format_bytes_lima(v["disk"].as_i64().unwrap_or(0)),
                dir: v["dir"].as_str().unwrap_or("").to_string(),
            })
        })
        .collect();

    Ok(instances)
}

fn format_bytes_lima(bytes: i64) -> String {
    if bytes >= 1073741824 {
        format!("{} GiB", bytes / 1073741824)
    } else if bytes >= 1048576 {
        format!("{} MiB", bytes / 1048576)
    } else {
        format!("{} B", bytes)
    }
}

/// Start a Lima instance
#[tauri::command]
pub async fn lima_start(name: String) -> Result<String, String> {
    let output = limactl_cmd()
        .args(["start", &name])
        .output()
        .map_err(|e| format!("Failed to start Lima instance: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "limactl start failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(format!("Lima instance '{}' started", name))
}

/// Stop a Lima instance
#[tauri::command]
pub async fn lima_stop(name: String) -> Result<String, String> {
    let output = limactl_cmd()
        .args(["stop", &name])
        .output()
        .map_err(|e| format!("Failed to stop Lima instance: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "limactl stop failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(format!("Lima instance '{}' stopped", name))
}

/// Delete a Lima instance
#[tauri::command]
pub async fn lima_delete(name: String, force: bool) -> Result<String, String> {
    let mut args = vec!["delete"];
    if force {
        args.push("--force");
    }
    args.push(&name);

    let output = limactl_cmd()
        .args(&args)
        .output()
        .map_err(|e| format!("Failed to delete Lima instance: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "limactl delete failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(format!("Lima instance '{}' deleted", name))
}

/// Get Lima instance info (shell)
#[tauri::command]
pub async fn lima_info(name: String) -> Result<String, String> {
    let output = limactl_cmd()
        .args(["info"])
        .output()
        .map_err(|e| format!("Failed to get Lima info: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "limactl info failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let _ = name; // info is global, name kept for API consistency
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Execute a command inside a Lima VM
#[tauri::command]
pub async fn lima_shell(name: String, command: String) -> Result<String, String> {
    let output = limactl_cmd()
        .args(["shell", &name, "--", "sh", "-c", &command])
        .output()
        .map_err(|e| format!("Failed to execute in Lima: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if !output.status.success() {
        return Err(format!("Command failed:\n{}\n{}", stdout, stderr));
    }

    Ok(format!("{}{}", stdout, stderr))
}

/// List available Lima templates
#[tauri::command]
pub async fn lima_templates() -> Result<String, String> {
    let output = limactl_cmd()
        .args(["start", "--list-templates"])
        .output()
        .map_err(|e| format!("Failed to list templates: {}", e))?;

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}
