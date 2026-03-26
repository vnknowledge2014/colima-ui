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
    Command::new("docker")
}

/// List all Docker containers
#[tauri::command]
pub async fn list_containers(all: bool) -> Result<Vec<DockerContainer>, String> {
    let mut args = vec!["ps", "--format", "json", "--no-trunc"];
    if all {
        args.push("-a");
    }

    let output = docker_cmd()
        .args(&args)
        .output()
        .map_err(|e| format!("Failed to execute docker: {}", e))?;

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

    let containers: Vec<DockerContainer> = stdout
        .lines()
        .filter(|line| !line.trim().is_empty())
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect();

    Ok(containers)
}

/// Start a Docker container
#[tauri::command]
pub async fn start_container(container_id: String) -> Result<String, String> {
    let output = docker_cmd()
        .args(["start", &container_id])
        .output()
        .map_err(|e| format!("Failed to start container: {}", e))?;

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
    let output = docker_cmd()
        .args(["stop", &container_id])
        .output()
        .map_err(|e| format!("Failed to stop container: {}", e))?;

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
    let output = docker_cmd()
        .args(["restart", &container_id])
        .output()
        .map_err(|e| format!("Failed to restart container: {}", e))?;

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
    let mut args = vec!["rm"];
    if force {
        args.push("-f");
    }
    args.push(&container_id);

    let output = docker_cmd()
        .args(&args)
        .output()
        .map_err(|e| format!("Failed to remove container: {}", e))?;

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
    let output = docker_cmd()
        .args(["logs", "--tail", &tail, "--timestamps", &container_id])
        .output()
        .map_err(|e| format!("Failed to get logs: {}", e))?;

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
#[tauri::command]
pub async fn list_images() -> Result<Vec<DockerImage>, String> {
    let output = docker_cmd()
        .args(["images", "--format", "json"])
        .output()
        .map_err(|e| format!("Failed to list images: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "docker images failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.trim().is_empty() {
        return Ok(vec![]);
    }

    let images: Vec<DockerImage> = stdout
        .lines()
        .filter(|line| !line.trim().is_empty())
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect();

    Ok(images)
}

/// Inspect a container (raw JSON)
#[tauri::command]
pub async fn inspect_container(container_id: String) -> Result<String, String> {
    let output = docker_cmd()
        .args(["inspect", &container_id])
        .output()
        .map_err(|e| format!("Failed to inspect container: {}", e))?;

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
    let mut args = vec!["rmi"];
    if force {
        args.push("-f");
    }
    args.push(&image_id);

    let output = docker_cmd()
        .args(&args)
        .output()
        .map_err(|e| format!("Failed to remove image: {}", e))?;

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
    let output = docker_cmd()
        .args(["pull", &image_name])
        .output()
        .map_err(|e| format!("Failed to pull image: {}", e))?;

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
    let output = docker_cmd()
        .args(["image", "prune", "-a", "-f"])
        .output()
        .map_err(|e| format!("Failed to prune images: {}", e))?;

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
    let output = docker_cmd()
        .args(["image", "inspect", &image_id])
        .output()
        .map_err(|e| format!("Failed to inspect image: {}", e))?;

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
    let output = docker_cmd()
        .args(["tag", &source, &target])
        .output()
        .map_err(|e| format!("Failed to tag image: {}", e))?;

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
    let mut args = vec!["system", "prune", "-f"];
    if all {
        args.push("-a");
    }

    let output = docker_cmd()
        .args(&args)
        .output()
        .map_err(|e| format!("Failed to system prune: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "docker system prune failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Docker system disk usage
#[tauri::command]
pub async fn system_df() -> Result<String, String> {
    let output = docker_cmd()
        .args(["system", "df", "-v", "--format", "json"])
        .output()
        .map_err(|e| format!("Failed to get system df: {}", e))?;

    // system df may not support --format json on all versions
    if !output.status.success() {
        // Fallback to plain text
        let fallback = docker_cmd()
            .args(["system", "df", "-v"])
            .output()
            .map_err(|e| format!("Failed to get system df: {}", e))?;
        return Ok(String::from_utf8_lossy(&fallback.stdout).to_string());
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Get container stats (one-shot, no streaming)
#[tauri::command]
pub async fn container_stats(container_id: String) -> Result<String, String> {
    let output = docker_cmd()
        .args(["stats", "--no-stream", "--format", "json", &container_id])
        .output()
        .map_err(|e| format!("Failed to get stats: {}", e))?;

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
    let output = docker_cmd()
        .args(["stats", "--no-stream", "--format", "json"])
        .output()
        .map_err(|e| format!("Failed to get stats: {}", e))?;

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
    let output = docker_cmd()
        .args(["top", &container_id])
        .output()
        .map_err(|e| format!("Failed to get top: {}", e))?;

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
    let output = docker_cmd()
        .args(["exec", &container_id, "sh", "-c", &command])
        .output()
        .map_err(|e| format!("Failed to exec: {}", e))?;

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

    let args_ref: Vec<&str> = args.iter().map(|s| s.as_str()).collect();

    let output = docker_cmd()
        .args(&args_ref)
        .output()
        .map_err(|e| format!("Failed to run container: {}", e))?;

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
    let output = docker_cmd()
        .args(["rename", &container_id, &new_name])
        .output()
        .map_err(|e| format!("Failed to rename container: {}", e))?;

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
    let output = docker_cmd()
        .args(["pause", &container_id])
        .output()
        .map_err(|e| format!("Failed to pause container: {}", e))?;

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
    let output = docker_cmd()
        .args(["unpause", &container_id])
        .output()
        .map_err(|e| format!("Failed to unpause container: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "docker unpause failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(format!("Container {} unpaused", container_id))
}
