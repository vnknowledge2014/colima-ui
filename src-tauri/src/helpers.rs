//! Shared helper utilities for the API server.
//!
//! Generic response wrappers, blocking task runner, CLI command runner,
//! timed caching, and global state (port forwards, system info cache).

use axum::{http::StatusCode, response::Json};
use serde::Serialize;
use std::collections::HashMap;
use std::process::Command;
use std::sync::Mutex;

use crate::commands::system;

// ===== API Response Wrappers =====

/// Generic API response wrapper
#[derive(Serialize)]
pub struct ApiResponse<T: Serialize> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

pub fn ok<T: Serialize>(data: T) -> (StatusCode, Json<ApiResponse<T>>) {
    (
        StatusCode::OK,
        Json(ApiResponse {
            success: true,
            data: Some(data),
            error: None,
        }),
    )
}

pub fn err<T: Serialize>(msg: String) -> (StatusCode, Json<ApiResponse<T>>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ApiResponse {
            success: false,
            data: None,
            error: Some(msg),
        }),
    )
}

// ===== Blocking Task Runner =====

/// Run a blocking closure on the thread pool to avoid starving the tokio reactor.
/// All colima/docker/system commands use std::process::Command::output() which blocks.
pub async fn run_blocking<T, F>(f: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, String> + Send + 'static,
{
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| format!("Task join error: {}", e))?
}

// ===== CLI Command Runner =====

pub fn run_cmd(program: &str, args: &[&str]) -> Result<String, String> {
    let mut cmd = Command::new(program);
    cmd.args(args);

    // Auto-set DOCKER_HOST for docker/docker-compose/kind commands
    if program == "docker" || program == "docker-compose" || program == "kind" {
        if let Some((host, _)) = crate::path_util::detect_docker_host() {
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

// ===== Timed Cache =====

/// Fix #18: Timed system info cache — refreshes every 5 minutes.
/// Replaces the OnceLock (which never invalidated, showing stale versions).
pub struct TimedCache<T> {
    pub data: Option<T>,
    pub cached_at: std::time::Instant,
    pub ttl: std::time::Duration,
}

impl<T: Clone> TimedCache<T> {
    pub fn new(ttl: std::time::Duration) -> Self {
        Self {
            data: None,
            cached_at: std::time::Instant::now(),
            ttl,
        }
    }

    pub fn get_or_init(&mut self, f: impl FnOnce() -> T) -> T {
        if let Some(ref data) = self.data {
            if self.cached_at.elapsed() < self.ttl {
                return data.clone();
            }
        }
        let val = f();
        self.data = Some(val.clone());
        self.cached_at = std::time::Instant::now();
        val
    }
}

pub static SYSTEM_INFO_CACHE: std::sync::LazyLock<Mutex<TimedCache<system::SystemInfo>>> =
    std::sync::LazyLock::new(|| Mutex::new(TimedCache::new(std::time::Duration::from_secs(300))));

pub fn load_system_info() -> system::SystemInfo {
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

// ===== Global State =====

/// Port forward process tracking (pid by port key)
pub static PORT_FORWARDS: std::sync::LazyLock<Mutex<HashMap<String, u32>>> =
    std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));
