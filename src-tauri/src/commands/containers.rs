use serde::{Deserialize, Serialize};

use crate::commands::activity;
use crate::commands::runtime;

/// Docker container info from `docker ps --format json`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DockerContainer {
    #[serde(alias = "ID", alias = "Id")]
    pub id: String,
    #[serde(alias = "Names")]
    pub names: String,
    #[serde(alias = "Image")]
    pub image: String,
    #[serde(alias = "Status")]
    pub status: String,
    #[serde(alias = "State")]
    pub state: String,
    #[serde(alias = "Ports")]
    pub ports: String,
    #[serde(default, alias = "CreatedAt")]
    pub created_at: String,
    #[serde(default, alias = "Size")]
    pub size: String,
    #[serde(default, alias = "Command")]
    pub command: String,
}

/// Docker image info
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DockerImage {
    #[serde(alias = "ID", alias = "Id")]
    pub id: String,
    #[serde(alias = "Repository")]
    pub repository: String,
    #[serde(alias = "Tag")]
    pub tag: String,
    #[serde(alias = "Size")]
    pub size: String,
    #[serde(default, alias = "CreatedAt")]
    pub created_at: String,
}


/// List containers via CLI only (no Bollard, no Tauri state required).
/// Used by HTTP route handlers which don't have access to Tauri managed state.
pub async fn list_containers_cli(all: bool) -> Result<Vec<serde_json::Value>, String> {
    let mut args = vec!["ps".into(), "--format".into(), "json".into(), "--no-trunc".into()];
    if all {
        args.push("-a".into());
    }

    let output = runtime::run(args, runtime::DEFAULT_TIMEOUT).await?;

    if !output.status.success() {
        return Err(format!(
            "docker ps failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.trim().is_empty() {
        return Ok(vec![]);
    }

    Ok(stdout
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .map(normalize_cli_labels)
        .collect())
}

/// Convert the CLI's comma-separated `Labels` string into the object shape that
/// `docker_state::map_containers` produces from the Bollard API.
///
/// `docker ps --format json` renders labels as `"a=1,b=2"` while Bollard returns
/// a map. Without this the same container has two different shapes depending on
/// which path served the request, and compose grouping would work through one
/// and not the other.
fn normalize_cli_labels(mut value: serde_json::Value) -> serde_json::Value {
    let raw = match value.get("Labels").and_then(|l| l.as_str()) {
        Some(s) => s.to_string(),
        // Already an object, or absent — leave a consistent empty object.
        None => {
            if value.get("Labels").is_none() {
                if let Some(obj) = value.as_object_mut() {
                    obj.insert("Labels".into(), serde_json::json!({}));
                }
            }
            return value;
        }
    };

    let mut labels = serde_json::Map::new();
    for pair in raw.split(',') {
        let pair = pair.trim();
        if pair.is_empty() {
            continue;
        }
        // Label values may themselves contain '=', so split only on the first.
        match pair.split_once('=') {
            Some((k, v)) => {
                labels.insert(k.to_string(), serde_json::Value::String(v.to_string()));
            }
            None => {
                labels.insert(pair.to_string(), serde_json::Value::String(String::new()));
            }
        }
    }

    if let Some(obj) = value.as_object_mut() {
        obj.insert("Labels".into(), serde_json::Value::Object(labels));
    }
    value
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_cli_label_string_into_an_object() {
        let input = serde_json::json!({
            "Id": "abc",
            "Labels": "com.docker.compose.project=web,com.docker.compose.service=api"
        });
        let out = normalize_cli_labels(input);
        assert_eq!(out["Labels"]["com.docker.compose.project"], "web");
        assert_eq!(out["Labels"]["com.docker.compose.service"], "api");
    }

    #[test]
    fn keeps_equals_signs_inside_label_values() {
        let out = normalize_cli_labels(serde_json::json!({ "Labels": "cmd=a=b=c" }));
        assert_eq!(out["Labels"]["cmd"], "a=b=c");
    }

    #[test]
    fn produces_an_empty_object_when_there_are_no_labels() {
        // Both the empty-string and the absent case, so the frontend never has
        // to check for null.
        for input in [
            serde_json::json!({ "Id": "abc", "Labels": "" }),
            serde_json::json!({ "Id": "abc" }),
        ] {
            let out = normalize_cli_labels(input);
            assert!(out["Labels"].is_object(), "expected an object: {out}");
            assert_eq!(out["Labels"].as_object().unwrap().len(), 0);
        }
    }

    #[test]
    fn leaves_an_existing_object_untouched() {
        let input = serde_json::json!({ "Labels": { "a": "1" } });
        let out = normalize_cli_labels(input);
        assert_eq!(out["Labels"]["a"], "1");
    }

    #[test]
    fn tolerates_a_label_with_no_value() {
        let out = normalize_cli_labels(serde_json::json!({ "Labels": "flag,other=2" }));
        assert_eq!(out["Labels"]["flag"], "");
        assert_eq!(out["Labels"]["other"], "2");
    }
}

/// List all Docker containers
/// Always fetches fresh data: tries Bollard first (fast), falls back to Docker CLI.
/// Returns Err if Docker daemon is unavailable.
#[tauri::command]
pub async fn list_containers(     state: tauri::State<'_, std::sync::Arc<tokio::sync::RwLock<crate::docker_state::DockerState>>>,     all: bool, ) -> Result<Vec<serde_json::Value>, crate::error::ColimaError> {
    async move {
    // Try Bollard SDK first (faster — uses Docker socket directly)
    let mut bollard_error: Option<String> = None;
    {
        let lock = state.read().await;
        if let Some(docker) = &lock.docker {
            match tokio::time::timeout(
                std::time::Duration::from_secs(5),
                docker.list_containers(Some(bollard::container::ListContainersOptions::<String> {
                    all,
                    ..Default::default()
                })),
            )
            .await
            {
                Ok(Ok(containers)) => {
                    return Ok(crate::docker_state::map_containers(&containers));
                }
                Ok(Err(e)) => {
                    bollard_error = Some(format!("{}", e));
                }
                Err(_) => {
                    bollard_error = Some("Bollard timed out".to_string());
                }
            }
        }
    }

    // Fallback to Docker CLI (on blocking thread pool)
    let output = runtime::run(vec!["ps".into(), "--format".into(), "json".into(), "--no-trunc".into(), "-a".into()], runtime::DEFAULT_TIMEOUT).await?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let mapped: Vec<serde_json::Value> = stdout
            .lines()
            .filter(|l| !l.trim().is_empty())
            .filter_map(|l| serde_json::from_str(l).ok())
            .map(|c: serde_json::Value| {
                serde_json::json!({
                    "Id": c.get("ID").or(c.get("Id")).unwrap_or(&serde_json::Value::String(String::new())),
                    "Names": c.get("Names").or(c.get("names")).unwrap_or(&serde_json::Value::String(String::new())),
                    "Image": c.get("Image").or(c.get("image")).unwrap_or(&serde_json::Value::String(String::new())),
                    "Status": c.get("Status").or(c.get("status")).unwrap_or(&serde_json::Value::String(String::new())),
                    "State": c.get("State").or(c.get("state")).unwrap_or(&serde_json::Value::String(String::new())),
                    "Ports": c.get("Ports").or(c.get("ports")).unwrap_or(&serde_json::Value::String(String::new())),
                    "CreatedAt": c.get("CreatedAt").or(c.get("created_at")).unwrap_or(&serde_json::Value::String(String::new())),
                    "Size": c.get("Size").or(c.get("size")).unwrap_or(&serde_json::Value::String(String::new())),
                    "Command": c.get("Command").or(c.get("command")).unwrap_or(&serde_json::Value::String(String::new())),
                })
            })
            .collect();
        if all {
            Ok(mapped)
        } else {
            Ok(mapped.iter().filter(|c| c["State"] == "running").cloned().collect())
        }
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(bollard_error.unwrap_or_else(|| format!("Docker is not available: {}", stderr.trim())))
    }
    }
    .await.map_err(|e: String| crate::error::ColimaError::from(e))
}

/// Start a Docker container
#[tauri::command]
pub async fn start_container(container_id: String) -> Result<String, crate::error::ColimaError> {
    // The block below takes ownership, so what the record needs is kept here.
    let logged_container_id = container_id.clone();
    let result = async move {
    let output = runtime::run(vec!["start".into(), container_id.clone()], runtime::DEFAULT_TIMEOUT).await?;

    if !output.status.success() {
        return Err(format!(
            "docker start failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(format!("Container {} started", container_id))
    }
    .await;

    crate::commands::activity::record(
        activity::ActivityEntry::new(activity::ActivityKind::Lifecycle, "start", "container", &logged_container_id)
            .outcome_of(&result),
    );

    result.map_err(|e: String| crate::error::ColimaError::from(e))
}

/// Stop a Docker container
#[tauri::command]
pub async fn stop_container(container_id: String) -> Result<String, crate::error::ColimaError> {
    // The block below takes ownership, so what the record needs is kept here.
    let logged_container_id = container_id.clone();
    let result = async move {
    let output = runtime::run(vec!["stop".into(), container_id.clone()], runtime::DEFAULT_TIMEOUT).await?;

    if !output.status.success() {
        return Err(format!(
            "docker stop failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(format!("Container {} stopped", container_id))
    }
    .await;

    crate::commands::activity::record(
        activity::ActivityEntry::new(activity::ActivityKind::Lifecycle, "stop", "container", &logged_container_id)
            .outcome_of(&result),
    );

    result.map_err(|e: String| crate::error::ColimaError::from(e))
}

/// Restart a Docker container
#[tauri::command]
pub async fn restart_container(container_id: String) -> Result<String, crate::error::ColimaError> {
    // The block below takes ownership, so what the record needs is kept here.
    let logged_container_id = container_id.clone();
    let result = async move {
    let output = runtime::run(vec!["restart".into(), container_id.clone()], runtime::DEFAULT_TIMEOUT).await?;

    if !output.status.success() {
        return Err(format!(
            "docker restart failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(format!("Container {} restarted", container_id))
    }
    .await;

    crate::commands::activity::record(
        activity::ActivityEntry::new(activity::ActivityKind::Lifecycle, "restart", "container", &logged_container_id)
            .outcome_of(&result),
    );

    result.map_err(|e: String| crate::error::ColimaError::from(e))
}

/// Remove a Docker container
#[tauri::command]
pub async fn remove_container(container_id: String, force: bool) -> Result<String, crate::error::ColimaError> {
    // The block below takes ownership, so the id the record needs is kept here.
    let logged_container_id = container_id.clone();
    let result = async move {
    let mut args = vec!["rm".to_string()];
    if force {
        args.push("-f".to_string());
    }
    args.push(container_id.clone());

    let output = runtime::run(args, runtime::DEFAULT_TIMEOUT).await?;

    if !output.status.success() {
        return Err(format!(
            "docker rm failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(format!("Container {} removed", container_id))
    }
    .await;

    crate::commands::activity::record(
        activity::ActivityEntry::new(activity::ActivityKind::Destructive, "remove", "container", &logged_container_id)
            .outcome_of(&result),
    );

    result.map_err(|e: String| crate::error::ColimaError::from(e))
}

/// Get container logs (last N lines)
#[tauri::command]
pub async fn container_logs(container_id: String, lines: u32) -> Result<String, crate::error::ColimaError> {
    async move {
    let tail = lines.to_string();
    let output = runtime::run(vec!["logs".into(), "--tail".into(), tail, "--timestamps".into(), container_id], runtime::DEFAULT_TIMEOUT).await?;

    if !output.status.success() {
        return Err(format!(
            "docker logs failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    // Docker logs may output to both stdout and stderr
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = if stdout.is_empty() {
        stderr.to_string()
    } else if stderr.is_empty() {
        stdout.to_string()
    } else {
        format!("{}\n{}", stdout, stderr)
    };

    Ok(combined)
    }
    .await.map_err(|e: String| crate::error::ColimaError::from(e))
}

/// List Docker images
/// Always fetches fresh data: tries Bollard first (fast), falls back to Docker CLI.
/// Returns Err if Docker daemon is unavailable.
#[tauri::command]
pub async fn list_images(     state: tauri::State<'_, std::sync::Arc<tokio::sync::RwLock<crate::docker_state::DockerState>>>, ) -> Result<Vec<serde_json::Value>, crate::error::ColimaError> {
    async move {
    // Try Bollard SDK first (faster — uses Docker socket directly)
    let mut bollard_error: Option<String> = None;
    {
        let lock = state.read().await;
        if let Some(docker) = &lock.docker {
            match tokio::time::timeout(
                std::time::Duration::from_secs(5),
                docker.list_images(Some(bollard::image::ListImagesOptions::<String> {
                    all: false,
                    ..Default::default()
                })),
            )
            .await
            {
                Ok(Ok(images)) => {
                    return Ok(crate::docker_state::map_images(&images));
                }
                Ok(Err(e)) => {
                    bollard_error = Some(format!("{}", e));
                }
                Err(_) => {
                    bollard_error = Some("Bollard timed out".to_string());
                }
            }
        }
    }

    // Fallback to Docker CLI (on blocking thread pool)
    let output = runtime::run(vec!["images".into(), "--format".into(), "json".into(), "--no-trunc".into()], runtime::DEFAULT_TIMEOUT).await?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let mapped: Vec<serde_json::Value> = stdout
            .lines()
            .filter(|l| !l.trim().is_empty())
            .filter_map(|l| serde_json::from_str(l).ok())
            .map(|img: serde_json::Value| {
                serde_json::json!({
                    "Id": img.get("ID").or(img.get("Id")).unwrap_or(&serde_json::Value::String(String::new())),
                    "Repository": img.get("Repository").or(img.get("repository")).unwrap_or(&serde_json::Value::String(String::new())),
                    "Tag": img.get("Tag").or(img.get("tag")).unwrap_or(&serde_json::Value::String(String::new())),
                    "Size": img.get("Size").or(img.get("size")).unwrap_or(&serde_json::Value::String(String::new())),
                    "CreatedAt": img.get("CreatedAt").or(img.get("CreatedSince")).or(img.get("created_at")).unwrap_or(&serde_json::Value::String(String::new())),
                })
            })
            .collect();
        Ok(mapped)
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(bollard_error.unwrap_or_else(|| format!("Docker is not available: {}", stderr.trim())))
    }
    }
    .await.map_err(|e: String| crate::error::ColimaError::from(e))
}

/// Inspect a container (raw JSON)
#[tauri::command]
pub async fn inspect_container(container_id: String) -> Result<String, crate::error::ColimaError> {
    async move {
    let output = runtime::run(vec!["inspect".into(), container_id], runtime::DEFAULT_TIMEOUT).await?;

    if !output.status.success() {
        return Err(format!(
            "docker inspect failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
    .await.map_err(|e: String| crate::error::ColimaError::from(e))
}

/// Remove a Docker image
#[tauri::command]
pub async fn remove_image(image_id: String, force: bool) -> Result<String, crate::error::ColimaError> {
    // The block below takes ownership, so the id the record needs is kept here.
    let logged_image_id = image_id.clone();
    let result = async move {
    let mut args = vec!["rmi".to_string()];
    if force {
        args.push("-f".to_string());
    }
    args.push(image_id.clone());

    let output = runtime::run(args, runtime::DEFAULT_TIMEOUT).await?;

    if !output.status.success() {
        return Err(format!(
            "docker rmi failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(format!("Image {} removed", image_id))
    }
    .await;

    crate::commands::activity::record(
        activity::ActivityEntry::new(activity::ActivityKind::Destructive, "remove", "image", &logged_image_id)
            .outcome_of(&result),
    );

    result.map_err(|e: String| crate::error::ColimaError::from(e))
}

/// Pull a Docker image
#[tauri::command]
pub async fn pull_image(image_name: String) -> Result<String, crate::error::ColimaError> {
    // The block below takes ownership, so what the record needs is kept here.
    let logged_image_name = image_name.clone();
    let started = std::time::Instant::now();
    let result = async move {
    let output = runtime::run(vec!["pull".into(), image_name], runtime::DEFAULT_TIMEOUT).await?;

    if !output.status.success() {
        return Err(format!(
            "docker pull failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
    .await;

    crate::commands::activity::record(
        activity::ActivityEntry::new(activity::ActivityKind::Task, "pull", "image", &logged_image_name)
            .took(started.elapsed().as_millis() as i64)
            .outcome_of(&result),
    );

    result.map_err(|e: String| crate::error::ColimaError::from(e))
}

/// Prune unused Docker images
#[tauri::command]
pub async fn prune_images() -> Result<String, crate::error::ColimaError> {
    let result = async move {
    let output = runtime::run(vec!["image".into(), "prune".into(), "-a".into(), "-f".into()], runtime::DEFAULT_TIMEOUT).await?;

    if !output.status.success() {
        return Err(format!(
            "docker image prune failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
    .await;

    crate::commands::activity::record(
        activity::ActivityEntry::new(activity::ActivityKind::Destructive, "prune", "image", "")
            .detail(result.as_deref().map(activity::prune_summary).unwrap_or_default())
            .outcome_of(&result),
    );

    result.map_err(|e: String| crate::error::ColimaError::from(e))
}

/// Inspect a Docker image (raw JSON)
#[tauri::command]
pub async fn inspect_image(image_id: String) -> Result<String, crate::error::ColimaError> {
    async move {
    let output = runtime::run(vec!["image".into(), "inspect".into(), image_id], runtime::DEFAULT_TIMEOUT).await?;

    if !output.status.success() {
        return Err(format!(
            "docker image inspect failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
    .await.map_err(|e: String| crate::error::ColimaError::from(e))
}

/// Tag a Docker image
#[tauri::command]
pub async fn tag_image(source: String, target: String) -> Result<String, crate::error::ColimaError> {
    async move {
    let output = runtime::run(vec!["tag".into(), source, target.clone()], runtime::DEFAULT_TIMEOUT).await?;

    if !output.status.success() {
        return Err(format!(
            "docker tag failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(format!("Image tagged as {}", target))
    }
    .await.map_err(|e: String| crate::error::ColimaError::from(e))
}

/// Docker system prune (containers, images, networks, build cache)
#[tauri::command]
pub async fn system_prune(all: bool) -> Result<String, crate::error::ColimaError> {
    let result = async move {
    let mut args = vec!["system".to_string(), "prune".to_string(), "-f".to_string()];
    if all {
        args.push("-a".to_string());
    }

    let output = runtime::run(args, runtime::DEFAULT_TIMEOUT).await?;

    if !output.status.success() {
        return Err(format!(
            "docker system prune failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
    .await;

    crate::commands::activity::record(
        activity::ActivityEntry::new(activity::ActivityKind::Destructive, "prune", "system", "")
            .detail(result.as_deref().map(activity::prune_summary).unwrap_or_default())
            .outcome_of(&result),
    );

    result.map_err(|e: String| crate::error::ColimaError::from(e))
}

/// Docker system disk usage (plain text for frontend parsing)
#[tauri::command]
pub async fn system_df() -> Result<String, crate::error::ColimaError> {
    async move {
    let output = runtime::run(vec!["system".into(), "df".into()], runtime::DEFAULT_TIMEOUT).await?;

    if !output.status.success() {
        return Err(format!(
            "docker system df failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
    .await.map_err(|e: String| crate::error::ColimaError::from(e))
}

/// Get container stats (one-shot, no streaming)
#[tauri::command]
pub async fn container_stats(container_id: String) -> Result<String, crate::error::ColimaError> {
    async move {
    let output = runtime::run(vec!["stats".into(), "--no-stream".into(), "--format".into(), "json".into(), container_id], runtime::DEFAULT_TIMEOUT).await?;

    if !output.status.success() {
        return Err(format!(
            "docker stats failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
    .await.map_err(|e: String| crate::error::ColimaError::from(e))
}

/// Get all container stats (one-shot)
#[tauri::command]
pub async fn all_container_stats() -> Result<String, crate::error::ColimaError> {
    async move {
    let output = runtime::run(vec!["stats".into(), "--no-stream".into(), "--format".into(), "json".into()], runtime::DEFAULT_TIMEOUT).await?;

    if !output.status.success() {
        return Err(format!(
            "docker stats failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
    .await.map_err(|e: String| crate::error::ColimaError::from(e))
}

/// Get running processes inside a container
#[tauri::command]
pub async fn container_top(container_id: String) -> Result<String, crate::error::ColimaError> {
    async move {
    let output = runtime::run(vec!["top".into(), container_id], runtime::DEFAULT_TIMEOUT).await?;

    if !output.status.success() {
        return Err(format!(
            "docker top failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
    .await.map_err(|e: String| crate::error::ColimaError::from(e))
}

/// Execute a command inside a running container
#[tauri::command]
pub async fn container_exec(container_id: String, command: String) -> Result<String, crate::error::ColimaError> {
    async move {
    // Security validation (applies to both Tauri IPC and HTTP routes)
    if !crate::validation::is_valid_container_id(&container_id) {
        return Err("Invalid container ID format".to_string());
    }
    if crate::validation::contains_shell_injection(&command) {
        return Err("Command contains forbidden characters (shell injection blocked)".to_string());
    }

    let output = runtime::run(vec!["exec".into(), container_id, "sh".into(), "-c".into(), command], runtime::DEFAULT_TIMEOUT).await?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if !output.status.success() {
        if !stderr.is_empty() {
            return Err(format!("exec failed: {}", stderr));
        }
        return Err("exec failed with no output".to_string());
    }

    let combined = if stdout.is_empty() {
        stderr.to_string()
    } else {
        stdout.to_string()
    };

    Ok(combined)
    }
    .await.map_err(|e: String| crate::error::ColimaError::from(e))
}

/// Run a new container from an image
#[tauri::command]
#[allow(clippy::too_many_arguments)] // All 8 params map 1:1 to the frontend IPC contract; grouping them would break the Tauri command signature
pub async fn run_container(     image: String,     name: String,     ports: Vec<String>,     env_vars: Vec<String>,     volumes: Vec<String>,     detach: bool,     remove_on_exit: bool,     extra_args: Vec<String>, ) -> Result<String, crate::error::ColimaError> {
    // The block below takes ownership, so what the record needs is kept here.
    let logged_image = image.clone();
    let logged_name = name.clone();
    // The count, never the values: `env_vars` is exactly where a password
    // would be, and a count answers "was it configured" without carrying one.
    let env_count = env_vars.len();
    let port_count = ports.len();
    let result = async move {
    // Security validation (applies to both Tauri IPC and HTTP routes)
    for arg in &extra_args {
        let arg_lower = arg.to_lowercase();
        for banned in crate::validation::BANNED_DOCKER_FLAGS {
            if arg_lower.starts_with(&banned.to_lowercase()) {
                return Err(format!(
                    "Flag '{}' is not allowed for security reasons",
                    arg
                ));
            }
        }
        // Block bind mounts to host root
        if (arg_lower.contains("source=/,") || arg_lower.contains("source=/\","))
            && arg_lower.contains("type=bind")
        {
            return Err("Bind-mounting host root '/' is not allowed".to_string());
        }
    }
    // Validate volume mounts for host root bind
    for v in &volumes {
        if v.starts_with("/:") {
            return Err("Bind-mounting host root '/' is not allowed".to_string());
        }
    }

    let mut args = vec!["run".to_string()];

    if detach {
        args.push("-d".to_string());
    }
    if remove_on_exit {
        args.push("--rm".to_string());
    }
    if !name.is_empty() {
        args.push("--name".to_string());
        args.push(name);
    }
    for port in &ports {
        args.push("-p".to_string());
        args.push(port.clone());
    }
    for env in &env_vars {
        args.push("-e".to_string());
        args.push(env.clone());
    }
    for vol in &volumes {
        args.push("-v".to_string());
        args.push(vol.clone());
    }
    for extra in &extra_args {
        args.push(extra.clone());
    }
    args.push(image);

    let output = runtime::run(args, runtime::DEFAULT_TIMEOUT).await?;

    if !output.status.success() {
        return Err(format!(
            "docker run failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }
    .await;

    crate::commands::activity::record(
        activity::ActivityEntry::new(activity::ActivityKind::Lifecycle, "run", "container", &logged_image)
            .named(&logged_name)
            .detail(format!("from {logged_image}, {port_count} ports, {env_count} env vars"))
            .outcome_of(&result),
    );

    result.map_err(|e: String| crate::error::ColimaError::from(e))
}

/// Rename a container
#[tauri::command]
pub async fn rename_container(container_id: String, new_name: String) -> Result<String, crate::error::ColimaError> {
    // The block below takes ownership, so what the record needs is kept here.
    let logged_container_id = container_id.clone();
    let logged_new_name = new_name.clone();
    let result = async move {
    let output = runtime::run(vec!["rename".into(), container_id, new_name.clone()], runtime::DEFAULT_TIMEOUT).await?;

    if !output.status.success() {
        return Err(format!(
            "docker rename failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(format!("Container renamed to {}", new_name))
    }
    .await;

    crate::commands::activity::record(
        activity::ActivityEntry::new(activity::ActivityKind::Lifecycle, "rename", "container", &logged_container_id)
            .named(&logged_new_name)
            .outcome_of(&result),
    );

    result.map_err(|e: String| crate::error::ColimaError::from(e))
}

/// Pause a container
#[tauri::command]
pub async fn pause_container(container_id: String) -> Result<String, crate::error::ColimaError> {
    // The block below takes ownership, so what the record needs is kept here.
    let logged_container_id = container_id.clone();
    let result = async move {
    let output = runtime::run(vec!["pause".into(), container_id.clone()], runtime::DEFAULT_TIMEOUT).await?;

    if !output.status.success() {
        return Err(format!(
            "docker pause failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(format!("Container {} paused", container_id))
    }
    .await;

    crate::commands::activity::record(
        activity::ActivityEntry::new(activity::ActivityKind::Lifecycle, "pause", "container", &logged_container_id)
            .outcome_of(&result),
    );

    result.map_err(|e: String| crate::error::ColimaError::from(e))
}

/// Unpause a container
#[tauri::command]
pub async fn unpause_container(container_id: String) -> Result<String, crate::error::ColimaError> {
    // The block below takes ownership, so what the record needs is kept here.
    let logged_container_id = container_id.clone();
    let result = async move {
    let output = runtime::run(vec!["unpause".into(), container_id.clone()], runtime::DEFAULT_TIMEOUT).await?;

    if !output.status.success() {
        return Err(format!(
            "docker unpause failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(format!("Container {} unpaused", container_id))
    }
    .await;

    crate::commands::activity::record(
        activity::ActivityEntry::new(activity::ActivityKind::Lifecycle, "unpause", "container", &logged_container_id)
            .outcome_of(&result),
    );

    result.map_err(|e: String| crate::error::ColimaError::from(e))
}

/// Diagnostic command to debug Docker connectivity issues from inside the app.
/// Reports: resolved paths, socket detection, DOCKER_HOST, PATH, and test results.
#[tauri::command]
pub async fn docker_diagnose(     state: tauri::State<'_, std::sync::Arc<tokio::sync::RwLock<crate::docker_state::DockerState>>>, ) -> Result<serde_json::Value, crate::error::ColimaError> {
    async move {
    let docker_path = crate::path_util::resolve_binary("docker");
    let colima_path = crate::path_util::resolve_binary("colima");
    let docker_host = crate::path_util::detect_docker_host();
    let env_path = std::env::var("PATH").unwrap_or_default();
    let home = std::env::var("HOME").unwrap_or_default();

    // Check socket existence
    let socket_path = docker_host.as_ref().map(|h| h.0.trim_start_matches("unix://").to_string());
    let socket_exists = socket_path
        .as_ref()
        .is_some_and(|p| std::path::Path::new(p).exists());

    // Check Bollard state
    let bollard_connected = {
        let lock = state.read().await;
        lock.docker.is_some()
    };

    // Test docker CLI with timeout
    let cli_test = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        tokio::task::spawn_blocking(move || {
            let output = crate::commands::runtime::get_runtime_cmd()
                .args(["version", "--format", "{{.Server.Version}}"])
                .output();
            match output {
                Ok(o) => {
                    let stdout = String::from_utf8_lossy(&o.stdout).trim().to_string();
                    let stderr = String::from_utf8_lossy(&o.stderr).trim().to_string();
                    serde_json::json!({
                        "success": o.status.success(),
                        "exit_code": o.status.code(),
                        "stdout": stdout,
                        "stderr": stderr,
                    })
                }
                Err(e) => serde_json::json!({
                    "success": false,
                    "error": format!("{}", e),
                }),
            }
        }),
    )
    .await;

    let cli_result = match cli_test {
        Ok(Ok(v)) => v,
        Ok(Err(e)) => serde_json::json!({"error": format!("join error: {}", e)}),
        Err(_) => serde_json::json!({"error": "timed out after 5s"}),
    };

    Ok(serde_json::json!({
        "docker_binary": docker_path,
        "colima_binary": colima_path,
        "docker_host": docker_host,
        "socket_path": socket_path,
        "socket_exists": socket_exists,
        "bollard_connected": bollard_connected,
        "home": home,
        "path": env_path,
        "cli_test": cli_result,
    }))
    }
    .await.map_err(|e: String| crate::error::ColimaError::from(e))
}
