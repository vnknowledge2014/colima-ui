use serde::{Deserialize, Serialize};
use std::process::Command;

/// Docker network info
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DockerNetwork {
    #[serde(alias = "ID", alias = "Id")]
    pub id: String,
    #[serde(alias = "Name")]
    pub name: String,
    #[serde(alias = "Driver")]
    pub driver: String,
    #[serde(alias = "Scope")]
    pub scope: String,
    #[serde(default, alias = "IPv6")]
    pub ipv6: String,
    #[serde(default, alias = "Internal")]
    pub internal: String,
    #[serde(default, alias = "Labels")]
    pub labels: String,
}

fn docker_cmd() -> Command {
    Command::new("docker")
}

/// List all Docker networks
#[tauri::command]
pub async fn list_networks() -> Result<Vec<DockerNetwork>, String> {
    let output = docker_cmd()
        .args(["network", "ls", "--format", "json", "--no-trunc"])
        .output()
        .map_err(|e| format!("Failed to list networks: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "docker network ls failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.trim().is_empty() {
        return Ok(vec![]);
    }

    let networks: Vec<DockerNetwork> = stdout
        .lines()
        .filter(|line| !line.trim().is_empty())
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect();

    Ok(networks)
}

/// Create a Docker network
#[tauri::command]
pub async fn create_network(
    name: String,
    driver: String,
    subnet: String,
) -> Result<String, String> {
    let mut args = vec!["network", "create"];

    let driver_flag;
    if !driver.is_empty() {
        driver_flag = driver.clone();
        args.push("--driver");
        args.push(&driver_flag);
    }

    let subnet_flag;
    if !subnet.is_empty() {
        subnet_flag = subnet.clone();
        args.push("--subnet");
        args.push(&subnet_flag);
    }

    args.push(&name);

    let output = docker_cmd()
        .args(&args)
        .output()
        .map_err(|e| format!("Failed to create network: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "docker network create failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(format!("Network '{}' created", name))
}

/// Remove a Docker network
#[tauri::command]
pub async fn remove_network(name: String) -> Result<String, String> {
    let output = docker_cmd()
        .args(["network", "rm", &name])
        .output()
        .map_err(|e| format!("Failed to remove network: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "docker network rm failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(format!("Network '{}' removed", name))
}

/// Inspect a Docker network (raw JSON)
#[tauri::command]
pub async fn inspect_network(name: String) -> Result<String, String> {
    let output = docker_cmd()
        .args(["network", "inspect", &name])
        .output()
        .map_err(|e| format!("Failed to inspect network: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "docker network inspect failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Prune unused Docker networks
#[tauri::command]
pub async fn prune_networks() -> Result<String, String> {
    let output = docker_cmd()
        .args(["network", "prune", "-f"])
        .output()
        .map_err(|e| format!("Failed to prune networks: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "docker network prune failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}
