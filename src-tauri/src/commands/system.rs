use serde::Serialize;
use std::process::Command;

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
    Command::new(cmd)
        .args(args)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

/// Check system prerequisites
#[tauri::command]
pub async fn check_system() -> Result<SystemInfo, String> {
    let colima_version = get_version("colima", &["version"]);
    let docker_version = get_version("docker", &["--version"]);
    let lima_version = get_version("limactl", &["--version"]);

    Ok(SystemInfo {
        colima_installed: colima_version.is_some(),
        colima_version: colima_version.unwrap_or_default(),
        docker_installed: docker_version.is_some(),
        docker_version: docker_version.unwrap_or_default(),
        lima_installed: lima_version.is_some(),
        lima_version: lima_version.unwrap_or_default(),
    })
}

/// Get Colima version string
#[tauri::command]
pub async fn get_colima_version() -> Result<String, String> {
    let output = Command::new("colima")
        .args(["version"])
        .output()
        .map_err(|e| format!("Failed to get colima version: {}", e))?;

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}
