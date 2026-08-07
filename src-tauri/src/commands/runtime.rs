use std::process::Command;

/// Detect the active container runtime name and profile.
/// Returns (runtime_name, profile_name, docker_host_socket).
pub fn detect_runtime() -> (String, String, Option<String>) {
    let mut runtime_name = "docker".to_string();
    let mut profile_name = "default".to_string();
    let mut host_sock = None;

    if let Some((host, profile)) = crate::path_util::detect_docker_host() {
        host_sock = Some(host);
        profile_name = profile;
    }

    // Determine runtime from colima status output
    let status_output = Command::new(crate::path_util::resolve_binary("colima"))
        .args(["status", "-p", &profile_name])
        .output();

    if let Ok(output) = status_output {
        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.contains("runtime: podman") {
            runtime_name = "podman".to_string();
        } else if stdout.contains("runtime: containerd") {
            runtime_name = "containerd".to_string();
        }
    }

    (runtime_name, profile_name, host_sock)
}

/// Gets the appropriate CLI command (`docker`, `podman`, or `nerdctl`)
/// based on the current active Colima profile's runtime.
pub fn get_runtime_cmd() -> Command {
    let (runtime_name, profile_name, host_sock) = detect_runtime();

    let mut cmd;
    if runtime_name == "containerd" {
        // Colima provides `colima nerdctl` wrapper which correctly executes inside the VM
        cmd = Command::new(crate::path_util::resolve_binary("colima"));
        cmd.args(["-p", &profile_name, "nerdctl"]);
    } else {
        let bin = if runtime_name == "containerd" { "nerdctl" } else { &runtime_name };
        cmd = Command::new(crate::path_util::resolve_binary(bin));
        if let Some(host) = host_sock {
            // For podman and docker, setting DOCKER_HOST works natively on macOS
            cmd.env("DOCKER_HOST", host);
        }
    }

    crate::path_util::apply_path_to_cmd(&mut cmd);
    cmd
}

/// Returns the active runtime name (e.g., "docker", "podman", "containerd").
#[tauri::command]
pub async fn get_runtime_info() -> Result<String, crate::error::ColimaError> {
    async move {
    let (runtime_name, _, _) = detect_runtime();
    Ok(runtime_name)
    }
    .await.map_err(|e: String| crate::error::ColimaError::from(e))
}
