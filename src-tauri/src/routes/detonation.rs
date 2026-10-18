//! HTTP surface for detonation sessions.
//!
//! Thin, like the other route modules: the rules — what may be run, who is
//! entitled, and when the instance is destroyed — all live in
//! `commands::detonation`, because the Tauri command path must enforce exactly
//! the same ones. A route that re-implemented any of them would be a second
//! answer to a question with one correct answer.
//!
//! The timeline is not polled through here. Events arrive on the existing SSE
//! stream as `detonation.event` / `detonation.status`; these routes cover
//! starting, stopping, reading a snapshot, and exporting.

use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;

use crate::api_server::{err, ok, ApiResponse};
use crate::commands::detonation::{self, DetonationConfig, DetonationSession};

pub async fn api_detonation_start(
    Json(config): Json<DetonationConfig>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    match detonation::start(config) {
        Ok(id) => ok(id),
        Err(e) => err(crate::error::ColimaError::from(e)),
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionBody {
    pub session_id: String,
}

/// Never gated. A stop control behind the entitlement gate would leave a lapsed
/// user with a VM they can see running and cannot stop.
pub async fn api_detonation_cancel(
    Json(body): Json<SessionBody>,
) -> (StatusCode, Json<ApiResponse<bool>>) {
    ok(detonation::cancel(&body.session_id))
}

pub async fn api_detonation_get(
    Json(body): Json<SessionBody>,
) -> (StatusCode, Json<ApiResponse<Option<DetonationSession>>>) {
    ok(detonation::get_session(&body.session_id))
}

pub async fn api_detonation_list() -> (StatusCode, Json<ApiResponse<Vec<DetonationSession>>>) {
    ok(detonation::list_sessions())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportBody {
    pub session_id: String,
    pub dir: String,
    pub filename: String,
}

pub async fn api_detonation_export(
    Json(body): Json<ExportBody>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    match detonation::export_session(
        &body.session_id,
        std::path::Path::new(&body.dir),
        &body.filename,
    ) {
        Ok(path) => ok(path),
        Err(e) => err(crate::error::ColimaError::from(e)),
    }
}
