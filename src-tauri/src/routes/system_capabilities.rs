//! HTTP route for host tool detection.
//!
//! Separate from `routes/capabilities.rs`, which serves a static API schema for
//! AI agents. Same word, unrelated meanings — merging them would fuse two
//! contracts that change for different reasons.

use axum::{http::StatusCode, response::Json};

use crate::commands::system_capabilities::{self, Capability};
use crate::helpers::{err, ok, ApiResponse};

pub async fn api_system_capabilities() -> (StatusCode, Json<ApiResponse<Vec<Capability>>>) {
    match system_capabilities::get_system_capabilities().await {
        Ok(caps) => ok(caps),
        Err(e) => err(e),
    }
}
