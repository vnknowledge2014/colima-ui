use serde::{Deserialize, Serialize};
use std::process::Command;

/// Docker Compose project info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComposeProject {
    #[serde(alias = "Name")]
    pub name: String,
    #[serde(alias = "Status")]
    pub status: String,
    #[serde(alias = "ConfigFiles", default)]
    pub config_files: String,
}

fn docker_cmd() -> Command {
    let mut cmd = Command::new("docker");
    if let Some(host) = crate::path_util::detect_docker_host() {
        cmd.env("DOCKER_HOST", host);
    }
    cmd
}

/// Run a Docker CLI command on a blocking thread pool with a 10s timeout.
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

/// List Docker Compose projects
#[tauri::command]
pub async fn list_compose_projects() -> Result<Vec<ComposeProject>, String> {
    let output = docker_output(vec!["compose".into(), "ls".into(), "--format".into(), "json".into(), "-a".into()]).await?;

    if !output.status.success() {
        return Err(format!(
            "docker compose ls failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.trim().is_empty() {
        return Ok(vec![]);
    }

    // docker compose ls --format json returns a JSON array
    let projects: Vec<ComposeProject> = serde_json::from_str(stdout.trim()).unwrap_or_else(|_| {
        // Fallback: try line-by-line JSON
        stdout
            .lines()
            .filter(|l| !l.trim().is_empty())
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect()
    });

    Ok(projects)
}

/// Start a Docker Compose project
#[tauri::command]
pub async fn compose_up(project_dir: String, detach: bool) -> Result<String, String> {
    let mut args = vec!["compose".to_string()];
    if !project_dir.is_empty() {
        args.push("-f".to_string());
        args.push(project_dir);
    }
    args.push("up".to_string());
    if detach {
        args.push("-d".to_string());
    }

    let output = docker_output(args).await?;

    if !output.status.success() {
        return Err(format!(
            "docker compose up failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok("Compose project started".to_string())
}

/// Stop a Docker Compose project
#[tauri::command]
pub async fn compose_down(project_name: String) -> Result<String, String> {
    let output = docker_output(vec!["compose".into(), "-p".into(), project_name.clone(), "down".into()]).await?;

    if !output.status.success() {
        return Err(format!(
            "docker compose down failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(format!("Compose project '{}' stopped", project_name))
}

/// Restart a Docker Compose project
#[tauri::command]
pub async fn compose_restart(project_name: String) -> Result<String, String> {
    let output = docker_output(vec!["compose".into(), "-p".into(), project_name.clone(), "restart".into()]).await?;

    if !output.status.success() {
        return Err(format!(
            "docker compose restart failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(format!("Compose project '{}' restarted", project_name))
}

/// Get compose project logs
#[tauri::command]
pub async fn compose_logs(project_name: String, lines: u32) -> Result<String, String> {
    let tail = lines.to_string();
    let output = docker_output(vec![
        "compose".into(), "-p".into(), project_name, "logs".into(),
        "--tail".into(), tail, "--no-color".into(),
    ]).await?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    let combined = if stdout.is_empty() {
        stderr.to_string()
    } else {
        stdout.to_string()
    };

    Ok(combined)
}

/// List services in a compose project
#[tauri::command]
pub async fn compose_ps(project_name: String) -> Result<String, String> {
    let output = docker_output(vec![
        "compose".into(), "-p".into(), project_name, "ps".into(), "--format".into(), "json".into(),
    ]).await?;

    if !output.status.success() {
        return Err(format!(
            "docker compose ps failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}
