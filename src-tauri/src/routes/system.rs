use std::process::Command;
use axum::{
    extract::Query,
    http::StatusCode,
    response::Json,
};
use crate::api_server::*;
use crate::commands::system;
use crate::commands::containers;
use crate::routes::payloads::*;

pub async fn api_check_system() -> (StatusCode, Json<ApiResponse<system::SystemInfo>>) {
    let info = SYSTEM_INFO_CACHE
        .lock()
        .map(|mut cache| cache.get_or_init(load_system_info))
        .unwrap_or_else(|_| load_system_info());
    ok(info)
}


pub async fn api_get_version() -> (StatusCode, Json<ApiResponse<String>>) {
    let info = SYSTEM_INFO_CACHE
        .lock()
        .map(|mut cache| cache.get_or_init(load_system_info))
        .unwrap_or_else(|_| load_system_info());
    ok(info.colima_version)
}


pub async fn api_check_homebrew() -> (StatusCode, Json<ApiResponse<HomebrewStatus>>) {
    match run_blocking(|| {
        let output = Command::new("brew").arg("--version").output();
        match output {
            Ok(o) if o.status.success() => {
                let version = String::from_utf8_lossy(&o.stdout)
                    .lines()
                    .next()
                    .unwrap_or("")
                    .to_string();
                Ok(HomebrewStatus {
                    installed: true,
                    version,
                })
            }
            _ => Ok(HomebrewStatus {
                installed: false,
                version: String::new(),
            }),
        }
    })
    .await
    {
        Ok(status) => ok(status),
        Err(e) => err(e),
    }
}


pub async fn api_check_tool(Query(q): Query<ToolQuery>) -> (StatusCode, Json<ApiResponse<ToolStatus>>) {
    let name = q.name;
    // Whitelist of allowed tools to prevent arbitrary command execution
    let allowed = ["kubectl", "kind", "helm", "krunkit", "nerdctl"];
    if !allowed.contains(&name.as_str()) {
        return err(format!("Unknown tool: {}", name));
    }
    match run_blocking(move || {
        let output = Command::new(&name).arg("version").output();
        match output {
            Ok(o) if o.status.success() => {
                let version = String::from_utf8_lossy(&o.stdout)
                    .lines()
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_string();
                Ok(ToolStatus {
                    installed: true,
                    version,
                })
            }
            Ok(o) => {
                // Some tools use --version instead of version
                let output2 = Command::new(&name).arg("--version").output();
                match output2 {
                    Ok(o2) if o2.status.success() => {
                        let version = String::from_utf8_lossy(&o2.stdout)
                            .lines()
                            .next()
                            .unwrap_or("")
                            .trim()
                            .to_string();
                        Ok(ToolStatus {
                            installed: true,
                            version,
                        })
                    }
                    _ => {
                        // Binary exists but version command failed — still installed
                        let stderr = String::from_utf8_lossy(&o.stderr);
                        Ok(ToolStatus {
                            installed: true,
                            version: stderr.lines().next().unwrap_or("").trim().to_string(),
                        })
                    }
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(ToolStatus {
                installed: false,
                version: String::new(),
            }),
            Err(e) => Err(format!("Failed to check {}: {}", name, e)),
        }
    })
    .await
    {
        Ok(status) => ok(status),
        Err(e) => err(e),
    }
}


pub async fn api_get_platform() -> (StatusCode, Json<ApiResponse<PlatformInfo>>) {
    match run_blocking(|| Ok(detect_platform())).await {
        Ok(info) => ok(info),
        Err(e) => err(e),
    }
}


pub async fn api_host_specs() -> (StatusCode, Json<ApiResponse<system::HostSpecs>>) {
    match run_blocking(|| Ok(system::detect_host_specs())).await {
        Ok(specs) => ok(specs),
        Err(e) => err(e),
    }
}


pub async fn api_install_dep(
    Json(req): Json<InstallDepRequest>,
) -> (StatusCode, Json<ApiResponse<InstallResult>>) {
    let valid_names = ["colima", "docker", "lima"];
    if !valid_names.contains(&req.name.as_str()) {
        return err(format!("Invalid dependency name: {}", req.name));
    }

    match run_blocking(move || {
        // Map dep name to package name per method
        let pkg = match (req.method.as_str(), req.name.as_str()) {
            ("brew", name) => name.to_string(),
            ("apt", "docker") => "docker.io".to_string(),
            ("apt", name) => name.to_string(),
            ("nix", name) => name.to_string(),
            ("wsl-brew", name) => name.to_string(),
            ("manual", _) => {
                return Ok(InstallResult {
                    success: true,
                    output: "Manual installation: visit https://github.com/abiosoft/colima"
                        .to_string(),
                });
            }
            _ => return Err(format!("Unknown install method: {}", req.method)),
        };

        let output = match req.method.as_str() {
            "brew" => Command::new("brew")
                .args(["install", &pkg])
                .output()
                .map_err(|e| format!("brew install failed: {}", e))?,
            "apt" => Command::new("sudo")
                .args(["apt-get", "install", "-y", &pkg])
                .output()
                .map_err(|e| format!("apt install failed: {}", e))?,
            "nix" => {
                let nix_pkg = format!("nixpkgs.{}", pkg);
                Command::new("nix-env")
                    .args(["-iA", &nix_pkg])
                    .output()
                    .map_err(|e| format!("nix install failed: {}", e))?
            }
            "wsl-brew" => Command::new("wsl")
                .args(["-e", "brew", "install", &pkg])
                .output()
                .map_err(|e| format!("wsl brew install failed: {}", e))?,
            _ => return Err(format!("Unknown method: {}", req.method)),
        };

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if output.status.success() {
            Ok(InstallResult {
                success: true,
                output: if stdout.is_empty() { stderr } else { stdout },
            })
        } else {
            Ok(InstallResult {
                success: false,
                output: format!("Install failed: {}", stderr),
            })
        }
    })
    .await
    {
        Ok(result) => ok(result),
        Err(e) => err(e),
    }
}


/// Delegates to commands layer for system prune
pub async fn api_system_prune(Query(q): Query<PruneQuery>) -> (StatusCode, Json<ApiResponse<String>>) {
    let all = q.all.unwrap_or(false);
    match containers::system_prune(all).await {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}


/// Delegates to commands layer for system df
pub async fn api_system_df() -> (StatusCode, Json<ApiResponse<String>>) {
    match containers::system_df().await {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}


pub async fn api_docker_df() -> (StatusCode, Json<ApiResponse<String>>) {
    match containers::system_df().await {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}


pub async fn api_docker_prune(
    Query(q): Query<DockerPruneQuery>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    // Fix #15: Require explicit confirmation for destructive prune operation
    if !q.confirm {
        return err("This will remove all unused images, containers, networks, and volumes. Pass ?confirm=true to proceed.".to_string());
    }
    match containers::system_prune(true).await {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}


/// Returns the API auth token for browser mode clients.
/// Protected by CORS (localhost-only origins), not by Bearer auth.
pub async fn api_auth_token() -> (StatusCode, Json<ApiResponse<String>>) {
    ok(get_api_token())
}


/// Simple health check endpoint (unauthenticated).
pub async fn api_health() -> (StatusCode, Json<ApiResponse<String>>) {
    ok("ok".to_string())
}
