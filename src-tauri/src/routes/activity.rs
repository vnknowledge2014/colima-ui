//! Reading the local action history in browser mode.
//!
//! Read-only, and deliberately so: this is a record of what happened, and an
//! endpoint that could write to it would let anything holding the local token
//! forge one. Rows are appended by the actions themselves, nowhere else.
//!
//! Not gated on entitlement either. A Free install keeps a shorter history, but
//! what it kept is its own record of its own machine.

use axum::{extract::Query, http::StatusCode, response::Json};

use crate::api_server::*;
use crate::commands::activity::{self, ActivityFilter, ActivityRow};

pub async fn api_activity_query(
    Query(filter): Query<ActivityFilter>,
) -> (StatusCode, Json<ApiResponse<Vec<ActivityRow>>>) {
    match crate::helpers::run_blocking(move || activity::query(&filter)).await {
        Ok(rows) => ok(rows),
        Err(e) => err(crate::error::ColimaError::internal(e)),
    }
}

/// The merged timeline: the action log plus the four stores that record their
/// own outcomes.
///
/// Never fails on a single unreadable source — the response names what was
/// missing instead, because a timeline that quietly drops a store reads as
/// "nothing happened".
pub async fn api_activity_feed(
    Query(filter): Query<ActivityFilter>,
) -> (StatusCode, Json<ApiResponse<crate::commands::activity_feed::Feed>>) {
    match crate::helpers::run_blocking(move || {
        Ok(crate::commands::activity_feed::feed(&filter))
    })
    .await
    {
        Ok(feed) => ok(feed),
        Err(e) => err(crate::error::ColimaError::internal(e)),
    }
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportBody {
    pub dest_dir: String,
    pub file_name: String,
    pub format: String,
    #[serde(default)]
    pub overwrite: bool,
}

/// Write the timeline to a file. Gated, unlike reading it.
pub async fn api_activity_export(
    Json(body): Json<ExportBody>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    match crate::commands::activity_feed::activity_export(
        body.dest_dir,
        body.file_name,
        body.format,
        Some(body.overwrite),
    )
    .await
    {
        Ok(path) => ok(path),
        Err(e) => err(e),
    }
}
