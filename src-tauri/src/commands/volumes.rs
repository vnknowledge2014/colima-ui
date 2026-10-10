use serde::{Deserialize, Serialize};

use crate::commands::activity;
use crate::commands::runtime;

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


/// List all Docker volumes
#[tauri::command]
pub async fn list_volumes() -> Result<Vec<DockerVolume>, crate::error::ColimaError> {
    async move {
    let output = runtime::run(vec!["volume".into(), "ls".into(), "--format".into(), "json".into()], runtime::DEFAULT_TIMEOUT).await?;

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
    .await.map_err(|e: String| crate::error::ColimaError::from(e))
}

/// Create a Docker volume
#[tauri::command]
pub async fn create_volume(name: String, driver: String) -> Result<String, crate::error::ColimaError> {
    if !crate::validation::is_valid_resource_name(&name) {
        return Err(crate::error::ColimaError::validation(format!("Invalid name: {:?}", name)));
    }
    async move {
    let mut args = vec!["volume".to_string(), "create".to_string()];

    if !driver.is_empty() && driver != "local" {
        args.push("--driver".to_string());
        args.push(driver);
    }
    args.push(name.clone());

    let output = runtime::run(args, runtime::DEFAULT_TIMEOUT).await?;

    if !output.status.success() {
        return Err(format!(
            "docker volume create failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(format!("Volume '{}' created", name))
    }
    .await.map_err(|e: String| crate::error::ColimaError::from(e))
}

/// Remove a Docker volume
#[tauri::command]
pub async fn remove_volume(name: String, force: bool) -> Result<String, crate::error::ColimaError> {
    if !crate::validation::is_valid_resource_name(&name) {
        return Err(crate::error::ColimaError::validation(format!("Invalid name: {:?}", name)));
    }
    // The block below takes ownership, so what the record needs is kept here.
    let logged_name = name.clone();
    let result = async move {
    let mut args = vec!["volume".to_string(), "rm".to_string()];
    if force {
        args.push("-f".to_string());
    }
    args.push(name.clone());

    let output = runtime::run(args, runtime::DEFAULT_TIMEOUT).await?;

    if !output.status.success() {
        return Err(format!(
            "docker volume rm failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(format!("Volume '{}' removed", name))
    }
    .await;

    crate::commands::activity::record(
        activity::ActivityEntry::new(activity::ActivityKind::Destructive, "remove", "volume", &logged_name)
            .outcome_of(&result),
    );

    result.map_err(|e: String| crate::error::ColimaError::from(e))
}

/// Prune unused Docker volumes
#[tauri::command]
pub async fn prune_volumes() -> Result<String, crate::error::ColimaError> {
    let result = async move {
    let output = runtime::run(vec!["volume".into(), "prune".into(), "-f".into()], runtime::DEFAULT_TIMEOUT).await?;

    if !output.status.success() {
        return Err(format!(
            "docker volume prune failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
    .await;

    crate::commands::activity::record(
        activity::ActivityEntry::new(activity::ActivityKind::Destructive, "prune", "volume", "")
            .detail(result.as_deref().map(activity::prune_summary).unwrap_or_default())
            .outcome_of(&result),
    );

    result.map_err(|e: String| crate::error::ColimaError::from(e))
}

/// Inspect a Docker volume (raw JSON)
#[tauri::command]
pub async fn inspect_volume(name: String) -> Result<String, crate::error::ColimaError> {
    if !crate::validation::is_valid_resource_name(&name) {
        return Err(crate::error::ColimaError::validation(format!("Invalid name: {:?}", name)));
    }
    async move {
    let output = runtime::run(vec!["volume".into(), "inspect".into(), name], runtime::DEFAULT_TIMEOUT).await?;

    if !output.status.success() {
        return Err(format!(
            "docker volume inspect failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
    .await.map_err(|e: String| crate::error::ColimaError::from(e))
}
