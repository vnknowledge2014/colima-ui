use serde::{Deserialize, Serialize};
use std::process::Command;

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

fn docker_cmd() -> Command {
    let mut cmd = Command::new("docker");
    if let Some(host) = crate::path_util::detect_docker_host() {
        cmd.env("DOCKER_HOST", host);
    }
    cmd
}

/// Run a Docker CLI command on a blocking thread pool with a timeout.
/// All Docker CLI calls MUST use this instead of calling .output() directly,
/// because .output() blocks the current thread and starves the Tokio runtime
/// when multiple commands are issued concurrently (e.g. list_containers +
/// list_images + list_volumes + list_networks all fired at once from the UI).
/// Timeout: 10 seconds. If Docker daemon is frozen, we return Err instead of
/// hanging forever (which causes permanent loading spinners in the UI).
async fn docker_output(args: Vec<String>) -> Result<std::process::Output, String> {
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        tokio::task::spawn_blocking(move || {
            docker_cmd()
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

/// List all Docker containers
/// Always fetches fresh data: tries Bollard first (fast), falls back to Docker CLI.
/// Returns Err if Docker daemon is unavailable.
#[tauri::command]
pub async fn list_containers(
    state: tauri::State<'_, std::sync::Arc<tokio::sync::RwLock<crate::docker_state::DockerState>>>,
    all: bool,
) -> Result<Vec<serde_json::Value>, String> {
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
    let output = docker_output(vec!["ps".into(), "--format".into(), "json".into(), "--no-trunc".into(), "-a".into()]).await?;

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

/// Start a Docker container
#[tauri::command]
pub async fn start_container(container_id: String) -> Result<String, String> {
    let output = docker_output(vec!["start".into(), container_id.clone()]).await?;

    if !output.status.success() {
        return Err(format!(
            "docker start failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(format!("Container {} started", container_id))
}

/// Stop a Docker container
#[tauri::command]
pub async fn stop_container(container_id: String) -> Result<String, String> {
    let output = docker_output(vec!["stop".into(), container_id.clone()]).await?;

    if !output.status.success() {
        return Err(format!(
            "docker stop failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(format!("Container {} stopped", container_id))
}

/// Restart a Docker container
#[tauri::command]
pub async fn restart_container(container_id: String) -> Result<String, String> {
    let output = docker_output(vec!["restart".into(), container_id.clone()]).await?;

    if !output.status.success() {
        return Err(format!(
            "docker restart failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(format!("Container {} restarted", container_id))
}

/// Remove a Docker container
#[tauri::command]
pub async fn remove_container(container_id: String, force: bool) -> Result<String, String> {
    let mut args = vec!["rm".to_string()];
    if force {
        args.push("-f".to_string());
    }
    args.push(container_id.clone());

    let output = docker_output(args).await?;

    if !output.status.success() {
        return Err(format!(
            "docker rm failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(format!("Container {} removed", container_id))
}

/// Get container logs (last N lines)
#[tauri::command]
pub async fn container_logs(container_id: String, lines: u32) -> Result<String, String> {
    let tail = lines.to_string();
    let output = docker_output(vec!["logs".into(), "--tail".into(), tail, "--timestamps".into(), container_id]).await?;

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

/// List Docker images
/// Always fetches fresh data: tries Bollard first (fast), falls back to Docker CLI.
/// Returns Err if Docker daemon is unavailable.
#[tauri::command]
pub async fn list_images(
    state: tauri::State<'_, std::sync::Arc<tokio::sync::RwLock<crate::docker_state::DockerState>>>,
) -> Result<Vec<serde_json::Value>, String> {
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
    let output = docker_output(vec!["images".into(), "--format".into(), "json".into(), "--no-trunc".into()]).await?;

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

/// Inspect a container (raw JSON)
#[tauri::command]
pub async fn inspect_container(container_id: String) -> Result<String, String> {
    let output = docker_output(vec!["inspect".into(), container_id]).await?;

    if !output.status.success() {
        return Err(format!(
            "docker inspect failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Remove a Docker image
#[tauri::command]
pub async fn remove_image(image_id: String, force: bool) -> Result<String, String> {
    let mut args = vec!["rmi".to_string()];
    if force {
        args.push("-f".to_string());
    }
    args.push(image_id.clone());

    let output = docker_output(args).await?;

    if !output.status.success() {
        return Err(format!(
            "docker rmi failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(format!("Image {} removed", image_id))
}

/// Pull a Docker image
#[tauri::command]
pub async fn pull_image(image_name: String) -> Result<String, String> {
    let output = docker_output(vec!["pull".into(), image_name]).await?;

    if !output.status.success() {
        return Err(format!(
            "docker pull failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Prune unused Docker images
#[tauri::command]
pub async fn prune_images() -> Result<String, String> {
    let output = docker_output(vec!["image".into(), "prune".into(), "-a".into(), "-f".into()]).await?;

    if !output.status.success() {
        return Err(format!(
            "docker image prune failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Inspect a Docker image (raw JSON)
#[tauri::command]
pub async fn inspect_image(image_id: String) -> Result<String, String> {
    let output = docker_output(vec!["image".into(), "inspect".into(), image_id]).await?;

    if !output.status.success() {
        return Err(format!(
            "docker image inspect failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Tag a Docker image
#[tauri::command]
pub async fn tag_image(source: String, target: String) -> Result<String, String> {
    let output = docker_output(vec!["tag".into(), source, target.clone()]).await?;

    if !output.status.success() {
        return Err(format!(
            "docker tag failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(format!("Image tagged as {}", target))
}

/// Docker system prune (containers, images, networks, build cache)
#[tauri::command]
pub async fn system_prune(all: bool) -> Result<String, String> {
    let mut args = vec!["system".to_string(), "prune".to_string(), "-f".to_string()];
    if all {
        args.push("-a".to_string());
    }

    let output = docker_output(args).await?;

    if !output.status.success() {
        return Err(format!(
            "docker system prune failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Docker system disk usage (plain text for frontend parsing)
#[tauri::command]
pub async fn system_df() -> Result<String, String> {
    let output = docker_output(vec!["system".into(), "df".into()]).await?;

    if !output.status.success() {
        return Err(format!(
            "docker system df failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Get container stats (one-shot, no streaming)
#[tauri::command]
pub async fn container_stats(container_id: String) -> Result<String, String> {
    let output = docker_output(vec!["stats".into(), "--no-stream".into(), "--format".into(), "json".into(), container_id]).await?;

    if !output.status.success() {
        return Err(format!(
            "docker stats failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Get all container stats (one-shot)
#[tauri::command]
pub async fn all_container_stats() -> Result<String, String> {
    let output = docker_output(vec!["stats".into(), "--no-stream".into(), "--format".into(), "json".into()]).await?;

    if !output.status.success() {
        return Err(format!(
            "docker stats failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Get running processes inside a container
#[tauri::command]
pub async fn container_top(container_id: String) -> Result<String, String> {
    let output = docker_output(vec!["top".into(), container_id]).await?;

    if !output.status.success() {
        return Err(format!(
            "docker top failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Execute a command inside a running container
#[tauri::command]
pub async fn container_exec(container_id: String, command: String) -> Result<String, String> {
    let output = docker_output(vec!["exec".into(), container_id, "sh".into(), "-c".into(), command]).await?;

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

/// Run a new container from an image
#[tauri::command]
pub async fn run_container(
    image: String,
    name: String,
    ports: Vec<String>,
    env_vars: Vec<String>,
    volumes: Vec<String>,
    detach: bool,
    remove_on_exit: bool,
    extra_args: Vec<String>,
) -> Result<String, String> {
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

    let output = docker_output(args).await?;

    if !output.status.success() {
        return Err(format!(
            "docker run failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Rename a container
#[tauri::command]
pub async fn rename_container(container_id: String, new_name: String) -> Result<String, String> {
    let output = docker_output(vec!["rename".into(), container_id, new_name.clone()]).await?;

    if !output.status.success() {
        return Err(format!(
            "docker rename failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(format!("Container renamed to {}", new_name))
}

/// Pause a container
#[tauri::command]
pub async fn pause_container(container_id: String) -> Result<String, String> {
    let output = docker_output(vec!["pause".into(), container_id.clone()]).await?;

    if !output.status.success() {
        return Err(format!(
            "docker pause failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(format!("Container {} paused", container_id))
}

/// Unpause a container
#[tauri::command]
pub async fn unpause_container(container_id: String) -> Result<String, String> {
    let output = docker_output(vec!["unpause".into(), container_id.clone()]).await?;

    if !output.status.success() {
        return Err(format!(
            "docker unpause failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(format!("Container {} unpaused", container_id))
}
