use serde::{Deserialize, Serialize};
use std::process::Command;

/// Docker volume info
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DockerVolume {
    #[serde(alias = "Name")]
    pub name: String,
    #[serde(alias = "Driver")]
    pub driver: String,
    #[serde(default, alias = "Mountpoint")]
    pub mountpoint: String,
    #[serde(default, alias = "Scope")]
    pub scope: String,
    #[serde(default, alias = "Labels")]
    pub labels: String,
}

/// Volume inspect info (raw JSON)
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeInspect {
    pub name: String,
    pub driver: String,
    pub mountpoint: String,
    pub scope: String,
    #[serde(default)]
    pub labels: serde_json::Value,
    #[serde(default)]
    pub options: serde_json::Value,
    #[serde(default)]
    pub created_at: String,
}

fn docker_cmd() -> Command {
    Command::new("docker")
}

/// List all Docker volumes
#[tauri::command]
pub async fn list_volumes() -> Result<Vec<DockerVolume>, String> {
    let output = docker_cmd()
        .args(["volume", "ls", "--format", "json"])
        .output()
        .map_err(|e| format!("Failed to list volumes: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "docker volume ls failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.trim().is_empty() {
        return Ok(vec![]);
    }

    let volumes: Vec<DockerVolume> = stdout
        .lines()
        .filter(|line| !line.trim().is_empty())
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect();

    Ok(volumes)
}

/// Create a Docker volume
#[tauri::command]
pub async fn create_volume(name: String, driver: String) -> Result<String, String> {
    let mut args = vec!["volume", "create"];
    
    let driver_flag;
    if !driver.is_empty() && driver != "local" {
        driver_flag = driver.clone();
        args.push("--driver");
        args.push(&driver_flag);
    }
    args.push(&name);

    let output = docker_cmd()
        .args(&args)
        .output()
        .map_err(|e| format!("Failed to create volume: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "docker volume create failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(format!("Volume '{}' created", name))
}

/// Remove a Docker volume
#[tauri::command]
pub async fn remove_volume(name: String, force: bool) -> Result<String, String> {
    let mut args = vec!["volume", "rm"];
    if force {
        args.push("-f");
    }
    args.push(&name);

    let output = docker_cmd()
        .args(&args)
        .output()
        .map_err(|e| format!("Failed to remove volume: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "docker volume rm failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(format!("Volume '{}' removed", name))
}

/// Prune unused Docker volumes
#[tauri::command]
pub async fn prune_volumes() -> Result<String, String> {
    let output = docker_cmd()
        .args(["volume", "prune", "-f"])
        .output()
        .map_err(|e| format!("Failed to prune volumes: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "docker volume prune failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Inspect a Docker volume (raw JSON)
#[tauri::command]
pub async fn inspect_volume(name: String) -> Result<String, String> {
    let output = docker_cmd()
        .args(["volume", "inspect", &name])
        .output()
        .map_err(|e| format!("Failed to inspect volume: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "docker volume inspect failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}
