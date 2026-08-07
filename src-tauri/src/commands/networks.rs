use serde::{Deserialize, Serialize};

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


/// Run a Docker CLI command on a blocking thread pool with a 10s timeout.
async fn docker_output(args: Vec<String>) -> Result<std::process::Output, String> {
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        tokio::task::spawn_blocking(move || {
            crate::commands::runtime::get_runtime_cmd()
                .args(args.iter().map(|s| s.as_str()).collect::<Vec<&str>>())
                .output()
                .map_err(|e| format!("Failed to run docker command: {}", e))
        }),
    )
    .await;

    match result {
        Ok(join_result) => join_result.map_err(|e| format!("Task join error: {}", e))?,
        Err(_) => Err("Docker command timed out (daemon may be unresponsive)".to_string()),
    }
}

/// List all Docker networks
#[tauri::command]
pub async fn list_networks() -> Result<Vec<DockerNetwork>, crate::error::ColimaError> {
    async move {
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
    .await.map_err(|e: String| crate::error::ColimaError::from(e))
}

/// Create a Docker network
#[tauri::command]
pub async fn create_network(     name: String,     driver: String,     subnet: String, ) -> Result<String, crate::error::ColimaError> {
    async move {
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
    .await.map_err(|e: String| crate::error::ColimaError::from(e))
}

/// Remove a Docker network
#[tauri::command]
pub async fn remove_network(name: String) -> Result<String, crate::error::ColimaError> {
    async move {
    let output = docker_output(vec!["network".into(), "rm".into(), name.clone()]).await?;

    if !output.status.success() {
        return Err(format!(
            "docker network rm failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(format!("Network '{}' removed", name))
    }
    .await.map_err(|e: String| crate::error::ColimaError::from(e))
}

/// Inspect a Docker network (raw JSON)
#[tauri::command]
pub async fn inspect_network(name: String) -> Result<String, crate::error::ColimaError> {
    async move {
    let output = docker_output(vec!["network".into(), "inspect".into(), name]).await?;

    if !output.status.success() {
        return Err(format!(
            "docker network inspect failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
    .await.map_err(|e: String| crate::error::ColimaError::from(e))
}

/// Prune unused Docker networks
#[tauri::command]
pub async fn prune_networks() -> Result<String, crate::error::ColimaError> {
    async move {
    let output = docker_output(vec!["network".into(), "prune".into(), "-f".into()]).await?;

    if !output.status.success() {
        return Err(format!(
            "docker network prune failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
    .await.map_err(|e: String| crate::error::ColimaError::from(e))
}
