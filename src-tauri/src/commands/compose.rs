use serde::{Deserialize, Serialize};

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

/// List Docker Compose projects
#[tauri::command]
pub async fn list_compose_projects() -> Result<Vec<ComposeProject>, crate::error::ColimaError> {
    async move {
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
    .await.map_err(|e: String| crate::error::ColimaError::from(e))
}

/// Start a Docker Compose project
#[tauri::command]
pub async fn compose_up(project_dir: String, detach: bool) -> Result<String, crate::error::ColimaError> {
    async move {
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
    .await.map_err(|e: String| crate::error::ColimaError::from(e))
}

/// Stop a Docker Compose project
#[tauri::command]
pub async fn compose_down(project_name: String) -> Result<String, crate::error::ColimaError> {
    async move {
    let output = docker_output(vec!["compose".into(), "-p".into(), project_name.clone(), "down".into()]).await?;

    if !output.status.success() {
        return Err(format!(
            "docker compose down failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(format!("Compose project '{}' stopped", project_name))
    }
    .await.map_err(|e: String| crate::error::ColimaError::from(e))
}

/// Restart a Docker Compose project
#[tauri::command]
pub async fn compose_restart(project_name: String) -> Result<String, crate::error::ColimaError> {
    async move {
    let output = docker_output(vec!["compose".into(), "-p".into(), project_name.clone(), "restart".into()]).await?;

    if !output.status.success() {
        return Err(format!(
            "docker compose restart failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(format!("Compose project '{}' restarted", project_name))
    }
    .await.map_err(|e: String| crate::error::ColimaError::from(e))
}

/// Get compose project logs
#[tauri::command]
pub async fn compose_logs(project_name: String, lines: u32) -> Result<String, crate::error::ColimaError> {
    async move {
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
    .await.map_err(|e: String| crate::error::ColimaError::from(e))
}

/// List services in a compose project
#[tauri::command]
pub async fn compose_ps(project_name: String) -> Result<String, crate::error::ColimaError> {
    async move {
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
    .await.map_err(|e: String| crate::error::ColimaError::from(e))
}
