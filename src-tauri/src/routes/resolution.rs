//! HTTP mirror of the environment resolution record.
//! Browser mode reaches the same Environment page as the desktop app; a
//! missing route here would silently diverge the two UIs.

use axum::{http::StatusCode, response::Json};

use crate::commands::resolution::{self, EnvironmentReport};
use crate::helpers::{ok, ApiResponse};

/// GET /api/environment/resolutions
///
/// Blocking work (a one-shot `$SHELL -l -c` snapshot with a 3s timeout) runs
/// on the request thread; tauri's async runtime is not involved here. The
/// snapshot itself is cached per process lifetime, so only the first call
/// pays.
pub async fn api_get_environment_resolutions() -> (StatusCode, Json<ApiResponse<EnvironmentReport>>)
{
    // run_in_place: the sync command does its own bounded waiting; wrap
    // spawn_blocking keeps the axum runtime responsive during the 3s window.
    let report = tauri::async_runtime::spawn_blocking(resolution::get_environment_resolutions)
        .await
        .unwrap_or_else(|e| EnvironmentReport {
            rows: Vec::new(),
            env_duality: Vec::new(),
            shell_snapshot_error: Some(format!("resolution task failed: {}", e)),
        });
    ok(report)
}
