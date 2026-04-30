use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::process::Command;
use std::sync::OnceLock;
use tower_http::cors::{Any, CorsLayer};

use crate::commands::{colima, docker, models, networks, system, volumes};
use crate::instance_reader;
use crate::terminal_session::{self, SharedSessionManager};

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

fn run_cmd(program: &str, args: &[&str]) -> Result<String, String> {
    let output = Command::new(program)
        .args(args)
        .output()
        .map_err(|e| format!("Failed to execute {}: {}", program, e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("{} failed: {}", program, stderr));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
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
    let info = SYSTEM_INFO_CACHE.get_or_init(|| {
        load_system_info()
    });
    ok(info.clone())
}

async fn api_get_version() -> (StatusCode, Json<ApiResponse<String>>) {
    let info = SYSTEM_INFO_CACHE.get_or_init(|| load_system_info());
    ok(info.colima_version.clone())
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

async fn api_stop_instance(Query(q): Query<ProfileQuery>) -> (StatusCode, Json<ApiResponse<String>>) {
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
            return Err(format!("colima stop failed: {}", String::from_utf8_lossy(&output.stderr)));
        }
        Ok(format!("Instance '{}' stopped", profile))
    }).await {
        Ok(msg) => ok(msg),
        Err(e) => err(e),
    }
}

async fn api_delete_instance(Query(q): Query<ProfileQuery>) -> (StatusCode, Json<ApiResponse<String>>) {
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
            return Err(format!("colima delete failed: {}", String::from_utf8_lossy(&output.stderr)));
        }
        Ok(format!("Instance '{}' deleted", profile))
    }).await {
        Ok(msg) => ok(msg),
        Err(e) => err(e),
    }
}

async fn api_instance_status(Query(q): Query<ProfileQuery>) -> (StatusCode, Json<ApiResponse<colima::InstanceStatus>>) {
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
            return Err(format!("colima status failed: {}", String::from_utf8_lossy(&output.stderr)));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        serde_json::from_str(&stdout).map_err(|e| format!("Failed to parse status: {}", e))
    }).await {
        Ok(status) => ok(status),
        Err(e) => err(e),
    }
}

async fn api_ssh_command(Query(q): Query<ProfileQuery>) -> (StatusCode, Json<ApiResponse<Vec<String>>>) {
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
            return Err(format!("kubernetes {} failed: {}", action, String::from_utf8_lossy(&output.stderr)));
        }
        Ok(format!("Kubernetes {} completed", action))
    }).await {
        Ok(msg) => ok(msg),
        Err(e) => err(e),
    }
}

async fn api_start_instance(Json(config): Json<colima::StartConfig>) -> (StatusCode, Json<ApiResponse<String>>) {
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
            return Err(format!("colima start failed: {}", String::from_utf8_lossy(&output.stderr)));
        }
        Ok(format!("Instance '{}' started successfully", config.profile))
    }).await {
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

async fn api_list_containers(Query(q): Query<ContainerQuery>) -> (StatusCode, Json<ApiResponse<Vec<docker::DockerContainer>>>) {
    let all = q.all;
    match run_blocking(move || {
        let mut args = vec!["ps", "--format", "json", "--no-trunc"];
        if all { args.push("-a"); }
        let output = Command::new("docker")
            .args(&args)
            .output()
            .map_err(|e| format!("Failed to execute docker: {}", e))?;

        if !output.status.success() {
            return Err(format!("docker ps failed: {}", String::from_utf8_lossy(&output.stderr)));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.trim().is_empty() { return Ok(vec![]); }

        Ok(stdout.lines()
            .filter(|l| !l.trim().is_empty())
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect())
    }).await {
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

fn default_lines() -> u32 { 200 }

async fn api_start_container(Query(q): Query<ContainerIdQuery>) -> (StatusCode, Json<ApiResponse<String>>) {
    let id = q.container_id;
    match run_blocking(move || run_cmd("docker", &["start", &id]).map(|_| format!("Container {} started", id))).await {
        Ok(msg) => ok(msg),
        Err(e) => err(e),
    }
}

async fn api_stop_container(Query(q): Query<ContainerIdQuery>) -> (StatusCode, Json<ApiResponse<String>>) {
    let id = q.container_id;
    match run_blocking(move || run_cmd("docker", &["stop", &id]).map(|_| format!("Container {} stopped", id))).await {
        Ok(msg) => ok(msg),
        Err(e) => err(e),
    }
}

async fn api_restart_container(Query(q): Query<ContainerIdQuery>) -> (StatusCode, Json<ApiResponse<String>>) {
    let id = q.container_id;
    match run_blocking(move || run_cmd("docker", &["restart", &id]).map(|_| format!("Container {} restarted", id))).await {
        Ok(msg) => ok(msg),
        Err(e) => err(e),
    }
}

async fn api_remove_container(Query(q): Query<ContainerIdQuery>) -> (StatusCode, Json<ApiResponse<String>>) {
    let id = q.container_id;
    let force = q.force;
    match run_blocking(move || {
        let mut args = vec!["rm"];
        if force { args.push("-f"); }
        args.push(&id);
        run_cmd("docker", &args).map(|_| format!("Container {} removed", id))
    }).await {
        Ok(msg) => ok(msg),
        Err(e) => err(e),
    }
}

async fn api_container_logs(Query(q): Query<ContainerIdQuery>) -> (StatusCode, Json<ApiResponse<String>>) {
    let id = q.container_id;
    let lines = q.lines;
    match run_blocking(move || {
        let tail = lines.to_string();
        let output = Command::new("docker")
            .args(["logs", "--tail", &tail, "--timestamps", &id])
            .output()
            .map_err(|e| format!("Failed to get logs: {}", e))?;

        if !output.status.success() {
            return Err(format!("docker logs failed: {}", String::from_utf8_lossy(&output.stderr)));
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        Ok(if stdout.is_empty() { stderr.to_string() } else { stdout.to_string() })
    }).await {
        Ok(logs) => ok(logs),
        Err(e) => err(e),
    }
}

async fn api_inspect_container(Query(q): Query<ContainerIdQuery>) -> (StatusCode, Json<ApiResponse<String>>) {
    let id = q.container_id;
    match run_blocking(move || run_cmd("docker", &["inspect", &id])).await {
        Ok(info) => ok(info),
        Err(e) => err(e),
    }
}

async fn api_list_images() -> (StatusCode, Json<ApiResponse<Vec<docker::DockerImage>>>) {
    match run_blocking(|| {
        let output = Command::new("docker")
            .args(["images", "--format", "json"])
            .output()
            .map_err(|e| format!("Failed to list images: {}", e))?;

        if !output.status.success() {
            return Err(format!("docker images failed: {}", String::from_utf8_lossy(&output.stderr)));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.trim().is_empty() { return Ok(vec![]); }

        Ok(stdout.lines()
            .filter(|l| !l.trim().is_empty())
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect())
    }).await {
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

async fn api_remove_image(Query(q): Query<ImageIdQuery>) -> (StatusCode, Json<ApiResponse<String>>) {
    let id = q.image_id;
    let force = q.force.unwrap_or(false);
    match run_blocking(move || {
        let mut args = vec!["rmi"];
        if force { args.push("-f"); }
        args.push(&id);
        run_cmd("docker", &args)
    }).await {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}

async fn api_pull_image(Query(q): Query<ImagePullQuery>) -> (StatusCode, Json<ApiResponse<String>>) {
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

async fn api_inspect_image(Query(q): Query<ImageIdQuery>) -> (StatusCode, Json<ApiResponse<String>>) {
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
        if all { args.push("-a"); }
        run_cmd("docker", &args)
    }).await {
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
        let output = Command::new("docker")
            .args(["volume", "ls", "--format", "json"])
            .output()
            .map_err(|e| format!("Failed to list volumes: {}", e))?;
        if !output.status.success() {
            return Err(format!("docker volume ls failed: {}", String::from_utf8_lossy(&output.stderr)));
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.trim().is_empty() { return Ok(vec![]); }
        Ok(stdout.lines().filter(|l| !l.trim().is_empty()).filter_map(|l| serde_json::from_str(l).ok()).collect())
    }).await {
        Ok(list) => ok(list),
        Err(e) => err(e),
    }
}

async fn api_create_volume(Json(body): Json<CreateVolumeBody>) -> (StatusCode, Json<ApiResponse<String>>) {
    match run_blocking(move || {
        let mut args = vec!["volume".to_string(), "create".to_string()];
        if !body.driver.is_empty() && body.driver != "local" {
            args.push("--driver".to_string());
            args.push(body.driver);
        }
        args.push(body.name);
        let args_ref: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        run_cmd("docker", &args_ref)
    }).await {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}

async fn api_remove_volume(Query(q): Query<VolumeNameQuery>) -> (StatusCode, Json<ApiResponse<String>>) {
    let name = q.name;
    let force = q.force.unwrap_or(false);
    match run_blocking(move || {
        let mut args = vec!["volume", "rm"];
        if force { args.push("-f"); }
        args.push(&name);
        run_cmd("docker", &args)
    }).await {
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

async fn api_inspect_volume(Query(q): Query<VolumeNameQuery>) -> (StatusCode, Json<ApiResponse<String>>) {
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
        let output = Command::new("docker")
            .args(["network", "ls", "--format", "json", "--no-trunc"])
            .output()
            .map_err(|e| format!("Failed to list networks: {}", e))?;
        if !output.status.success() {
            return Err(format!("docker network ls failed: {}", String::from_utf8_lossy(&output.stderr)));
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.trim().is_empty() { return Ok(vec![]); }
        Ok(stdout.lines().filter(|l| !l.trim().is_empty()).filter_map(|l| serde_json::from_str(l).ok()).collect())
    }).await {
        Ok(list) => ok(list),
        Err(e) => err(e),
    }
}

async fn api_create_network(Json(body): Json<CreateNetworkBody>) -> (StatusCode, Json<ApiResponse<String>>) {
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
    }).await {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}

async fn api_remove_network(Query(q): Query<NetworkNameQuery>) -> (StatusCode, Json<ApiResponse<String>>) {
    let name = q.name;
    match run_blocking(move || run_cmd("docker", &["network", "rm", &name])).await {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}

async fn api_inspect_network(Query(q): Query<NetworkNameQuery>) -> (StatusCode, Json<ApiResponse<String>>) {
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

fn default_true() -> bool { true }

#[derive(Deserialize)]
struct RenameContainerBody {
    #[serde(alias = "containerId")]
    container_id: String,
    #[serde(alias = "newName")]
    new_name: String,
}

async fn api_container_stats(Query(q): Query<ContainerIdQuery>) -> (StatusCode, Json<ApiResponse<String>>) {
    let id = q.container_id;
    match run_blocking(move || run_cmd("docker", &["stats", "--no-stream", "--format", "json", &id])).await {
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

async fn api_container_top(Query(q): Query<ContainerIdQuery>) -> (StatusCode, Json<ApiResponse<String>>) {
    let id = q.container_id;
    match run_blocking(move || run_cmd("docker", &["top", &id])).await {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}

async fn api_container_exec(Json(body): Json<ContainerExecBody>) -> (StatusCode, Json<ApiResponse<String>>) {
    match run_blocking(move || run_cmd("docker", &["exec", &body.container_id, "sh", "-c", &body.command])).await {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}

async fn api_run_container(Json(body): Json<RunContainerBody>) -> (StatusCode, Json<ApiResponse<String>>) {
    match run_blocking(move || {
        let mut args = vec!["run".to_string()];
        if body.detach { args.push("-d".to_string()); }
        if body.remove_on_exit { args.push("--rm".to_string()); }
        if !body.name.is_empty() {
            args.push("--name".to_string());
            args.push(body.name);
        }
        for p in &body.ports { args.push("-p".to_string()); args.push(p.clone()); }
        for e in &body.env_vars { args.push("-e".to_string()); args.push(e.clone()); }
        for v in &body.volumes { args.push("-v".to_string()); args.push(v.clone()); }
        for a in &body.extra_args { args.push(a.clone()); }
        args.push(body.image);
        let args_ref: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        run_cmd("docker", &args_ref)
    }).await {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}

async fn api_rename_container(Json(body): Json<RenameContainerBody>) -> (StatusCode, Json<ApiResponse<String>>) {
    match run_blocking(move || run_cmd("docker", &["rename", &body.container_id, &body.new_name])).await {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}

async fn api_pause_container(Query(q): Query<ContainerIdQuery>) -> (StatusCode, Json<ApiResponse<String>>) {
    let id = q.container_id;
    match run_blocking(move || run_cmd("docker", &["pause", &id])).await {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}

async fn api_unpause_container(Query(q): Query<ContainerIdQuery>) -> (StatusCode, Json<ApiResponse<String>>) {
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
}

fn default_port() -> u16 { 11434 }

async fn api_list_models(Query(q): Query<ModelQuery>) -> (StatusCode, Json<ApiResponse<Vec<models::AiModel>>>) {
    let profile = q.profile;
    match run_blocking(move || {
        let output = Command::new("colima")
            .args(["ssh", "--profile", &profile, "--", "ollama", "list", "--json"])
            .output()
            .map_err(|e| format!("Failed to list models: {}", e))?;

        if !output.status.success() {
            return Err(format!("list models failed: {}", String::from_utf8_lossy(&output.stderr)));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.trim().is_empty() { return Ok(vec![]); }

        Ok(stdout.lines()
            .filter(|l| !l.trim().is_empty())
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect())
    }).await {
        Ok(list) => ok(list),
        Err(e) => err(e),
    }
}

async fn api_pull_model(Query(q): Query<ModelQuery>) -> (StatusCode, Json<ApiResponse<String>>) {
    let profile = q.profile;
    let model_name = q.model_name;
    match run_blocking(move || {
        run_cmd("colima", &["ssh", "--profile", &profile, "--", "ollama", "pull", &model_name])
            .map(|_| format!("Model '{}' pulled", model_name))
    }).await {
        Ok(msg) => ok(msg),
        Err(e) => err(e),
    }
}

async fn api_serve_model(Query(q): Query<ModelQuery>) -> (StatusCode, Json<ApiResponse<String>>) {
    let profile = q.profile;
    let model_name = q.model_name;
    let port = q.port;
    match run_blocking(move || {
        run_cmd("colima", &["ssh", "--profile", &profile, "--", "ollama", "serve", &model_name, "--port", &port.to_string()])
            .map(|_| format!("Model '{}' served on port {}", model_name, port))
    }).await {
        Ok(msg) => ok(msg),
        Err(e) => err(e),
    }
}

async fn api_delete_model(Query(q): Query<ModelQuery>) -> (StatusCode, Json<ApiResponse<String>>) {
    let profile = q.profile;
    let model_name = q.model_name;
    match run_blocking(move || {
        run_cmd("colima", &["ssh", "--profile", &profile, "--", "ollama", "rm", &model_name])
            .map(|_| format!("Model '{}' deleted", model_name))
    }).await {
        Ok(msg) => ok(msg),
        Err(e) => err(e),
    }
}

// ===== Terminal session routes (browser mode) =====

#[derive(Deserialize)]
struct TerminalCreateParams {
    session_id: String,
    profile: String,
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
        m.create(&params.session_id, &params.profile)
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

async fn api_ai_chat(Json(body): Json<serde_json::Value>) -> (StatusCode, Json<ApiResponse<String>>) {
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

async fn api_ai_list_models(Json(body): Json<serde_json::Value>) -> (StatusCode, Json<ApiResponse<String>>) {
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

async fn api_lima_delete(Json(body): Json<LimaDeleteBody>) -> (StatusCode, Json<ApiResponse<String>>) {
    let name = body.name;
    let force = body.force;
    match run_blocking(move || {
        if force {
            run_cmd("limactl", &["delete", "--force", &name])
        } else {
            run_cmd("limactl", &["delete", &name])
        }
    }).await {
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

async fn api_lima_shell(Json(body): Json<LimaShellBody>) -> (StatusCode, Json<ApiResponse<String>>) {
    let name = body.name;
    let command = body.command;
    match run_blocking(move || run_cmd("limactl", &["shell", &name, "--", "sh", "-c", &command])).await {
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
            run_cmd("kubectl", &["get", "pods", "-o", "json", "--all-namespaces"])
        } else {
            run_cmd("kubectl", &["get", "pods", "-o", "json", "-n", &ns])
        }
    }).await {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}

async fn api_k8s_services(Query(q): Query<K8sNsQuery>) -> (StatusCode, Json<ApiResponse<String>>) {
    let ns = q.namespace;
    match run_blocking(move || {
        if ns.is_empty() || ns == "all" {
            run_cmd("kubectl", &["get", "services", "-o", "json", "--all-namespaces"])
        } else {
            run_cmd("kubectl", &["get", "services", "-o", "json", "-n", &ns])
        }
    }).await {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}

async fn api_k8s_deployments(Query(q): Query<K8sNsQuery>) -> (StatusCode, Json<ApiResponse<String>>) {
    let ns = q.namespace;
    match run_blocking(move || {
        if ns.is_empty() || ns == "all" {
            run_cmd("kubectl", &["get", "deployments", "-o", "json", "--all-namespaces"])
        } else {
            run_cmd("kubectl", &["get", "deployments", "-o", "json", "-n", &ns])
        }
    }).await {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}

async fn api_k8s_pod_logs(Query(q): Query<K8sPodLogQuery>) -> (StatusCode, Json<ApiResponse<String>>) {
    let tail = q.lines.to_string();
    let ns = q.namespace;
    let pod = q.pod;
    match run_blocking(move || run_cmd("kubectl", &["logs", "-n", &ns, &pod, "--tail", &tail, "--timestamps"])).await {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}

async fn api_k8s_delete_pod(Json(body): Json<K8sDeletePodBody>) -> (StatusCode, Json<ApiResponse<String>>) {
    let ns = body.namespace;
    let pod = body.pod;
    match run_blocking(move || run_cmd("kubectl", &["delete", "pod", "-n", &ns, &pod])).await {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}

async fn api_k8s_describe(Query(q): Query<K8sDescribeQuery>) -> (StatusCode, Json<ApiResponse<String>>) {
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
    match run_blocking(move || run_cmd("kubectl", &["scale", "deployment", &dep, "-n", &ns, &replicas])).await {
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
            run_cmd("kubectl", &["get", "events", "--sort-by=.metadata.creationTimestamp", "--all-namespaces"])
        } else {
            run_cmd("kubectl", &["get", "events", "--sort-by=.metadata.creationTimestamp", "-n", &ns])
        }
    }).await {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
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

fn default_log_lines() -> u32 { 200 }

async fn api_list_compose() -> (StatusCode, Json<ApiResponse<String>>) {
    match run_blocking(|| run_cmd("docker", &["compose", "ls", "--format", "json", "-a"])).await {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}

async fn api_compose_up(Json(body): Json<ComposeUpBody>) -> (StatusCode, Json<ApiResponse<String>>) {
    match run_blocking(move || {
        let mut args = vec!["compose"];
        if !body.project_dir.is_empty() {
            // project dir mode
        }
        args.push("up");
        if body.detach { args.push("-d"); }
        run_cmd("docker", &args)
    }).await {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}

async fn api_compose_down(Json(body): Json<ComposeProjectBody>) -> (StatusCode, Json<ApiResponse<String>>) {
    match run_blocking(move || run_cmd("docker", &["compose", "-p", &body.project_name, "down"])).await {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}

async fn api_compose_restart(Json(body): Json<ComposeProjectBody>) -> (StatusCode, Json<ApiResponse<String>>) {
    match run_blocking(move || run_cmd("docker", &["compose", "-p", &body.project_name, "restart"])).await {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}

async fn api_compose_logs(Query(q): Query<ComposeLogsQuery>) -> (StatusCode, Json<ApiResponse<String>>) {
    let tail = q.lines.to_string();
    match run_blocking(move || run_cmd("docker", &["compose", "-p", &q.project_name, "logs", "--tail", &tail, "--no-color"])).await {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}

async fn api_compose_ps(Query(q): Query<ComposePsQuery>) -> (StatusCode, Json<ApiResponse<String>>) {
    match run_blocking(move || run_cmd("docker", &["compose", "-p", &q.project_name, "ps", "--format", "json"])).await {
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
        // System
        .route("/api/system/check", get(api_check_system))
        .route("/api/system/version", get(api_get_version))
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
        // Lima
        .route("/api/lima", get(api_lima_list))
        .route("/api/lima/start", post(api_lima_start))
        .route("/api/lima/stop", post(api_lima_stop))
        .route("/api/lima/delete", post(api_lima_delete))
        .route("/api/lima/info", get(api_lima_info))
        .route("/api/lima/shell", post(api_lima_shell))
        .route("/api/lima/templates", get(api_lima_templates))
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

/// Start the HTTP API server on port 11420 on a dedicated thread
pub fn start_api_server() {
    std::thread::spawn(|| {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("Failed to create tokio runtime for API server");
        rt.block_on(async {
            let app = build_router();
            let listener = tokio::net::TcpListener::bind("127.0.0.1:11420")
                .await
                .expect("Failed to bind API server to port 11420");
            println!("HTTP API server running on http://127.0.0.1:11420");
            axum::serve(listener, app).await.unwrap();
        });
    });
}
