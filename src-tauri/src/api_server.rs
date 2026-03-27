use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{Json, sse::{Event, Sse}},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::process::Command;
use std::sync::{Mutex, OnceLock};
use tower_http::cors::{Any, CorsLayer};
use tokio::sync::broadcast;
use futures_util::stream::StreamExt;
use std::convert::Infallible;

use crate::commands::{colima, docker, models, networks, system, volumes};
use crate::instance_reader;
use crate::terminal_session::{self, SharedSessionManager};

// ===== SSE Broadcast Infrastructure =====

static SSE_TX: OnceLock<broadcast::Sender<SseMessage>> = OnceLock::new();

#[derive(Clone, Debug)]
struct SseMessage {
    event: String,
    data: String,
}

fn get_sse_tx() -> broadcast::Sender<SseMessage> {
    SSE_TX.get_or_init(|| {
        let (tx, _) = broadcast::channel(64);
        tx
    }).clone()
}

/// Publish an event to all connected SSE browser clients
pub fn publish_sse_event(event_type: &str, data: &serde_json::Value) {
    let tx = get_sse_tx();
    let _ = tx.send(SseMessage {
        event: event_type.to_string(),
        data: data.to_string(),
    });
}

async fn api_events() -> Sse<impl futures_util::Stream<Item = Result<Event, Infallible>>> {
    let rx = get_sse_tx().subscribe();
    let stream = tokio_stream::wrappers::BroadcastStream::new(rx)
        .filter_map(|msg| async move {
            match msg {
                Ok(m) => Some(Ok(Event::default().event(m.event).data(m.data))),
                Err(_) => None,
            }
        });
    Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(std::time::Duration::from_secs(15))
    )
}

/// Generic API response wrapper
#[derive(Serialize)]
struct ApiResponse<T: Serialize> {
    success: bool,
    data: Option<T>,
    error: Option<String>,
}

fn ok<T: Serialize>(data: T) -> (StatusCode, Json<ApiResponse<T>>) {
    (
        StatusCode::OK,
        Json(ApiResponse {
            success: true,
            data: Some(data),
            error: None,
        }),
    )
}

fn err<T: Serialize>(msg: String) -> (StatusCode, Json<ApiResponse<T>>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ApiResponse {
            success: false,
            data: None,
            error: Some(msg),
        }),
    )
}

/// Run a blocking closure on the thread pool to avoid starving the tokio reactor.
/// All colima/docker/system commands use std::process::Command::output() which blocks.
async fn run_blocking<T, F>(f: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, String> + Send + 'static,
{
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| format!("Task join error: {}", e))?
}

// ===== Helper to run a command and return stdout =====

/// Find the docker socket from the first running Colima instance.
/// Checks all profile dirs under ~/.colima/ for an active lima instance,
/// then returns the socket path for that profile.
fn detect_docker_host() -> Option<String> {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    let colima_home = std::env::var("COLIMA_HOME").unwrap_or_else(|_| format!("{}/.colima", home));
    let colima_path = std::path::Path::new(&colima_home);

    if !colima_path.exists() {
        return None;
    }

    // Scan profiles for a running instance (has ha.sock in _lima dir)
    if let Ok(entries) = std::fs::read_dir(colima_path) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('_') || name.starts_with('.') || !entry.path().is_dir() {
                continue;
            }

            // Map profile to lima instance name
            let lima_name = if name == "default" {
                "colima".to_string()
            } else {
                format!("colima-{}", name)
            };
            let lima_dir = colima_path.join("_lima").join(&lima_name);

            if lima_dir.join("ha.sock").exists() || lima_dir.join("ha.pid").exists() {
                // Found running instance — return its docker socket
                let sock = colima_path.join(&name).join("docker.sock");
                if sock.exists() {
                    return Some(format!("unix://{}", sock.display()));
                }
            }
        }
    }

    // Fallback: check the colima-level docker.sock symlink
    let fallback = colima_path.join("docker.sock");
    if fallback.exists() {
        return Some(format!("unix://{}", fallback.display()));
    }

    None
}

fn run_cmd(program: &str, args: &[&str]) -> Result<String, String> {
    let mut cmd = Command::new(program);
    cmd.args(args);

    // Auto-set DOCKER_HOST for docker/docker-compose/kind commands
    if program == "docker" || program == "docker-compose" || program == "kind" {
        if let Some(host) = detect_docker_host() {
            cmd.env("DOCKER_HOST", host);
        }
    }

    let output = cmd.output().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            let hint = match program {
                "kind" => "Install with: brew install kind",
                "kubectl" => "Install with: brew install kubectl",
                "helm" => "Install with: brew install helm",
                "limactl" => "Install with: brew install lima",
                "colima" => "Install with: brew install colima",
                "docker" => "Install with: brew install docker",
                _ => "Please install it and try again",
            };
            format!("'{}' is not installed. {}", program, hint)
        } else {
            format!("Failed to execute {}: {}", program, e)
        }
    })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("{} failed: {}", program, stderr));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Create a docker Command with DOCKER_HOST auto-detected from running Colima instance.
fn docker_cmd() -> Command {
    let mut cmd = Command::new("docker");
    if let Some(host) = detect_docker_host() {
        cmd.env("DOCKER_HOST", host);
    }
    cmd
}

// ===== System routes =====

/// Cached system info — loaded once on first request, then returned instantly.
static SYSTEM_INFO_CACHE: OnceLock<system::SystemInfo> = OnceLock::new();

fn load_system_info() -> system::SystemInfo {
    let colima_version = run_cmd("colima", &["version"]).unwrap_or_default();
    let docker_version = run_cmd("docker", &["--version"]).unwrap_or_default();
    let lima_version = run_cmd("limactl", &["--version"]).unwrap_or_default();

    system::SystemInfo {
        colima_installed: !colima_version.is_empty(),
        colima_version: colima_version.lines().next().unwrap_or("").to_string(),
        docker_installed: !docker_version.is_empty(),
        docker_version: docker_version.trim().to_string(),
        lima_installed: !lima_version.is_empty(),
        lima_version: lima_version.trim().to_string(),
    }
}

async fn api_check_system() -> (StatusCode, Json<ApiResponse<system::SystemInfo>>) {
    // First call: load from CLI (slow but only once). Subsequent: instant from cache.
    let info = SYSTEM_INFO_CACHE.get_or_init(|| load_system_info());
    ok(info.clone())
}

async fn api_get_version() -> (StatusCode, Json<ApiResponse<String>>) {
    let info = SYSTEM_INFO_CACHE.get_or_init(|| load_system_info());
    ok(info.colima_version.clone())
}

#[derive(Serialize)]
struct HomebrewStatus {
    installed: bool,
    version: String,
}

async fn api_check_homebrew() -> (StatusCode, Json<ApiResponse<HomebrewStatus>>) {
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

#[derive(Deserialize)]
struct ToolQuery {
    name: String,
}

#[derive(Serialize)]
struct ToolStatus {
    installed: bool,
    version: String,
}

async fn api_check_tool(Query(q): Query<ToolQuery>) -> (StatusCode, Json<ApiResponse<ToolStatus>>) {
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
                Ok(ToolStatus { installed: true, version })
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
                        Ok(ToolStatus { installed: true, version })
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
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                Ok(ToolStatus { installed: false, version: String::new() })
            }
            Err(e) => Err(format!("Failed to check {}: {}", name, e)),
        }
    })
    .await
    {
        Ok(status) => ok(status),
        Err(e) => err(e),
    }
}

// ===== Platform detection =====

#[derive(Serialize)]
struct PackageManagerInfo {
    name: String,
    available: bool,
    version: String,
}

#[derive(Serialize)]
struct PlatformInfo {
    os: String,
    arch: String,
    wsl: bool,
    wsl_available: bool,
    package_managers: Vec<PackageManagerInfo>,
}

fn check_cmd_version(cmd: &str, args: &[&str]) -> (bool, String) {
    match Command::new(cmd).args(args).output() {
        Ok(o) if o.status.success() => {
            let ver = String::from_utf8_lossy(&o.stdout)
                .lines()
                .next()
                .unwrap_or("")
                .to_string();
            (true, ver)
        }
        _ => (false, String::new()),
    }
}

fn detect_platform() -> PlatformInfo {
    let os = std::env::consts::OS.to_string(); // "macos", "linux", "windows"
    let arch = std::env::consts::ARCH.to_string(); // "x86_64", "aarch64"

    // WSL detection: check /proc/version for microsoft/WSL or env var
    let wsl = if cfg!(target_os = "linux") {
        std::env::var("WSL_DISTRO_NAME").is_ok()
            || std::fs::read_to_string("/proc/version")
                .unwrap_or_default()
                .to_lowercase()
                .contains("microsoft")
    } else {
        false
    };

    // On Windows, check if WSL is available
    let wsl_available = if cfg!(target_os = "windows") {
        Command::new("wsl")
            .arg("--list")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    } else {
        false
    };

    // Detect package managers
    let mut pms = Vec::new();

    // Homebrew (macOS + Linux)
    let (brew_ok, brew_ver) = check_cmd_version("brew", &["--version"]);
    pms.push(PackageManagerInfo {
        name: "brew".to_string(),
        available: brew_ok,
        version: brew_ver,
    });

    // apt (Linux / WSL)
    if cfg!(target_os = "linux") || wsl {
        let (apt_ok, apt_ver) = check_cmd_version("apt", &["--version"]);
        pms.push(PackageManagerInfo {
            name: "apt".to_string(),
            available: apt_ok,
            version: apt_ver,
        });
    }

    // nix (any platform)
    let (nix_ok, nix_ver) = check_cmd_version("nix", &["--version"]);
    pms.push(PackageManagerInfo {
        name: "nix".to_string(),
        available: nix_ok,
        version: nix_ver,
    });

    // Always offer manual
    pms.push(PackageManagerInfo {
        name: "manual".to_string(),
        available: true,
        version: String::new(),
    });

    PlatformInfo {
        os,
        arch,
        wsl,
        wsl_available,
        package_managers: pms,
    }
}

async fn api_get_platform() -> (StatusCode, Json<ApiResponse<PlatformInfo>>) {
    match run_blocking(|| Ok(detect_platform())).await {
        Ok(info) => ok(info),
        Err(e) => err(e),
    }
}

// ===== Install dependency with method =====

#[derive(Deserialize)]
struct InstallDepRequest {
    name: String,
    #[serde(default = "default_install_method")]
    method: String,
}

fn default_install_method() -> String {
    "brew".to_string()
}

#[derive(Serialize)]
struct InstallResult {
    success: bool,
    output: String,
}

async fn api_install_dep(
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

// ===== Colima routes =====

async fn api_list_instances() -> (StatusCode, Json<ApiResponse<Vec<colima::ColimaInstance>>>) {
    // Direct filesystem read — instant (<1ms) vs CLI (30-60s)
    let instances = instance_reader::list_instances_fast();
    ok(instances)
}

#[derive(Deserialize)]
struct ProfileQuery {
    #[serde(default = "default_profile")]
    profile: String,
    #[serde(default)]
    force: bool,
}

fn default_profile() -> String {
    "default".to_string()
}

async fn api_stop_instance(
    Query(q): Query<ProfileQuery>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    let profile = q.profile;
    let force = q.force;
    match run_blocking(move || {
        let mut args = vec!["stop"];
        let profile_flag;
        if profile != "default" && !profile.is_empty() {
            profile_flag = profile.clone();
            args.push("--profile");
            args.push(&profile_flag);
        }
        if force {
            args.push("--force");
        }
        let output = Command::new("colima")
            .args(&args)
            .output()
            .map_err(|e| format!("Failed to stop colima: {}", e))?;

        if !output.status.success() {
            return Err(format!(
                "colima stop failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        Ok(format!("Instance '{}' stopped", profile))
    })
    .await
    {
        Ok(msg) => ok(msg),
        Err(e) => err(e),
    }
}

async fn api_delete_instance(
    Query(q): Query<ProfileQuery>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    let profile = q.profile;
    let force = q.force;
    match run_blocking(move || {
        let mut args = vec!["delete"];
        let profile_flag;
        if profile != "default" && !profile.is_empty() {
            profile_flag = profile.clone();
            args.push("--profile");
            args.push(&profile_flag);
        }
        if force {
            args.push("--force");
        }
        let output = Command::new("colima")
            .args(&args)
            .output()
            .map_err(|e| format!("Failed to delete colima: {}", e))?;

        if !output.status.success() {
            return Err(format!(
                "colima delete failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        Ok(format!("Instance '{}' deleted", profile))
    })
    .await
    {
        Ok(msg) => ok(msg),
        Err(e) => err(e),
    }
}

async fn api_instance_status(
    Query(q): Query<ProfileQuery>,
) -> (StatusCode, Json<ApiResponse<colima::InstanceStatus>>) {
    let profile = q.profile;
    match run_blocking(move || {
        let mut args = vec!["status", "--json", "--extended"];
        let profile_flag;
        if profile != "default" && !profile.is_empty() {
            profile_flag = profile.clone();
            args.push("--profile");
            args.push(&profile_flag);
        }
        let output = Command::new("colima")
            .args(&args)
            .output()
            .map_err(|e| format!("Failed to get status: {}", e))?;

        if !output.status.success() {
            return Err(format!(
                "colima status failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        serde_json::from_str(&stdout).map_err(|e| format!("Failed to parse status: {}", e))
    })
    .await
    {
        Ok(status) => ok(status),
        Err(e) => err(e),
    }
}

async fn api_ssh_command(
    Query(q): Query<ProfileQuery>,
) -> (StatusCode, Json<ApiResponse<Vec<String>>>) {
    let profile = q.profile;
    let mut args = vec!["ssh".to_string()];
    if profile != "default" && !profile.is_empty() {
        args.push("--profile".to_string());
        args.push(profile);
    }
    ok(args)
}

#[derive(Deserialize)]
struct K8sQuery {
    #[serde(default = "default_profile")]
    profile: String,
    action: String,
}

async fn api_k8s_action(Query(q): Query<K8sQuery>) -> (StatusCode, Json<ApiResponse<String>>) {
    let profile = q.profile;
    let action = q.action;
    match run_blocking(move || {
        let valid_actions = ["start", "stop", "delete", "reset"];
        if !valid_actions.contains(&action.as_str()) {
            return Err(format!("Invalid kubernetes action: {}", action));
        }
        let mut args = vec!["kubernetes".to_string(), action.clone()];
        if profile != "default" && !profile.is_empty() {
            args.push("--profile".to_string());
            args.push(profile);
        }
        let args_ref: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let output = Command::new("colima")
            .args(&args_ref)
            .output()
            .map_err(|e| format!("Failed to execute kubernetes {}: {}", action, e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            // Treat "not enabled" / "not running" as success for delete/stop
            // (K3s is already in the desired state)
            if (action == "delete" || action == "stop")
                && (stderr.contains("not enabled") || stderr.contains("not running"))
            {
                return Ok(format!(
                    "Kubernetes {} completed (already disabled)",
                    action
                ));
            }
            return Err(format!("kubernetes {} failed: {}", action, stderr));
        }
        Ok(format!("Kubernetes {} completed", action))
    })
    .await
    {
        Ok(msg) => ok(msg),
        Err(e) => err(e),
    }
}

async fn api_start_instance(
    Json(config): Json<colima::StartConfig>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    match run_blocking(move || {
        let mut args = vec!["start".to_string()];

        if config.profile != "default" && !config.profile.is_empty() {
            args.push("--profile".to_string());
            args.push(config.profile.clone());
        }
        args.push("--runtime".to_string());
        args.push(config.runtime);
        args.push("--cpu".to_string());
        args.push(config.cpus.to_string());
        args.push("--memory".to_string());
        args.push(config.memory.to_string());
        args.push("--disk".to_string());
        args.push(config.disk.to_string());

        if !config.vm_type.is_empty() {
            args.push("--vm-type".to_string());
            args.push(config.vm_type);
        }
        if !config.arch.is_empty() {
            args.push("--arch".to_string());
            args.push(config.arch);
        }
        if !config.mount_type.is_empty() {
            args.push("--mount-type".to_string());
            args.push(config.mount_type);
        }
        if config.kubernetes {
            args.push("--kubernetes".to_string());
        }
        if config.network_address {
            args.push("--network-address".to_string());
        }

        let args_ref: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let output = Command::new("colima")
            .args(&args_ref)
            .output()
            .map_err(|e| format!("Failed to start colima: {}", e))?;

        if !output.status.success() {
            return Err(format!(
                "colima start failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        Ok(format!(
            "Instance '{}' started successfully",
            config.profile
        ))
    })
    .await
    {
        Ok(msg) => ok(msg),
        Err(e) => err(e),
    }
}

// ===== Docker routes =====

#[derive(Deserialize)]
struct ContainerQuery {
    #[serde(default)]
    all: bool,
}

async fn api_list_containers(
    Query(q): Query<ContainerQuery>,
) -> (StatusCode, Json<ApiResponse<Vec<docker::DockerContainer>>>) {
    let all = q.all;
    match run_blocking(move || {
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

        Ok(stdout
            .lines()
            .filter(|l| !l.trim().is_empty())
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect())
    })
    .await
    {
        Ok(list) => ok(list),
        Err(e) => err(e),
    }
}

#[derive(Deserialize)]
struct ContainerIdQuery {
    #[serde(rename = "containerId")]
    container_id: String,
    #[serde(default)]
    force: bool,
    #[serde(default = "default_lines")]
    lines: u32,
}

fn default_lines() -> u32 {
    200
}

async fn api_start_container(
    Query(q): Query<ContainerIdQuery>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    let id = q.container_id;
    match run_blocking(move || {
        run_cmd("docker", &["start", &id]).map(|_| format!("Container {} started", id))
    })
    .await
    {
        Ok(msg) => ok(msg),
        Err(e) => err(e),
    }
}

async fn api_stop_container(
    Query(q): Query<ContainerIdQuery>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    let id = q.container_id;
    match run_blocking(move || {
        run_cmd("docker", &["stop", &id]).map(|_| format!("Container {} stopped", id))
    })
    .await
    {
        Ok(msg) => ok(msg),
        Err(e) => err(e),
    }
}

async fn api_restart_container(
    Query(q): Query<ContainerIdQuery>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    let id = q.container_id;
    match run_blocking(move || {
        run_cmd("docker", &["restart", &id]).map(|_| format!("Container {} restarted", id))
    })
    .await
    {
        Ok(msg) => ok(msg),
        Err(e) => err(e),
    }
}

async fn api_remove_container(
    Query(q): Query<ContainerIdQuery>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    let id = q.container_id;
    let force = q.force;
    match run_blocking(move || {
        let mut args = vec!["rm"];
        if force {
            args.push("-f");
        }
        args.push(&id);
        run_cmd("docker", &args).map(|_| format!("Container {} removed", id))
    })
    .await
    {
        Ok(msg) => ok(msg),
        Err(e) => err(e),
    }
}

async fn api_container_logs(
    Query(q): Query<ContainerIdQuery>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    let id = q.container_id;
    let lines = q.lines;
    match run_blocking(move || {
        let tail = lines.to_string();
        let output = docker_cmd()
            .args(["logs", "--tail", &tail, "--timestamps", &id])
            .output()
            .map_err(|e| format!("Failed to get logs: {}", e))?;

        if !output.status.success() {
            return Err(format!(
                "docker logs failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        Ok(if stdout.is_empty() {
            stderr.to_string()
        } else {
            stdout.to_string()
        })
    })
    .await
    {
        Ok(logs) => ok(logs),
        Err(e) => err(e),
    }
}

async fn api_inspect_container(
    Query(q): Query<ContainerIdQuery>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    let id = q.container_id;
    match run_blocking(move || run_cmd("docker", &["inspect", &id])).await {
        Ok(info) => ok(info),
        Err(e) => err(e),
    }
}

async fn api_list_images() -> (StatusCode, Json<ApiResponse<Vec<docker::DockerImage>>>) {
    match run_blocking(|| {
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

        Ok(stdout
            .lines()
            .filter(|l| !l.trim().is_empty())
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect())
    })
    .await
    {
        Ok(list) => ok(list),
        Err(e) => err(e),
    }
}

// ===== Image management routes =====

#[derive(Deserialize)]
struct ImageIdQuery {
    #[serde(default, alias = "imageId")]
    image_id: String,
    #[serde(default)]
    force: Option<bool>,
}

#[derive(Deserialize)]
struct ImagePullQuery {
    #[serde(default, alias = "imageName")]
    image_name: String,
}

#[derive(Deserialize)]
struct ImageTagBody {
    source: String,
    target: String,
}

#[derive(Deserialize)]
struct PruneQuery {
    #[serde(default)]
    all: Option<bool>,
}

async fn api_remove_image(
    Query(q): Query<ImageIdQuery>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    let id = q.image_id;
    let force = q.force.unwrap_or(false);
    match run_blocking(move || {
        // First attempt: simple rmi
        let mut args = vec!["rmi"];
        if force {
            args.push("-f");
        }
        args.push(&id);
        let result = run_cmd("docker", &args);

        match result {
            Ok(out) => Ok(out),
            Err(e) if force && e.contains("being used by") => {
                // Find containers using this image and stop+remove them
                let ps_output = docker_cmd()
                    .args(["ps", "-a", "-q", "--filter", &format!("ancestor={}", id)])
                    .output()
                    .map_err(|e| format!("Failed to list containers: {}", e))?;

                let container_ids = String::from_utf8_lossy(&ps_output.stdout);
                for cid in container_ids.lines() {
                    let cid = cid.trim();
                    if cid.is_empty() {
                        continue;
                    }
                    // Stop then remove
                    let _ = docker_cmd().args(["stop", cid]).output();
                    let _ = docker_cmd().args(["rm", "-f", cid]).output();
                }

                // Retry image removal
                run_cmd("docker", &["rmi", "-f", &id])
            }
            Err(e) => Err(e),
        }
    })
    .await
    {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}

async fn api_pull_image(
    Query(q): Query<ImagePullQuery>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    let name = q.image_name;
    match run_blocking(move || run_cmd("docker", &["pull", &name])).await {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}

async fn api_prune_images() -> (StatusCode, Json<ApiResponse<String>>) {
    match run_blocking(|| run_cmd("docker", &["image", "prune", "-a", "-f"])).await {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}

async fn api_inspect_image(
    Query(q): Query<ImageIdQuery>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    let id = q.image_id;
    match run_blocking(move || run_cmd("docker", &["image", "inspect", &id])).await {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}

async fn api_tag_image(Json(body): Json<ImageTagBody>) -> (StatusCode, Json<ApiResponse<String>>) {
    match run_blocking(move || run_cmd("docker", &["tag", &body.source, &body.target])).await {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}

async fn api_system_prune(Query(q): Query<PruneQuery>) -> (StatusCode, Json<ApiResponse<String>>) {
    let all = q.all.unwrap_or(false);
    match run_blocking(move || {
        let mut args = vec!["system", "prune", "-f"];
        if all {
            args.push("-a");
        }
        run_cmd("docker", &args)
    })
    .await
    {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}

async fn api_system_df() -> (StatusCode, Json<ApiResponse<String>>) {
    match run_blocking(|| run_cmd("docker", &["system", "df", "-v"])).await {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}

// ===== Volume routes =====

#[derive(Deserialize)]
struct VolumeNameQuery {
    name: String,
    #[serde(default)]
    force: Option<bool>,
}

#[derive(Deserialize)]
struct CreateVolumeBody {
    name: String,
    #[serde(default)]
    driver: String,
}

async fn api_list_volumes() -> (StatusCode, Json<ApiResponse<Vec<volumes::DockerVolume>>>) {
    match run_blocking(|| {
        let output = docker_cmd()
            .args(["volume", "ls", "--format", "json"])
            .output()
            .map_err(|e| format!("Failed to list volumes: {}", e))?;
        if !output.status.success() {
            return Err(format!(
                "docker volume ls failed: {}",
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
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect())
    })
    .await
    {
        Ok(list) => ok(list),
        Err(e) => err(e),
    }
}

async fn api_create_volume(
    Json(body): Json<CreateVolumeBody>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    match run_blocking(move || {
        let mut args = vec!["volume".to_string(), "create".to_string()];
        if !body.driver.is_empty() && body.driver != "local" {
            args.push("--driver".to_string());
            args.push(body.driver);
        }
        args.push(body.name);
        let args_ref: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        run_cmd("docker", &args_ref)
    })
    .await
    {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}

async fn api_remove_volume(
    Query(q): Query<VolumeNameQuery>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    let name = q.name;
    let force = q.force.unwrap_or(false);
    match run_blocking(move || {
        let mut args = vec!["volume", "rm"];
        if force {
            args.push("-f");
        }
        args.push(&name);
        run_cmd("docker", &args)
    })
    .await
    {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}

async fn api_prune_volumes() -> (StatusCode, Json<ApiResponse<String>>) {
    match run_blocking(|| run_cmd("docker", &["volume", "prune", "-f"])).await {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}

async fn api_inspect_volume(
    Query(q): Query<VolumeNameQuery>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    let name = q.name;
    match run_blocking(move || run_cmd("docker", &["volume", "inspect", &name])).await {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}

// ===== Network routes =====

#[derive(Deserialize)]
struct NetworkNameQuery {
    name: String,
}

#[derive(Deserialize)]
struct CreateNetworkBody {
    name: String,
    #[serde(default)]
    driver: String,
    #[serde(default)]
    subnet: String,
}

async fn api_list_networks() -> (StatusCode, Json<ApiResponse<Vec<networks::DockerNetwork>>>) {
    match run_blocking(|| {
        let output = docker_cmd()
            .args(["network", "ls", "--format", "json", "--no-trunc"])
            .output()
            .map_err(|e| format!("Failed to list networks: {}", e))?;
        if !output.status.success() {
            return Err(format!(
                "docker network ls failed: {}",
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
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect())
    })
    .await
    {
        Ok(list) => ok(list),
        Err(e) => err(e),
    }
}

async fn api_create_network(
    Json(body): Json<CreateNetworkBody>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    match run_blocking(move || {
        let mut args = vec!["network".to_string(), "create".to_string()];
        if !body.driver.is_empty() {
            args.push("--driver".to_string());
            args.push(body.driver);
        }
        if !body.subnet.is_empty() {
            args.push("--subnet".to_string());
            args.push(body.subnet);
        }
        args.push(body.name);
        let args_ref: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        run_cmd("docker", &args_ref)
    })
    .await
    {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}

async fn api_remove_network(
    Query(q): Query<NetworkNameQuery>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    let name = q.name;
    match run_blocking(move || run_cmd("docker", &["network", "rm", &name])).await {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}

async fn api_inspect_network(
    Query(q): Query<NetworkNameQuery>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    let name = q.name;
    match run_blocking(move || run_cmd("docker", &["network", "inspect", &name])).await {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}

async fn api_prune_networks() -> (StatusCode, Json<ApiResponse<String>>) {
    match run_blocking(|| run_cmd("docker", &["network", "prune", "-f"])).await {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}

// ===== Container enhancement routes =====

#[derive(Deserialize)]
struct ContainerExecBody {
    #[serde(alias = "containerId")]
    container_id: String,
    command: String,
}

#[derive(Deserialize)]
struct RunContainerBody {
    image: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    ports: Vec<String>,
    #[serde(default, alias = "envVars")]
    env_vars: Vec<String>,
    #[serde(default)]
    volumes: Vec<String>,
    #[serde(default = "default_true")]
    detach: bool,
    #[serde(default, alias = "removeOnExit")]
    remove_on_exit: bool,
    #[serde(default, alias = "extraArgs")]
    extra_args: Vec<String>,
}

fn default_true() -> bool {
    true
}

#[derive(Deserialize)]
struct RenameContainerBody {
    #[serde(alias = "containerId")]
    container_id: String,
    #[serde(alias = "newName")]
    new_name: String,
}

async fn api_container_stats(
    Query(q): Query<ContainerIdQuery>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    let id = q.container_id;
    match run_blocking(move || {
        run_cmd("docker", &["stats", "--no-stream", "--format", "json", &id])
    })
    .await
    {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}

async fn api_all_container_stats() -> (StatusCode, Json<ApiResponse<String>>) {
    match run_blocking(|| run_cmd("docker", &["stats", "--no-stream", "--format", "json"])).await {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}

async fn api_container_top(
    Query(q): Query<ContainerIdQuery>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    let id = q.container_id;
    match run_blocking(move || run_cmd("docker", &["top", &id])).await {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}

async fn api_container_exec(
    Json(body): Json<ContainerExecBody>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    match run_blocking(move || {
        run_cmd(
            "docker",
            &["exec", &body.container_id, "sh", "-c", &body.command],
        )
    })
    .await
    {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}

async fn api_run_container(
    Json(body): Json<RunContainerBody>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    match run_blocking(move || {
        let mut args = vec!["run".to_string()];
        if body.detach {
            args.push("-d".to_string());
        }
        if body.remove_on_exit {
            args.push("--rm".to_string());
        }
        if !body.name.is_empty() {
            args.push("--name".to_string());
            args.push(body.name);
        }
        for p in &body.ports {
            args.push("-p".to_string());
            args.push(p.clone());
        }
        for e in &body.env_vars {
            args.push("-e".to_string());
            args.push(e.clone());
        }
        for v in &body.volumes {
            args.push("-v".to_string());
            args.push(v.clone());
        }
        for a in &body.extra_args {
            args.push(a.clone());
        }
        args.push(body.image);
        let args_ref: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        run_cmd("docker", &args_ref)
    })
    .await
    {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}

async fn api_rename_container(
    Json(body): Json<RenameContainerBody>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    match run_blocking(move || run_cmd("docker", &["rename", &body.container_id, &body.new_name]))
        .await
    {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}

async fn api_pause_container(
    Query(q): Query<ContainerIdQuery>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    let id = q.container_id;
    match run_blocking(move || run_cmd("docker", &["pause", &id])).await {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}

async fn api_unpause_container(
    Query(q): Query<ContainerIdQuery>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    let id = q.container_id;
    match run_blocking(move || run_cmd("docker", &["unpause", &id])).await {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}

// ===== Model routes =====

#[derive(Deserialize)]
struct ModelQuery {
    #[serde(default = "default_profile")]
    profile: String,
    #[serde(default)]
    #[serde(rename = "modelName")]
    model_name: String,
    #[serde(default = "default_port")]
    port: u16,
    #[serde(default)]
    runner: String,
}

fn default_port() -> u16 {
    11434
}

async fn api_list_models(
    Query(q): Query<ModelQuery>,
) -> (StatusCode, Json<ApiResponse<Vec<models::AiModel>>>) {
    let profile = q.profile;
    let runner = q.runner;
    match run_blocking(move || {
        let mut args = vec!["model", "list", "--profile", &profile];
        if !runner.is_empty() {
            args.push("--runner");
            args.push(&runner);
        }
        let output = Command::new("colima")
            .args(&args)
            .output()
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    "'colima' is not installed. Install with: brew install colima".to_string()
                } else {
                    format!("Failed to list models: {}", e)
                }
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            if stderr.contains("not supported") || stderr.contains("not available") {
                return Ok(vec![]);
            }
            if stderr.contains("krunkit") || stderr.contains("vm-type") {
                return Err("GPU support requires krunkit. Install with: brew tap slp/krunkit && brew install krunkit\nThen restart: colima start --runtime docker --vm-type krunkit".to_string());
            }
            return Err(format!("list models failed: {}", stderr));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.trim().is_empty() {
            return Ok(vec![]);
        }

        // Try JSON parse first
        if let Ok(models) = serde_json::from_str::<Vec<models::AiModel>>(&stdout) {
            return Ok(models);
        }

        // Fallback: parse text table output
        Ok(stdout
            .lines()
            .skip(1) // Skip header
            .filter(|l| !l.trim().is_empty() && !l.starts_with("---"))
            .map(|line| {
                let parts: Vec<&str> = line.split_whitespace().collect();
                models::AiModel {
                    name: parts.first().unwrap_or(&"unknown").to_string(),
                    size: parts.get(1).unwrap_or(&"").to_string(),
                    format: parts.get(2).unwrap_or(&"").to_string(),
                    family: parts.get(3).unwrap_or(&"").to_string(),
                    parameters: parts.get(4).unwrap_or(&"").to_string(),
                    quantization: parts.get(5).unwrap_or(&"").to_string(),
                }
            })
            .collect())
    })
    .await
    {
        Ok(list) => ok(list),
        Err(e) => err(e),
    }
}

async fn api_pull_model(Query(q): Query<ModelQuery>) -> (StatusCode, Json<ApiResponse<String>>) {
    let profile = q.profile;
    let model_name = q.model_name;
    let runner = q.runner;
    match run_blocking(move || {
        let mut args = vec!["model", "pull", &model_name, "--profile", &profile];
        if !runner.is_empty() {
            args.push("--runner");
            args.push(&runner);
        }
        run_cmd("colima", &args)
            .map(|_| format!("Model '{}' pulled", model_name))
    })
    .await
    {
        Ok(msg) => ok(msg),
        Err(e) => err(e),
    }
}

async fn api_serve_model(Query(q): Query<ModelQuery>) -> (StatusCode, Json<ApiResponse<String>>) {
    let profile = q.profile;
    let model_name = q.model_name;
    let port = q.port;
    let runner = q.runner;
    let port_str = port.to_string();
    match run_blocking(move || {
        let mut args = vec![
            "model", "serve", &model_name, "--port", &port_str, "--profile", &profile,
        ];
        if !runner.is_empty() {
            args.push("--runner");
            args.push(&runner);
        }
        run_cmd("colima", &args)
            .map(|_| format!("Model '{}' served on port {}", model_name, port))
    })
    .await
    {
        Ok(msg) => ok(msg),
        Err(e) => err(e),
    }
}

async fn api_delete_model(Query(q): Query<ModelQuery>) -> (StatusCode, Json<ApiResponse<String>>) {
    let profile = q.profile;
    let model_name = q.model_name;
    let runner = q.runner;
    match run_blocking(move || {
        let mut args = vec!["model", "delete", &model_name, "--profile", &profile];
        if !runner.is_empty() {
            args.push("--runner");
            args.push(&runner);
        }
        run_cmd("colima", &args)
            .map(|_| format!("Model '{}' deleted", model_name))
    })
    .await
    {
        Ok(msg) => ok(msg),
        Err(e) => err(e),
    }
}

// ===== Terminal session routes (browser mode) =====

#[derive(Deserialize)]
struct TerminalCreateParams {
    session_id: String,
    profile: String,
    #[serde(default = "default_vm_type")]
    vm_type: String,
}

fn default_vm_type() -> String {
    "colima".to_string()
}

#[derive(Deserialize)]
struct TerminalWriteParams {
    session_id: String,
    data: String,
}

#[derive(Deserialize)]
struct TerminalSessionParams {
    session_id: String,
}

async fn api_terminal_create(
    State(mgr): State<SharedSessionManager>,
    Json(params): Json<TerminalCreateParams>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    let result = {
        let mut m = mgr.lock().unwrap();
        m.create(&params.session_id, &params.profile, &params.vm_type)
    };
    match result {
        Ok(()) => ok(format!("Session '{}' created", params.session_id)),
        Err(e) => err(e),
    }
}

async fn api_terminal_write(
    State(mgr): State<SharedSessionManager>,
    Json(params): Json<TerminalWriteParams>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    let result = {
        let mut m = mgr.lock().unwrap();
        m.write(&params.session_id, &params.data)
    };
    match result {
        Ok(()) => ok("ok".to_string()),
        Err(e) => err(e),
    }
}

async fn api_terminal_read(
    State(mgr): State<SharedSessionManager>,
    Query(params): Query<TerminalSessionParams>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    let result = {
        let mut m = mgr.lock().unwrap();
        m.read(&params.session_id)
    };
    match result {
        Ok(data) => ok(data),
        // Return empty for non-existent sessions (graceful for stale polls)
        Err(_) => ok(String::new()),
    }
}

async fn api_terminal_close(
    State(mgr): State<SharedSessionManager>,
    Json(params): Json<TerminalSessionParams>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    let result = {
        let mut m = mgr.lock().unwrap();
        m.close(&params.session_id)
    };
    match result {
        Ok(()) => ok("closed".to_string()),
        Err(e) => err(e),
    }
}

async fn api_terminal_resize(
    State(_mgr): State<SharedSessionManager>,
    Json(_params): Json<serde_json::Value>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    // Resize not supported in pipe mode, but don't error
    ok("ok".to_string())
}

// ===== AI Chat route =====

async fn api_ai_chat(
    Json(body): Json<serde_json::Value>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    let provider = body["provider"].as_str().unwrap_or("").to_string();
    let model = body["model"].as_str().unwrap_or("").to_string();
    let api_key = body["api_key"].as_str().unwrap_or("").to_string();
    let endpoint = body["endpoint"].as_str().unwrap_or("").to_string();
    let messages: Vec<crate::commands::ai_chat::ChatMessage> = body["messages"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|m| {
                    Some(crate::commands::ai_chat::ChatMessage {
                        role: m["role"].as_str()?.to_string(),
                        content: m["content"].as_str()?.to_string(),
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    let request = crate::commands::ai_chat::AiChatRequest {
        provider,
        model,
        api_key,
        messages,
        endpoint,
    };

    match crate::commands::ai_chat::ai_chat(request).await {
        Ok(response) => ok(response),
        Err(e) => err(e),
    }
}

async fn api_ai_list_models(
    Json(body): Json<serde_json::Value>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    let provider = body["provider"].as_str().unwrap_or("").to_string();
    let api_key = body["api_key"].as_str().unwrap_or("").to_string();
    let endpoint = body["endpoint"].as_str().unwrap_or("").to_string();

    match crate::commands::ai_chat::ai_list_models(provider, api_key, endpoint).await {
        Ok(models) => ok(models),
        Err(e) => err(e),
    }
}

// ===== Docker System routes =====

async fn api_docker_df() -> (StatusCode, Json<ApiResponse<String>>) {
    match run_blocking(|| run_cmd("docker", &["system", "df"])).await {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}

async fn api_docker_prune() -> (StatusCode, Json<ApiResponse<String>>) {
    match run_blocking(|| run_cmd("docker", &["system", "prune", "-af"])).await {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}

// ===== Lima routes =====

#[derive(Deserialize)]
struct LimaNameBody {
    name: String,
}

#[derive(Deserialize)]
struct LimaDeleteBody {
    name: String,
    #[serde(default)]
    force: bool,
}

#[derive(Deserialize)]
struct LimaShellBody {
    name: String,
    command: String,
}

async fn api_lima_list() -> (StatusCode, Json<ApiResponse<String>>) {
    match run_blocking(|| run_cmd("limactl", &["list", "--json"])).await {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}

async fn api_lima_start(Json(body): Json<LimaNameBody>) -> (StatusCode, Json<ApiResponse<String>>) {
    let name = body.name;
    match run_blocking(move || run_cmd("limactl", &["start", &name])).await {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}

async fn api_lima_stop(Json(body): Json<LimaNameBody>) -> (StatusCode, Json<ApiResponse<String>>) {
    let name = body.name;
    match run_blocking(move || run_cmd("limactl", &["stop", &name])).await {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}

async fn api_lima_delete(
    Json(body): Json<LimaDeleteBody>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    let name = body.name;
    let force = body.force;
    match run_blocking(move || {
        if force {
            run_cmd("limactl", &["delete", "--force", &name])
        } else {
            run_cmd("limactl", &["delete", &name])
        }
    })
    .await
    {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}

async fn api_lima_info() -> (StatusCode, Json<ApiResponse<String>>) {
    match run_blocking(|| run_cmd("limactl", &["info"])).await {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}

async fn api_lima_shell(
    Json(body): Json<LimaShellBody>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    let name = body.name;
    let command = body.command;
    match run_blocking(move || {
        let output = Command::new("limactl")
            .args(["shell", &name, "--", "sh", "-c", &command])
            .output()
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    "'limactl' is not installed. Install with: brew install lima".to_string()
                } else {
                    format!("Failed to execute shell command: {}", e)
                }
            })?;

        // Combine stdout and stderr — shell commands write to both
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        let mut result = String::new();
        if !stdout.is_empty() {
            result.push_str(&stdout);
        }
        if !stderr.is_empty() {
            if !result.is_empty() && !result.ends_with('\n') {
                result.push('\n');
            }
            result.push_str(&stderr);
        }

        // Non-zero exit is not a fatal error for shell commands
        // (e.g. apt returns 100 for lock errors but still has useful output)
        if !output.status.success() && result.trim().is_empty() {
            return Err(format!(
                "Command exited with code {:?}",
                output.status.code()
            ));
        }

        Ok(result)
    })
    .await
    {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}

async fn api_lima_templates() -> (StatusCode, Json<ApiResponse<String>>) {
    match run_blocking(|| run_cmd("limactl", &["start", "--list-templates"])).await {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}

#[derive(Deserialize)]
struct LimaCreateBody {
    name: String,
    #[serde(default = "default_cpus")]
    cpus: u32,
    #[serde(default = "default_memory")]
    memory: u32,
    #[serde(default = "default_disk")]
    disk: u32,
    #[serde(default)]
    template: String,
}
fn default_cpus() -> u32 {
    2
}
fn default_memory() -> u32 {
    2
}
fn default_disk() -> u32 {
    60
}

async fn api_lima_create(
    Json(body): Json<LimaCreateBody>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    let name = body.name.clone();
    let cpus = body.cpus;
    let memory = body.memory;
    let disk = body.disk;
    let template = body.template.clone();
    let name_start = body.name.clone();

    match run_blocking(move || {
        let mut args = vec![
            "create".to_string(),
            format!("--name={}", name),
            format!("--cpus={}", cpus),
            format!("--memory={}", memory),
            format!("--disk={}", disk),
            "--tty=false".to_string(),
        ];
        if !template.is_empty() {
            args.push(format!("template:{}", template));
        }
        let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        run_cmd("limactl", &arg_refs)?;
        // auto-start after create
        run_cmd("limactl", &["start", &name_start])
    })
    .await
    {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}

// ===== Kubernetes routes =====

#[derive(Deserialize)]
struct K8sNsQuery {
    #[serde(default)]
    namespace: String,
}

#[derive(Deserialize)]
struct K8sPodLogQuery {
    namespace: String,
    pod: String,
    #[serde(default = "default_log_lines")]
    lines: u32,
}

#[derive(Deserialize)]
struct K8sDeletePodBody {
    namespace: String,
    pod: String,
}

#[derive(Deserialize)]
struct K8sDescribeQuery {
    namespace: String,
    #[serde(alias = "resourceType")]
    resource_type: String,
    name: String,
}

#[derive(Deserialize)]
struct K8sScaleBody {
    namespace: String,
    deployment: String,
    replicas: u32,
}

async fn api_k8s_check() -> (StatusCode, Json<ApiResponse<String>>) {
    match run_blocking(|| run_cmd("kubectl", &["cluster-info", "--request-timeout=3s"])).await {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}

async fn api_k8s_namespaces() -> (StatusCode, Json<ApiResponse<String>>) {
    match run_blocking(|| run_cmd("kubectl", &["get", "namespaces", "-o", "json"])).await {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}

async fn api_k8s_pods(Query(q): Query<K8sNsQuery>) -> (StatusCode, Json<ApiResponse<String>>) {
    let ns = q.namespace;
    match run_blocking(move || {
        if ns.is_empty() || ns == "all" {
            run_cmd(
                "kubectl",
                &["get", "pods", "-o", "json", "--all-namespaces"],
            )
        } else {
            run_cmd("kubectl", &["get", "pods", "-o", "json", "-n", &ns])
        }
    })
    .await
    {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}

async fn api_k8s_services(Query(q): Query<K8sNsQuery>) -> (StatusCode, Json<ApiResponse<String>>) {
    let ns = q.namespace;
    match run_blocking(move || {
        if ns.is_empty() || ns == "all" {
            run_cmd(
                "kubectl",
                &["get", "services", "-o", "json", "--all-namespaces"],
            )
        } else {
            run_cmd("kubectl", &["get", "services", "-o", "json", "-n", &ns])
        }
    })
    .await
    {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}

async fn api_k8s_deployments(
    Query(q): Query<K8sNsQuery>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    let ns = q.namespace;
    match run_blocking(move || {
        if ns.is_empty() || ns == "all" {
            run_cmd(
                "kubectl",
                &["get", "deployments", "-o", "json", "--all-namespaces"],
            )
        } else {
            run_cmd("kubectl", &["get", "deployments", "-o", "json", "-n", &ns])
        }
    })
    .await
    {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}

async fn api_k8s_pod_logs(
    Query(q): Query<K8sPodLogQuery>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    let tail = q.lines.to_string();
    let ns = q.namespace;
    let pod = q.pod;
    match run_blocking(move || {
        run_cmd(
            "kubectl",
            &["logs", "-n", &ns, &pod, "--tail", &tail, "--timestamps"],
        )
    })
    .await
    {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}

async fn api_k8s_delete_pod(
    Json(body): Json<K8sDeletePodBody>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    let ns = body.namespace;
    let pod = body.pod;
    match run_blocking(move || run_cmd("kubectl", &["delete", "pod", "-n", &ns, &pod])).await {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}

async fn api_k8s_describe(
    Query(q): Query<K8sDescribeQuery>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    let rt = q.resource_type;
    let ns = q.namespace;
    let name = q.name;
    match run_blocking(move || run_cmd("kubectl", &["describe", &rt, "-n", &ns, &name])).await {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}

async fn api_k8s_scale(Json(body): Json<K8sScaleBody>) -> (StatusCode, Json<ApiResponse<String>>) {
    let replicas = format!("--replicas={}", body.replicas);
    let ns = body.namespace;
    let dep = body.deployment;
    match run_blocking(move || {
        run_cmd(
            "kubectl",
            &["scale", "deployment", &dep, "-n", &ns, &replicas],
        )
    })
    .await
    {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}

async fn api_k8s_nodes() -> (StatusCode, Json<ApiResponse<String>>) {
    match run_blocking(|| run_cmd("kubectl", &["get", "nodes", "-o", "wide"])).await {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}

async fn api_k8s_events(Query(q): Query<K8sNsQuery>) -> (StatusCode, Json<ApiResponse<String>>) {
    let ns = q.namespace;
    match run_blocking(move || {
        if ns.is_empty() || ns == "all" {
            run_cmd(
                "kubectl",
                &[
                    "get",
                    "events",
                    "--sort-by=.metadata.creationTimestamp",
                    "--all-namespaces",
                ],
            )
        } else {
            run_cmd(
                "kubectl",
                &[
                    "get",
                    "events",
                    "--sort-by=.metadata.creationTimestamp",
                    "-n",
                    &ns,
                ],
            )
        }
    })
    .await
    {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}

// Generic K8s resource list endpoint — handles configmaps, secrets, statefulsets, etc.
#[derive(Deserialize)]
struct K8sResourceQuery {
    resource: String,
    #[serde(default)]
    namespace: String,
}

async fn api_k8s_resources(
    Query(q): Query<K8sResourceQuery>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    let resource = q.resource;
    let ns = q.namespace;
    // Whitelist allowed resource types
    let allowed = [
        "pods",
        "deployments",
        "services",
        "namespaces",
        "configmaps",
        "secrets",
        "statefulsets",
        "daemonsets",
        "replicasets",
        "jobs",
        "cronjobs",
        "ingresses",
        "persistentvolumes",
        "persistentvolumeclaims",
        "pv",
        "pvc",
        "endpoints",
        "serviceaccounts",
        "roles",
        "rolebindings",
        "clusterroles",
        "clusterrolebindings",
        "storageclasses",
        "networkpolicies",
        "horizontalpodautoscalers",
        "hpa",
        "limitranges",
        "resourcequotas",
        "poddisruptionbudgets",
        "pdb",
    ];
    if !allowed.contains(&resource.as_str()) {
        return err(format!("Resource type '{}' not allowed", resource));
    }
    match run_blocking(move || {
        if ns.is_empty() || ns == "all" {
            run_cmd(
                "kubectl",
                &["get", &resource, "-o", "json", "--all-namespaces"],
            )
        } else {
            run_cmd("kubectl", &["get", &resource, "-o", "json", "-n", &ns])
        }
    })
    .await
    {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}

// Generic K8s resource delete
#[derive(Deserialize)]
struct K8sDeleteBody {
    #[serde(alias = "resourceType")]
    resource_type: String,
    namespace: String,
    name: String,
}

async fn api_k8s_delete_resource(
    Json(body): Json<K8sDeleteBody>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    let rt = body.resource_type;
    let ns = body.namespace;
    let name = body.name;
    match run_blocking(move || run_cmd("kubectl", &["delete", &rt, &name, "-n", &ns])).await {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}

// Rollout restart (for deployments, statefulsets, daemonsets)
#[derive(Deserialize)]
struct K8sRestartBody {
    #[serde(alias = "resourceType")]
    resource_type: String,
    namespace: String,
    name: String,
}

async fn api_k8s_restart(
    Json(body): Json<K8sRestartBody>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    let rt = body.resource_type;
    let ns = body.namespace;
    let name = body.name;
    let target = format!("{}/{}", rt, name);
    match run_blocking(move || run_cmd("kubectl", &["rollout", "restart", &target, "-n", &ns]))
        .await
    {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}

// Get resource YAML
#[derive(Deserialize)]
struct K8sYamlQuery {
    #[serde(alias = "resourceType")]
    resource_type: String,
    namespace: String,
    name: String,
}

async fn api_k8s_yaml(Query(q): Query<K8sYamlQuery>) -> (StatusCode, Json<ApiResponse<String>>) {
    let rt = q.resource_type;
    let ns = q.namespace;
    let name = q.name;
    match run_blocking(move || {
        if ns.is_empty() || ns == "all" {
            run_cmd("kubectl", &["get", &rt, &name, "-o", "yaml"])
        } else {
            run_cmd("kubectl", &["get", &rt, &name, "-n", &ns, "-o", "yaml"])
        }
    })
    .await
    {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}

// Nodes as JSON
async fn api_k8s_nodes_json() -> (StatusCode, Json<ApiResponse<String>>) {
    match run_blocking(|| run_cmd("kubectl", &["get", "nodes", "-o", "json"])).await {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}

// Events as JSON
async fn api_k8s_events_json(
    Query(q): Query<K8sNsQuery>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    let ns = q.namespace;
    match run_blocking(move || {
        if ns.is_empty() || ns == "all" {
            run_cmd(
                "kubectl",
                &[
                    "get",
                    "events",
                    "-o",
                    "json",
                    "--sort-by=.metadata.creationTimestamp",
                    "--all-namespaces",
                ],
            )
        } else {
            run_cmd("kubectl", &["get", "events", "-o", "json", "-n", &ns])
        }
    })
    .await
    {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}

// K8s contexts
async fn api_k8s_contexts() -> (StatusCode, Json<ApiResponse<String>>) {
    match run_blocking(|| run_cmd("kubectl", &["config", "get-contexts", "-o", "name"])).await {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}

async fn api_k8s_current_context() -> (StatusCode, Json<ApiResponse<String>>) {
    match run_blocking(|| run_cmd("kubectl", &["config", "current-context"])).await {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}

#[derive(Deserialize)]
struct K8sContextBody {
    context: String,
}

async fn api_k8s_set_context(
    Json(body): Json<K8sContextBody>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    let ctx = body.context;
    match run_blocking(move || run_cmd("kubectl", &["config", "use-context", &ctx])).await {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}

// ===== Phase 2: YAML Apply =====

#[derive(Deserialize)]
struct K8sApplyBody {
    yaml: String,
    #[serde(default)]
    namespace: String,
}

async fn api_k8s_apply(Json(body): Json<K8sApplyBody>) -> (StatusCode, Json<ApiResponse<String>>) {
    let yaml_content = body.yaml;
    let ns = body.namespace;
    match run_blocking(move || {
        use std::io::Write;
        use std::process::{Command, Stdio};
        let mut args = vec!["apply", "-f", "-"];
        if !ns.is_empty() && ns != "all" {
            args.push("-n");
            args.push(&ns);
        }
        let mut child = Command::new("kubectl")
            .args(&args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to spawn kubectl: {}", e))?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(yaml_content.as_bytes())
                .map_err(|e| format!("Failed to write YAML: {}", e))?;
        }
        let output = child
            .wait_with_output()
            .map_err(|e| format!("Failed to wait: {}", e))?;
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).to_string())
        }
    })
    .await
    {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}

// ===== Phase 2: Port Forward =====

lazy_static::lazy_static! {
    static ref PORT_FORWARDS: Mutex<HashMap<String, u32>> = Mutex::new(HashMap::new());
}

#[derive(Deserialize)]
struct K8sPortForwardBody {
    namespace: String,
    #[serde(alias = "resourceType", default = "default_pod_type")]
    resource_type: String,
    name: String,
    #[serde(alias = "localPort")]
    local_port: u16,
    #[serde(alias = "remotePort")]
    remote_port: u16,
}
fn default_pod_type() -> String {
    "pod".to_string()
}

async fn api_k8s_port_forward_start(
    Json(body): Json<K8sPortForwardBody>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    let _key = format!("{}:{}", body.local_port, body.remote_port);
    let ns = body.namespace;
    let target = format!("{}/{}", body.resource_type, body.name);
    let ports = format!("{}:{}", body.local_port, body.remote_port);
    let local_port = body.local_port;

    match run_blocking(move || {
        use std::process::{Command, Stdio};
        let child = Command::new("kubectl")
            .args(&["port-forward", "-n", &ns, &target, &ports])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("Failed to start port-forward: {}", e))?;
        let pid = child.id();
        if let Ok(mut fwds) = PORT_FORWARDS.lock() {
            fwds.insert(format!("{}", local_port), pid);
        }
        Ok(format!(
            "Port forward started: localhost:{} → {}",
            local_port, ports
        ))
    })
    .await
    {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}

#[derive(Deserialize)]
struct K8sPortForwardStopBody {
    #[serde(alias = "localPort")]
    local_port: u16,
}

async fn api_k8s_port_forward_stop(
    Json(body): Json<K8sPortForwardStopBody>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    let port = body.local_port;
    match run_blocking(move || {
        if let Ok(mut fwds) = PORT_FORWARDS.lock() {
            let key = format!("{}", port);
            if let Some(pid) = fwds.remove(&key) {
                #[cfg(unix)]
                unsafe {
                    libc::kill(pid as i32, libc::SIGTERM);
                }
                return Ok(format!("Port forward on {} stopped", port));
            }
        }
        // Fallback: kill by port
        let _ = std::process::Command::new("lsof")
            .args(&["-ti", &format!(":{}", port)])
            .output()
            .and_then(|o| {
                let pids = String::from_utf8_lossy(&o.stdout).trim().to_string();
                if !pids.is_empty() {
                    std::process::Command::new("kill")
                        .args(pids.split('\n'))
                        .output()?;
                }
                Ok(())
            });
        Ok(format!("Port forward on {} stopped", port))
    })
    .await
    {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}

async fn api_k8s_port_forward_list() -> (StatusCode, Json<ApiResponse<String>>) {
    match run_blocking(|| {
        let fwds = PORT_FORWARDS.lock().map_err(|e| format!("{}", e))?;
        let result: Vec<String> = fwds
            .iter()
            .map(|(port, pid)| format!("{}:{}", port, pid))
            .collect();
        Ok(result.join("\n"))
    })
    .await
    {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}

// ===== Phase 2: Exec Shell =====

#[derive(Deserialize)]
struct K8sExecBody {
    namespace: String,
    pod: String,
    #[serde(default)]
    container: String,
}

async fn api_k8s_exec(Json(body): Json<K8sExecBody>) -> (StatusCode, Json<ApiResponse<String>>) {
    let ns = body.namespace;
    let pod = body.pod;
    let container = body.container;
    match run_blocking(move || {
        let mut cmd_str = format!("kubectl exec -it -n {} {}", ns, pod);
        if !container.is_empty() {
            cmd_str.push_str(&format!(" -c {}", container));
        }
        cmd_str.push_str(" -- /bin/sh");

        // Open in macOS Terminal.app
        #[cfg(target_os = "macos")]
        {
            std::process::Command::new("osascript")
                .args(&[
                    "-e",
                    &format!("tell application \"Terminal\" to do script \"{}\"", cmd_str),
                ])
                .spawn()
                .map_err(|e| format!("Failed to open terminal: {}", e))?;
        }
        #[cfg(target_os = "linux")]
        {
            // Try common terminals
            let terminals = ["gnome-terminal", "xterm", "konsole"];
            let mut launched = false;
            for term in &terminals {
                if std::process::Command::new(term)
                    .args(&["--", "sh", "-c", &cmd_str])
                    .spawn()
                    .is_ok()
                {
                    launched = true;
                    break;
                }
            }
            if !launched {
                return Err("No terminal emulator found".to_string());
            }
        }
        Ok(format!("Shell opened for pod {}", pod))
    })
    .await
    {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}

// ===== Phase 2: Container-level logs =====

#[derive(Deserialize)]
struct K8sContainerLogQuery {
    namespace: String,
    pod: String,
    #[serde(default)]
    container: String,
    #[serde(default = "default_log_lines")]
    lines: u32,
    #[serde(default)]
    previous: bool,
}

async fn api_k8s_container_logs(
    Query(q): Query<K8sContainerLogQuery>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    let tail = q.lines.to_string();
    let ns = q.namespace;
    let pod = q.pod;
    let container = q.container;
    let previous = q.previous;
    match run_blocking(move || {
        let mut args = vec!["logs", "-n", &ns, &pod, "--tail", &tail, "--timestamps"];
        if !container.is_empty() {
            args.push("-c");
            args.push(&container);
        }
        if previous {
            args.push("--previous");
        }
        run_cmd("kubectl", &args)
    })
    .await
    {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}

// Get pod containers list
async fn api_k8s_pod_containers(
    Query(q): Query<K8sDeletePodBody>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    let ns = q.namespace;
    let pod = q.pod;
    match run_blocking(move || {
        run_cmd(
            "kubectl",
            &[
                "get",
                "pod",
                "-n",
                &ns,
                &pod,
                "-o",
                "jsonpath={.spec.containers[*].name}",
            ],
        )
    })
    .await
    {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}

// ===== Phase 2: Node operations =====

#[derive(Deserialize)]
struct K8sNodeBody {
    name: String,
    action: String, // cordon, uncordon, drain
}

async fn api_k8s_node_action(
    Json(body): Json<K8sNodeBody>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    let name = body.name;
    let action = body.action;
    match run_blocking(move || match action.as_str() {
        "cordon" => run_cmd("kubectl", &["cordon", &name]),
        "uncordon" => run_cmd("kubectl", &["uncordon", &name]),
        "drain" => run_cmd(
            "kubectl",
            &[
                "drain",
                &name,
                "--ignore-daemonsets",
                "--delete-emptydir-data",
                "--force",
            ],
        ),
        _ => Err(format!("Unknown action: {}", action)),
    })
    .await
    {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}

// ===== Phase 2: Kind cluster management =====

async fn api_kind_list() -> (StatusCode, Json<ApiResponse<String>>) {
    match run_blocking(|| run_cmd("kind", &["get", "clusters"])).await {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}

#[derive(Deserialize)]
struct KindCreateBody {
    name: String,
    #[serde(default)]
    image: String,
}

async fn api_kind_create(
    Json(body): Json<KindCreateBody>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    let name = body.name;
    let image = body.image;
    match run_blocking(move || {
        let mut args = vec!["create", "cluster", "--name", &name];
        if !image.is_empty() {
            args.push("--image");
            args.push(&image);
        }
        run_cmd("kind", &args)
    })
    .await
    {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}

#[derive(Deserialize)]
struct KindDeleteBody {
    name: String,
}

async fn api_kind_delete(
    Json(body): Json<KindDeleteBody>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    let name = body.name;
    match run_blocking(move || run_cmd("kind", &["delete", "cluster", "--name", &name])).await {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}

// ===== Phase 3: Generic Scale =====

#[derive(Deserialize)]
struct K8sGenericScaleBody {
    #[serde(alias = "resourceType", default = "default_deployment_type")]
    resource_type: String,
    namespace: String,
    name: String,
    replicas: u32,
}
fn default_deployment_type() -> String {
    "deployment".to_string()
}

async fn api_k8s_generic_scale(
    Json(body): Json<K8sGenericScaleBody>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    let replicas = format!("--replicas={}", body.replicas);
    let ns = body.namespace;
    let name = body.name;
    let rt = body.resource_type;
    match run_blocking(move || run_cmd("kubectl", &["scale", &rt, &name, "-n", &ns, &replicas]))
        .await
    {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}

// ===== Phase 3: Cluster Health Analysis =====

async fn api_k8s_cluster_health() -> (StatusCode, Json<ApiResponse<String>>) {
    match run_blocking(|| {
        // Gather data from multiple sources
        let pods_raw = run_cmd("kubectl", &["get", "pods", "--all-namespaces", "-o", "json"]).unwrap_or_default();
        let deploys_raw = run_cmd("kubectl", &["get", "deployments", "--all-namespaces", "-o", "json"]).unwrap_or_default();
        let pvcs_raw = run_cmd("kubectl", &["get", "pvc", "--all-namespaces", "-o", "json"]).unwrap_or_default();
        let events_raw = run_cmd("kubectl", &["get", "events", "--all-namespaces", "--field-selector=type=Warning", "-o", "json"]).unwrap_or_default();
        let nodes_raw = run_cmd("kubectl", &["get", "nodes", "-o", "json"]).unwrap_or_default();

        let mut issues: Vec<serde_json::Value> = Vec::new();
        let mut score: u32 = 100;

        // Check pods
        if let Ok(pods) = serde_json::from_str::<serde_json::Value>(&pods_raw) {
            if let Some(items) = pods["items"].as_array() {
                let total_pods = items.len();
                let mut unhealthy = 0u32;
                for pod in items {
                    let phase = pod["status"]["phase"].as_str().unwrap_or("");
                    let name = pod["metadata"]["name"].as_str().unwrap_or("");
                    let ns = pod["metadata"]["namespace"].as_str().unwrap_or("");
                    let containers = pod["status"]["containerStatuses"].as_array();

                    if phase == "Failed" || phase == "Unknown" {
                        unhealthy += 1;
                        issues.push(serde_json::json!({
                            "severity": "error",
                            "category": "Pod",
                            "resource": format!("{}/{}", ns, name),
                            "message": format!("Pod is in {} phase", phase)
                        }));
                    } else if phase == "Pending" {
                        unhealthy += 1;
                        issues.push(serde_json::json!({
                            "severity": "warning",
                            "category": "Pod",
                            "resource": format!("{}/{}", ns, name),
                            "message": "Pod is pending"
                        }));
                    }

                    if let Some(statuses) = containers {
                        for cs in statuses {
                            let restarts = cs["restartCount"].as_u64().unwrap_or(0);
                            let ready = cs["ready"].as_bool().unwrap_or(false);
                            let cname = cs["name"].as_str().unwrap_or("");
                            if restarts > 5 {
                                issues.push(serde_json::json!({
                                    "severity": "warning",
                                    "category": "Pod",
                                    "resource": format!("{}/{}", ns, name),
                                    "message": format!("Container {} has {} restarts", cname, restarts)
                                }));
                            }
                            if !ready {
                                issues.push(serde_json::json!({
                                    "severity": "warning",
                                    "category": "Pod",
                                    "resource": format!("{}/{}", ns, name),
                                    "message": format!("Container {} is not ready", cname)
                                }));
                            }
                            // Check CrashLoopBackOff
                            if let Some(waiting) = cs["state"]["waiting"]["reason"].as_str() {
                                if waiting == "CrashLoopBackOff" || waiting == "ImagePullBackOff" || waiting == "ErrImagePull" {
                                    issues.push(serde_json::json!({
                                        "severity": "error",
                                        "category": "Pod",
                                        "resource": format!("{}/{}", ns, name),
                                        "message": format!("Container {} in {}", cname, waiting)
                                    }));
                                }
                            }
                        }
                    }
                }
                if unhealthy > 0 {
                    score = score.saturating_sub(unhealthy * 5);
                }
                issues.push(serde_json::json!({
                    "severity": "info",
                    "category": "Summary",
                    "resource": "Pods",
                    "message": format!("{} total, {} unhealthy", total_pods, unhealthy)
                }));
            }
        }

        // Check deployments
        if let Ok(deploys) = serde_json::from_str::<serde_json::Value>(&deploys_raw) {
            if let Some(items) = deploys["items"].as_array() {
                for dep in items {
                    let name = dep["metadata"]["name"].as_str().unwrap_or("");
                    let ns = dep["metadata"]["namespace"].as_str().unwrap_or("");
                    let desired = dep["spec"]["replicas"].as_u64().unwrap_or(0);
                    let ready = dep["status"]["readyReplicas"].as_u64().unwrap_or(0);
                    let available = dep["status"]["availableReplicas"].as_u64().unwrap_or(0);
                    if ready < desired {
                        score = score.saturating_sub(3);
                        issues.push(serde_json::json!({
                            "severity": "warning",
                            "category": "Deployment",
                            "resource": format!("{}/{}", ns, name),
                            "message": format!("Only {}/{} replicas ready", ready, desired)
                        }));
                    }
                    if available < desired {
                        issues.push(serde_json::json!({
                            "severity": "warning",
                            "category": "Deployment",
                            "resource": format!("{}/{}", ns, name),
                            "message": format!("Only {}/{} replicas available", available, desired)
                        }));
                    }
                }
            }
        }

        // Check PVCs
        if let Ok(pvcs) = serde_json::from_str::<serde_json::Value>(&pvcs_raw) {
            if let Some(items) = pvcs["items"].as_array() {
                for pvc in items {
                    let name = pvc["metadata"]["name"].as_str().unwrap_or("");
                    let ns = pvc["metadata"]["namespace"].as_str().unwrap_or("");
                    let phase = pvc["status"]["phase"].as_str().unwrap_or("");
                    if phase != "Bound" {
                        score = score.saturating_sub(5);
                        issues.push(serde_json::json!({
                            "severity": "error",
                            "category": "PVC",
                            "resource": format!("{}/{}", ns, name),
                            "message": format!("PVC is {}", phase)
                        }));
                    }
                }
            }
        }

        // Check nodes
        if let Ok(nodes) = serde_json::from_str::<serde_json::Value>(&nodes_raw) {
            if let Some(items) = nodes["items"].as_array() {
                for node in items {
                    let name = node["metadata"]["name"].as_str().unwrap_or("");
                    let conditions = node["status"]["conditions"].as_array();
                    if let Some(conds) = conditions {
                        for cond in conds {
                            let ctype = cond["type"].as_str().unwrap_or("");
                            let status = cond["status"].as_str().unwrap_or("");
                            if ctype == "Ready" && status != "True" {
                                score = score.saturating_sub(15);
                                issues.push(serde_json::json!({
                                    "severity": "error",
                                    "category": "Node",
                                    "resource": name,
                                    "message": "Node is NotReady"
                                }));
                            }
                            if (ctype == "MemoryPressure" || ctype == "DiskPressure" || ctype == "PIDPressure") && status == "True" {
                                score = score.saturating_sub(10);
                                issues.push(serde_json::json!({
                                    "severity": "error",
                                    "category": "Node",
                                    "resource": name,
                                    "message": format!("{} detected", ctype)
                                }));
                            }
                        }
                    }
                    // Check unschedulable
                    if node["spec"]["unschedulable"].as_bool().unwrap_or(false) {
                        issues.push(serde_json::json!({
                            "severity": "warning",
                            "category": "Node",
                            "resource": name,
                            "message": "Node is cordoned (unschedulable)"
                        }));
                    }
                }
            }
        }

        // Check warning events
        if let Ok(events) = serde_json::from_str::<serde_json::Value>(&events_raw) {
            if let Some(items) = events["items"].as_array() {
                let warning_count = items.len();
                if warning_count > 0 {
                    score = score.saturating_sub(std::cmp::min(warning_count as u32, 10));
                    // Get top 5 warnings
                    for evt in items.iter().rev().take(5) {
                        let reason = evt["reason"].as_str().unwrap_or("");
                        let msg = evt["message"].as_str().unwrap_or("").chars().take(120).collect::<String>();
                        let obj = format!("{}/{}",
                            evt["involvedObject"]["kind"].as_str().unwrap_or(""),
                            evt["involvedObject"]["name"].as_str().unwrap_or(""));
                        issues.push(serde_json::json!({
                            "severity": "warning",
                            "category": "Event",
                            "resource": obj,
                            "message": format!("{}: {}", reason, msg)
                        }));
                    }
                    issues.push(serde_json::json!({
                        "severity": "info",
                        "category": "Summary",
                        "resource": "Events",
                        "message": format!("{} warning events", warning_count)
                    }));
                }
            }
        }

        let result = serde_json::json!({
            "score": std::cmp::max(score, 0),
            "grade": if score >= 90 { "A" } else if score >= 75 { "B" } else if score >= 60 { "C" } else if score >= 40 { "D" } else { "F" },
            "issues": issues
        });

        Ok(result.to_string())
    }).await {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}

// ===== CRD Support =====

/// List all Custom Resource Definitions
async fn api_k8s_crds() -> (StatusCode, Json<ApiResponse<String>>) {
    match run_blocking(|| {
        run_cmd("kubectl", &["get", "crd", "-o", "json"])
    })
    .await
    {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}

#[derive(Deserialize)]
struct K8sCrdQuery {
    resource: String,    // e.g. "kustomizations.kustomize.toolkit.fluxcd.io"
    #[serde(default)]
    namespace: String,
}

/// List instances of a specific CRD type
async fn api_k8s_crd_resources(
    Query(q): Query<K8sCrdQuery>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    let resource = q.resource;
    let ns = q.namespace;
    // Validate: must look like a valid k8s resource name (alphanumeric, dots, hyphens)
    if resource.is_empty() || !resource.chars().all(|c| c.is_alphanumeric() || c == '.' || c == '-') {
        return err(format!("Invalid CRD resource: {}", resource));
    }
    match run_blocking(move || {
        if ns.is_empty() || ns == "all" {
            run_cmd("kubectl", &["get", &resource, "-o", "json", "--all-namespaces"])
        } else {
            run_cmd("kubectl", &["get", &resource, "-o", "json", "-n", &ns])
        }
    })
    .await
    {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}

// ===== Real-time Log Streaming via SSE =====

#[derive(Deserialize)]
struct K8sLogStreamQuery {
    namespace: String,
    pod: String,
    #[serde(default)]
    container: String,
    #[serde(default = "default_tail_lines")]
    tail: u32,
}

fn default_tail_lines() -> u32 { 50 }

async fn api_k8s_log_stream(
    Query(q): Query<K8sLogStreamQuery>,
) -> Sse<impl futures_util::Stream<Item = Result<Event, Infallible>>> {
    let ns = q.namespace;
    let pod = q.pod;
    let container = q.container;
    let tail = q.tail.to_string();

    let stream = async_stream::stream! {
        let mut args = vec![
            "logs".to_string(), "-f".to_string(),
            "-n".to_string(), ns,
            pod,
            "--tail".to_string(), tail,
            "--timestamps".to_string(),
        ];
        if !container.is_empty() {
            args.push("-c".to_string());
            args.push(container);
        }
        let child = tokio::process::Command::new("kubectl")
            .args(&args)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn();

        match child {
            Ok(mut child) => {
                if let Some(stdout) = child.stdout.take() {
                    let reader = tokio::io::BufReader::new(stdout);
                    let mut lines = tokio::io::AsyncBufReadExt::lines(reader);
                    while let Ok(Some(line)) = lines.next_line().await {
                        yield Ok(Event::default().data(line));
                    }
                }
                let _ = child.kill().await;
            }
            Err(e) => {
                yield Ok(Event::default().data(format!("[error] Failed to start kubectl: {}", e)));
            }
        }
    };

    Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(std::time::Duration::from_secs(15))
            .text("ping")
    )
}

// ===== HTTP Benchmark =====

#[derive(Deserialize)]
struct BenchmarkBody {
    url: String,
    #[serde(default = "default_concurrency")]
    concurrency: u32,
    #[serde(default = "default_requests")]
    requests: u32,
    #[serde(default)]
    method: String,   // GET, POST, PUT, DELETE
}

fn default_concurrency() -> u32 { 5 }
fn default_requests() -> u32 { 50 }

async fn api_k8s_benchmark(
    Json(body): Json<BenchmarkBody>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    let url = body.url;
    let concurrency = body.concurrency.min(100).max(1);
    let total = body.requests.min(10000).max(1);
    let method = if body.method.is_empty() { "GET".to_string() } else { body.method.to_uppercase() };

    // Validate URL — only allow localhost / 127.0.0.1 / K8s service IPs
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return err("URL must start with http:// or https://".to_string());
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .unwrap_or_default();

    let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(concurrency as usize));
    let latencies = std::sync::Arc::new(tokio::sync::Mutex::new(Vec::<u128>::with_capacity(total as usize)));
    let successes = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
    let failures = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));

    let start = std::time::Instant::now();
    let mut handles = Vec::new();

    for _ in 0..total {
        let permit = semaphore.clone().acquire_owned().await.unwrap();
        let c = client.clone();
        let u = url.clone();
        let m = method.clone();
        let lats = latencies.clone();
        let succ = successes.clone();
        let fail = failures.clone();

        handles.push(tokio::spawn(async move {
            let req_start = std::time::Instant::now();
            let result = match m.as_str() {
                "POST" => c.post(&u).send().await,
                "PUT" => c.put(&u).send().await,
                "DELETE" => c.delete(&u).send().await,
                _ => c.get(&u).send().await,
            };
            let elapsed = req_start.elapsed().as_millis();
            match result {
                Ok(resp) if resp.status().is_success() || resp.status().is_redirection() => {
                    succ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                }
                _ => {
                    fail.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                }
            }
            lats.lock().await.push(elapsed);
            drop(permit);
        }));
    }

    for h in handles {
        let _ = h.await;
    }

    let total_time = start.elapsed().as_millis();
    let mut lats = latencies.lock().await;
    lats.sort();

    let count = lats.len();
    let avg = if count > 0 { lats.iter().sum::<u128>() / count as u128 } else { 0 };
    let p50 = if count > 0 { lats[count * 50 / 100] } else { 0 };
    let p95 = if count > 0 { lats[count * 95 / 100] } else { 0 };
    let p99 = if count > 0 { lats[count.saturating_sub(1) * 99 / 100] } else { 0 };
    let min = lats.first().copied().unwrap_or(0);
    let max = lats.last().copied().unwrap_or(0);
    let rps = if total_time > 0 { count as f64 / total_time as f64 * 1000.0 } else { 0.0 };

    let result = serde_json::json!({
        "total_requests": total,
        "success": successes.load(std::sync::atomic::Ordering::Relaxed),
        "failed": failures.load(std::sync::atomic::Ordering::Relaxed),
        "total_time_ms": total_time,
        "avg_latency_ms": avg,
        "min_latency_ms": min,
        "max_latency_ms": max,
        "p50_ms": p50,
        "p95_ms": p95,
        "p99_ms": p99,
        "requests_per_sec": format!("{:.1}", rps),
        "concurrency": concurrency,
        "method": method,
    });

    ok(result.to_string())
}

// ===== Compose routes =====

#[derive(Deserialize)]
struct ComposeUpBody {
    #[serde(alias = "projectDir", default)]
    project_dir: String,
    #[serde(default = "default_true")]
    detach: bool,
}

#[derive(Deserialize)]
struct ComposeProjectBody {
    #[serde(alias = "projectName")]
    project_name: String,
}

#[derive(Deserialize)]
struct ComposeLogsQuery {
    #[serde(alias = "projectName")]
    project_name: String,
    #[serde(default = "default_log_lines")]
    lines: u32,
}

#[derive(Deserialize)]
struct ComposePsQuery {
    #[serde(alias = "projectName")]
    project_name: String,
}

fn default_log_lines() -> u32 {
    200
}

async fn api_list_compose() -> (StatusCode, Json<ApiResponse<String>>) {
    match run_blocking(|| run_cmd("docker", &["compose", "ls", "--format", "json", "-a"])).await {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}

async fn api_compose_up(
    Json(body): Json<ComposeUpBody>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    match run_blocking(move || {
        let mut args = vec!["compose"];
        if !body.project_dir.is_empty() {
            // project dir mode
        }
        args.push("up");
        if body.detach {
            args.push("-d");
        }
        run_cmd("docker", &args)
    })
    .await
    {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}

async fn api_compose_down(
    Json(body): Json<ComposeProjectBody>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    match run_blocking(move || run_cmd("docker", &["compose", "-p", &body.project_name, "down"]))
        .await
    {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}

async fn api_compose_restart(
    Json(body): Json<ComposeProjectBody>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    match run_blocking(move || run_cmd("docker", &["compose", "-p", &body.project_name, "restart"]))
        .await
    {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}

async fn api_compose_logs(
    Query(q): Query<ComposeLogsQuery>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    let tail = q.lines.to_string();
    match run_blocking(move || {
        run_cmd(
            "docker",
            &[
                "compose",
                "-p",
                &q.project_name,
                "logs",
                "--tail",
                &tail,
                "--no-color",
            ],
        )
    })
    .await
    {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}

async fn api_compose_ps(
    Query(q): Query<ComposePsQuery>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    match run_blocking(move || {
        run_cmd(
            "docker",
            &["compose", "-p", &q.project_name, "ps", "--format", "json"],
        )
    })
    .await
    {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}

/// Build the axum router with all API routes
pub fn build_router() -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        // SSE stream for browser mode
        .route("/api/events", get(api_events))
        // System
        .route("/api/system/check", get(api_check_system))
        .route("/api/system/version", get(api_get_version))
        .route("/api/system/homebrew", get(api_check_homebrew))
        .route("/api/system/check-tool", get(api_check_tool))
        .route("/api/system/platform", get(api_get_platform))
        .route("/api/system/install", post(api_install_dep))
        // Colima instances
        .route("/api/instances", get(api_list_instances))
        .route("/api/instances/start", post(api_start_instance))
        .route("/api/instances/stop", post(api_stop_instance))
        .route("/api/instances/delete", post(api_delete_instance))
        .route("/api/instances/status", get(api_instance_status))
        .route("/api/instances/ssh", get(api_ssh_command))
        .route("/api/instances/k8s", post(api_k8s_action))
        // Docker containers
        .route("/api/containers", get(api_list_containers))
        .route("/api/containers/start", post(api_start_container))
        .route("/api/containers/stop", post(api_stop_container))
        .route("/api/containers/restart", post(api_restart_container))
        .route("/api/containers/remove", post(api_remove_container))
        .route("/api/containers/logs", get(api_container_logs))
        .route("/api/containers/inspect", get(api_inspect_container))
        .route("/api/containers/stats", get(api_container_stats))
        .route("/api/containers/stats/all", get(api_all_container_stats))
        .route("/api/containers/top", get(api_container_top))
        .route("/api/containers/exec", post(api_container_exec))
        .route("/api/containers/run", post(api_run_container))
        .route("/api/containers/rename", post(api_rename_container))
        .route("/api/containers/pause", post(api_pause_container))
        .route("/api/containers/unpause", post(api_unpause_container))
        .route("/api/images", get(api_list_images))
        .route("/api/images/remove", post(api_remove_image))
        .route("/api/images/pull", post(api_pull_image))
        .route("/api/images/prune", post(api_prune_images))
        .route("/api/images/inspect", get(api_inspect_image))
        .route("/api/images/tag", post(api_tag_image))
        // Docker volumes
        .route("/api/volumes", get(api_list_volumes))
        .route("/api/volumes/create", post(api_create_volume))
        .route("/api/volumes/remove", post(api_remove_volume))
        .route("/api/volumes/prune", post(api_prune_volumes))
        .route("/api/volumes/inspect", get(api_inspect_volume))
        // Docker networks
        .route("/api/networks", get(api_list_networks))
        .route("/api/networks/create", post(api_create_network))
        .route("/api/networks/remove", post(api_remove_network))
        .route("/api/networks/inspect", get(api_inspect_network))
        .route("/api/networks/prune", post(api_prune_networks))
        // System
        .route("/api/system/prune", post(api_system_prune))
        .route("/api/system/df", get(api_system_df))
        // Models
        .route("/api/models", get(api_list_models))
        .route("/api/models/pull", post(api_pull_model))
        .route("/api/models/serve", post(api_serve_model))
        .route("/api/models/delete", post(api_delete_model))
        // Compose
        .route("/api/compose", get(api_list_compose))
        .route("/api/compose/up", post(api_compose_up))
        .route("/api/compose/down", post(api_compose_down))
        .route("/api/compose/restart", post(api_compose_restart))
        .route("/api/compose/logs", get(api_compose_logs))
        .route("/api/compose/ps", get(api_compose_ps))
        // Kubernetes
        .route("/api/k8s/check", get(api_k8s_check))
        .route("/api/k8s/namespaces", get(api_k8s_namespaces))
        .route("/api/k8s/pods", get(api_k8s_pods))
        .route("/api/k8s/services", get(api_k8s_services))
        .route("/api/k8s/deployments", get(api_k8s_deployments))
        .route("/api/k8s/pods/logs", get(api_k8s_pod_logs))
        .route("/api/k8s/pods/delete", post(api_k8s_delete_pod))
        .route("/api/k8s/describe", get(api_k8s_describe))
        .route("/api/k8s/scale", post(api_k8s_scale))
        .route("/api/k8s/nodes", get(api_k8s_nodes))
        .route("/api/k8s/events", get(api_k8s_events))
        .route("/api/k8s/resources", get(api_k8s_resources))
        .route("/api/k8s/resources/delete", post(api_k8s_delete_resource))
        .route("/api/k8s/resources/restart", post(api_k8s_restart))
        .route("/api/k8s/resources/yaml", get(api_k8s_yaml))
        .route("/api/k8s/nodes/json", get(api_k8s_nodes_json))
        .route("/api/k8s/events/json", get(api_k8s_events_json))
        .route("/api/k8s/contexts", get(api_k8s_contexts))
        .route("/api/k8s/contexts/current", get(api_k8s_current_context))
        .route("/api/k8s/contexts/set", post(api_k8s_set_context))
        // K8s Phase 2
        .route("/api/k8s/apply", post(api_k8s_apply))
        .route(
            "/api/k8s/port-forward/start",
            post(api_k8s_port_forward_start),
        )
        .route(
            "/api/k8s/port-forward/stop",
            post(api_k8s_port_forward_stop),
        )
        .route("/api/k8s/port-forward/list", get(api_k8s_port_forward_list))
        .route("/api/k8s/exec", post(api_k8s_exec))
        .route("/api/k8s/pods/containers", get(api_k8s_pod_containers))
        .route("/api/k8s/pods/container-logs", get(api_k8s_container_logs))
        .route("/api/k8s/nodes/action", post(api_k8s_node_action))
        // Kind
        .route("/api/kind", get(api_kind_list))
        .route("/api/kind/create", post(api_kind_create))
        .route("/api/kind/delete", post(api_kind_delete))
        // K8s Phase 3
        .route("/api/k8s/scale-generic", post(api_k8s_generic_scale))
        .route("/api/k8s/cluster-health", get(api_k8s_cluster_health))
        // CRDs
        .route("/api/k8s/crds", get(api_k8s_crds))
        .route("/api/k8s/crds/resources", get(api_k8s_crd_resources))
        // Log streaming
        .route("/api/k8s/pods/logs/stream", get(api_k8s_log_stream))
        // Benchmark
        .route("/api/k8s/benchmark", post(api_k8s_benchmark))
        // Lima
        .route("/api/lima", get(api_lima_list))
        .route("/api/lima/start", post(api_lima_start))
        .route("/api/lima/stop", post(api_lima_stop))
        .route("/api/lima/delete", post(api_lima_delete))
        .route("/api/lima/info", get(api_lima_info))
        .route("/api/lima/shell", post(api_lima_shell))
        .route("/api/lima/templates", get(api_lima_templates))
        .route("/api/lima/create", post(api_lima_create))
        // Docker System
        .route("/api/docker/df", get(api_docker_df))
        .route("/api/docker/prune", post(api_docker_prune))
        // AI Chat
        .route("/api/ai/chat", post(api_ai_chat))
        .route("/api/ai/models", post(api_ai_list_models))
        // Terminal sessions (browser mode)
        .route("/api/terminal/create", post(api_terminal_create))
        .route("/api/terminal/write", post(api_terminal_write))
        .route("/api/terminal/read", get(api_terminal_read))
        .route("/api/terminal/close", post(api_terminal_close))
        .route("/api/terminal/resize", post(api_terminal_resize))
        .with_state(terminal_session::create_session_manager())
        .layer(cors)
}

/// Start the HTTP API server on port 11420 on a dedicated thread.
/// Never panics — if binding fails, the server simply won't start.
pub fn start_api_server() {
    // Initialize SSE broadcast channel eagerly
    let _ = get_sse_tx();

    std::thread::spawn(|| {
        let rt = match tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => rt,
            Err(e) => {
                eprintln!("[API Server] Failed to create tokio runtime: {}", e);
                return;
            }
        };
        rt.block_on(async {
            let app = build_router();

            // Spawn Docker bollard watcher for SSE events
            tokio::spawn(sse_docker_watcher());
            // Spawn instance change publisher for SSE events
            tokio::spawn(sse_instance_publisher());

            // Try to bind with retries (previous instance may still be releasing the port)
            let mut listener_opt = None;
            for attempt in 0..5 {
                match tokio::net::TcpListener::bind("127.0.0.1:11420").await {
                    Ok(l) => {
                        listener_opt = Some(l);
                        break;
                    }
                    Err(e) => {
                        eprintln!(
                            "[API Server] Bind attempt {}/5 failed: {} — retrying in 1s",
                            attempt + 1,
                            e
                        );
                        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                    }
                }
            }

            match listener_opt {
                Some(listener) => {
                    println!("HTTP API server running on http://127.0.0.1:11420");
                    if let Err(e) = axum::serve(listener, app).await {
                        eprintln!("[API Server] Server error: {}", e);
                    }
                }
                None => {
                    eprintln!("[API Server] Could not bind to port 11420 after 5 attempts — API server disabled");
                }
            }
        });
    });
}

/// Watch Docker events via bollard and broadcast state changes to SSE clients
async fn sse_docker_watcher() {
    use bollard::system::EventsOptions;

    // Connect to Docker using detected socket
    let docker = match detect_docker_host() {
        Some(host) => {
            bollard::Docker::connect_with_local(
                host.trim_start_matches("unix://"),
                120,
                bollard::API_DEFAULT_VERSION,
            ).ok()
        }
        None => bollard::Docker::connect_with_defaults().ok(),
    };

    let docker = match docker {
        Some(d) => d,
        None => {
            eprintln!("[SSE] Could not connect to Docker — SSE Docker watcher disabled");
            return;
        }
    };

    // Initial push
    if let Some(data) = fetch_docker_state(&docker).await {
        publish_sse_event("docker-state-updated", &data);
    }

    // Watch events and push updates
    let mut stream = docker.events(Some(EventsOptions::<String>::default()));
    while let Some(event) = stream.next().await {
        if event.is_ok() {
            if let Some(data) = fetch_docker_state(&docker).await {
                publish_sse_event("docker-state-updated", &data);
            }
        }
    }
}

/// Fetch current Docker containers + images and return as JSON
async fn fetch_docker_state(docker: &bollard::Docker) -> Option<serde_json::Value> {
    use bollard::container::ListContainersOptions;
    use bollard::image::ListImagesOptions;

    let containers = docker
        .list_containers(Some(ListContainersOptions::<String> {
            all: true,
            ..Default::default()
        }))
        .await
        .unwrap_or_default();

    let images = docker
        .list_images(Some(ListImagesOptions::<String> {
            all: false,
            ..Default::default()
        }))
        .await
        .unwrap_or_default();

    let mut mapped_containers = Vec::new();
    for c in containers {
        let names = c.names.unwrap_or_default().join(", ").replace("/", "");
        let ports = match c.ports {
            Some(ports) => ports
                .iter()
                .map(|p| {
                    let typ_str = p
                        .typ
                        .as_ref()
                        .map(|t| format!("{:?}", t).to_lowercase().replace("\"", ""))
                        .unwrap_or_else(|| "tcp".to_string());
                    if let Some(ip) = &p.ip {
                        format!("{}:{}->{}/{}", ip, p.public_port.unwrap_or(0), p.private_port, typ_str)
                    } else {
                        format!("{}/{}", p.private_port, typ_str)
                    }
                })
                .collect::<Vec<String>>()
                .join(", "),
            None => "".to_string(),
        };

        mapped_containers.push(serde_json::json!({
            "id": c.id.unwrap_or_default(),
            "Names": names,
            "Image": c.image.unwrap_or_default(),
            "Status": c.status.unwrap_or_default(),
            "State": c.state.unwrap_or_default(),
            "Ports": ports,
            "CreatedAt": c.created.unwrap_or(0).to_string(),
            "Size": c.size_rw.unwrap_or(0).to_string(),
            "Command": c.command.unwrap_or_default(),
        }));
    }

    let mut mapped_images = Vec::new();
    for i in images {
        let tags = i.repo_tags.clone();
        let (repo, tag) = if !tags.is_empty() && tags[0] != "<none>:<none>" {
            let parts: Vec<&str> = tags[0].split(':').collect();
            if parts.len() == 2 {
                (parts[0].to_string(), parts[1].to_string())
            } else {
                (tags[0].clone(), "latest".to_string())
            }
        } else {
            ("<none>".to_string(), "<none>".to_string())
        };

        mapped_images.push(serde_json::json!({
            "id": i.id.replace("sha256:", ""),
            "Repository": repo,
            "Tag": tag,
            "Size": i.size.to_string(),
            "CreatedAt": i.created.to_string(),
        }));
    }

    Some(serde_json::json!({
        "containers": mapped_containers,
        "images": mapped_images
    }))
}

/// Periodically publish instance state to SSE clients
async fn sse_instance_publisher() {
    let mut last_json = String::new();
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        let instances = instance_reader::list_instances_fast();
        let data = serde_json::json!({ "instances": instances });
        let json = data.to_string();
        // Only publish if state actually changed (avoid noise)
        if json != last_json {
            publish_sse_event("instances-update", &data);
            last_json = json;
        }
    }
}
