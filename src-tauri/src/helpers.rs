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

/// Generic API response wrapper.
///
/// `error` carries the same structured payload the Tauri IPC path returns, so
/// browser mode and desktop mode see an identical error contract.
#[derive(Serialize)]
pub struct ApiResponse<T: Serialize> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<crate::error::ColimaError>,
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

/// Build an error response.
///
/// Takes anything convertible into [`crate::error::ColimaError`], which is what
/// lets the ~115 existing `err(e.to_string())` call sites keep working while
/// gaining classification — `From<String>` classifies the message.
pub fn err<T: Serialize>(e: impl Into<crate::error::ColimaError>) -> (StatusCode, Json<ApiResponse<T>>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ApiResponse {
            success: false,
            data: None,
            error: Some(e.into()),
        }),
    )
}

/// Build a [`crate::error::ColimaError`] from a finished process, attaching the
/// command and its exit status.
///
/// Prefer this over `format!("... {}", stderr)` at any site that has the
/// `Output` in hand: the classification is the same, but the user also learns
/// which command failed and how.
pub fn error_from_output(
    command: &str,
    output: &std::process::Output,
) -> crate::error::ColimaError {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let detail = if stderr.trim().is_empty() {
        format!("`{}` failed with no error output", command)
    } else {
        stderr.trim().to_string()
    };

    let mut e = crate::error::ColimaError::from(detail).with_command(command);
    if let Some(code) = output.status.code() {
        e = e.with_exit_code(code);
    }
    // A process that ran and failed is a command failure unless the message
    // says something more specific.
    if e.code == crate::error::ErrorCode::Unknown {
        e.code = crate::error::ErrorCode::CommandFailed;
    }
    e
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

/// Build a `Command` with the environment every CLI path in this app needs.
///
/// Shared by `run_cmd` and `streaming_cmd::run_cmd_streaming` on purpose: two
/// independently-built command paths drift, and a `docker` invocation that
/// silently loses `DOCKER_HOST` talks to the wrong daemon.
pub fn build_cmd(program: &str, args: &[&str]) -> Command {
    let mut cmd = Command::new(program);
    cmd.args(args);

    // Auto-set DOCKER_HOST for docker/docker-compose/kind commands
    if program == "docker" || program == "docker-compose" || program == "kind" {
        if let Some((host, _)) = crate::path_util::detect_docker_host() {
            cmd.env("DOCKER_HOST", host);
        }
    }

    cmd
}

/// Turn a spawn/exec failure into the message the user sees.
///
/// Shared with the streaming path so "not installed" reads identically wherever
/// the command was launched from.
pub fn describe_exec_error(program: &str, e: &std::io::Error) -> String {
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
}

pub fn run_cmd(program: &str, args: &[&str]) -> Result<String, String> {
    let mut cmd = build_cmd(program, args);

    let output = cmd
        .output()
        .map_err(|e| describe_exec_error(program, &e))?;

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

#[cfg(test)]
mod tests {
    use super::*;

    /// `run_cmd`'s body was extracted into `build_cmd` + `describe_exec_error` so
    /// the streaming path could share it. These pin the observable behaviour that
    /// extraction had to preserve.
    #[test]
    fn run_cmd_returns_stdout() {
        let out = run_cmd("sh", &["-c", "printf 'hello'"]).expect("should succeed");
        assert_eq!(out, "hello");
    }

    #[test]
    fn run_cmd_reports_a_failing_command_with_its_stderr() {
        let err = run_cmd("sh", &["-c", "echo 'no good' >&2; exit 1"])
            .expect_err("non-zero exit must be an error");
        assert!(err.contains("no good"), "unexpected error: {}", err);
    }

    #[test]
    fn run_cmd_hints_at_installation_for_a_missing_binary() {
        let err = run_cmd("colimaui-no-such-binary", &[]).expect_err("should fail");
        assert!(err.contains("is not installed"), "unexpected error: {}", err);
    }

    #[test]
    fn docker_host_is_set_only_for_container_tooling() {
        // The env decision lives in `build_cmd` now; both callers depend on it
        // being made the same way.
        let docker = build_cmd("docker", &["version"]);
        let other = build_cmd("sh", &["-c", "true"]);
        let has_docker_host = |cmd: &Command| {
            cmd.get_envs()
                .any(|(k, v)| k == "DOCKER_HOST" && v.is_some())
        };
        // Only assert the negative unconditionally: whether DOCKER_HOST gets set
        // for docker depends on a socket existing on the machine running the test.
        assert!(!has_docker_host(&other));
        if crate::path_util::detect_docker_host().is_some() {
            assert!(has_docker_host(&docker));
        }
    }
}
