use serde::Serialize;
use std::process::Command;
use crate::path_util;

#[derive(Debug, Clone, Serialize)]
pub struct SystemInfo {
    pub colima_installed: bool,
    pub colima_version: String,
    pub docker_installed: bool,
    pub docker_version: String,
    pub lima_installed: bool,
    pub lima_version: String,
}

fn get_version(cmd: &str, args: &[&str]) -> Option<String> {
    let resolved = path_util::resolve_binary(cmd);
    let mut command = Command::new(&resolved);
    command.args(args);
    path_util::apply_path_to_cmd(&mut command);
    command
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

/// Check system prerequisites
/// Runs on a blocking thread with a 5s timeout to prevent app startup hangs
/// when Docker/Colima daemons are unresponsive.
#[tauri::command]
pub async fn check_system() -> Result<SystemInfo, String> {
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        tokio::task::spawn_blocking(|| {
            let colima_version = get_version("colima", &["version"]);
            // Use --version (not context-dependent) to avoid Docker daemon connection
            let docker_version = get_version("docker", &["--version"]);
            let lima_version = get_version("limactl", &["--version"]);

            SystemInfo {
                colima_installed: colima_version.is_some(),
                colima_version: colima_version.unwrap_or_default(),
                docker_installed: docker_version.is_some(),
                docker_version: docker_version.unwrap_or_default(),
                lima_installed: lima_version.is_some(),
                lima_version: lima_version.unwrap_or_default(),
            }
        }),
    )
    .await;

    match result {
        Ok(Ok(info)) => Ok(info),
        Ok(Err(e)) => Err(format!("Task join error: {}", e)),
        Err(_) => {
            // Timeout — return partial info so the app doesn't freeze
            Ok(SystemInfo {
                colima_installed: false,
                colima_version: String::new(),
                docker_installed: false,
                docker_version: String::new(),
                lima_installed: false,
                lima_version: String::new(),
            })
        }
    }
}

/// Get Colima version string
#[tauri::command]
pub async fn get_colima_version() -> Result<String, String> {
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        tokio::task::spawn_blocking(|| {
            Command::new("colima")
                .args(["version"])
                .output()
                .map_err(|e| format!("Failed to get colima version: {}", e))
        }),
    )
    .await;

    match result {
        Ok(Ok(Ok(output))) => Ok(String::from_utf8_lossy(&output.stdout).trim().to_string()),
        Ok(Ok(Err(e))) => Err(e),
        Ok(Err(e)) => Err(format!("Task join error: {}", e)),
        Err(_) => Err("Colima version check timed out".to_string()),
    }
}

/// Check if an optional tool is installed
#[tauri::command]
pub async fn check_tool(name: String) -> Result<serde_json::Value, String> {
    let allowed = ["kubectl", "kind", "helm", "krunkit", "nerdctl"];
    if !allowed.contains(&name.as_str()) {
        return Err(format!("Unknown tool: {}", name));
    }
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        tokio::task::spawn_blocking(move || {
            let version = get_version(&name, &["version"])
                .or_else(|| get_version(&name, &["--version"]));
            serde_json::json!({
                "installed": version.is_some(),
                "version": version.unwrap_or_default()
            })
        }),
    )
    .await;

    match result {
        Ok(Ok(json)) => Ok(json),
        Ok(Err(e)) => Err(format!("Task join error: {}", e)),
        Err(_) => Ok(serde_json::json!({ "installed": false, "version": "" })),
    }
}
