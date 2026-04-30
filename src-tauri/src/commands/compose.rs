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
    Command::new("docker")
}

/// List Docker Compose projects
#[tauri::command]
pub async fn list_compose_projects() -> Result<Vec<ComposeProject>, String> {
    let output = docker_cmd()
        .args(["compose", "ls", "--format", "json", "-a"])
        .output()
        .map_err(|e| format!("Failed to list compose projects: {}", e))?;

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
    let projects: Vec<ComposeProject> = serde_json::from_str(stdout.trim())
        .unwrap_or_else(|_| {
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
    let mut args = vec!["compose"];
    if !project_dir.is_empty() {
        args.push("-f");
        args.push(&project_dir);
    }
    args.push("up");
    if detach {
        args.push("-d");
    }

    let output = docker_cmd()
        .args(&args)
        .output()
        .map_err(|e| format!("Failed to compose up: {}", e))?;

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
    let output = docker_cmd()
        .args(["compose", "-p", &project_name, "down"])
        .output()
        .map_err(|e| format!("Failed to compose down: {}", e))?;

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
    let output = docker_cmd()
        .args(["compose", "-p", &project_name, "restart"])
        .output()
        .map_err(|e| format!("Failed to compose restart: {}", e))?;

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
    let output = docker_cmd()
        .args(["compose", "-p", &project_name, "logs", "--tail", &tail, "--no-color"])
        .output()
        .map_err(|e| format!("Failed to get compose logs: {}", e))?;

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
    let output = docker_cmd()
        .args(["compose", "-p", &project_name, "ps", "--format", "json"])
        .output()
        .map_err(|e| format!("Failed to list compose services: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "docker compose ps failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}
