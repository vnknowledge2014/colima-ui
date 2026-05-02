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
    let mut cmd = Command::new("docker");
    if let Some(host) = crate::path_util::detect_docker_host() {
        cmd.env("DOCKER_HOST", host);
    }
    cmd
}

/// Run a Docker CLI command on a blocking thread pool to avoid starving Tokio.
async fn docker_output(args: Vec<String>) -> Result<std::process::Output, String> {
    tokio::task::spawn_blocking(move || {
        docker_cmd()
            .args(args.iter().map(|s| s.as_str()).collect::<Vec<&str>>())
            .output()
            .map_err(|e| format!("Failed to run docker command: {}", e))
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}

/// List all Docker networks
#[tauri::command]
pub async fn list_networks() -> Result<Vec<DockerNetwork>, String> {
    let output = docker_output(vec!["network".into(), "ls".into(), "--format".into(), "json".into(), "--no-trunc".into()]).await?;

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
    let mut args = vec!["network".to_string(), "create".to_string()];

    if !driver.is_empty() {
        args.push("--driver".to_string());
        args.push(driver);
    }

    if !subnet.is_empty() {
        args.push("--subnet".to_string());
        args.push(subnet);
    }

    args.push(name.clone());

    let output = docker_output(args).await?;

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
    let output = docker_output(vec!["network".into(), "rm".into(), name.clone()]).await?;

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
    let output = docker_output(vec!["network".into(), "inspect".into(), name]).await?;

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
    let output = docker_output(vec!["network".into(), "prune".into(), "-f".into()]).await?;

    if !output.status.success() {
        return Err(format!(
            "docker network prune failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}
