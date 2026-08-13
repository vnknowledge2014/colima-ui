use axum::{http::StatusCode, response::Json};
use crate::api_server::*;
use crate::commands::topology;

/// The Docker topology graph for the current engine.
///
/// No instance parameter: there is no multi-instance targeting layer in this
/// codebase, so a parameter here would be decoration that callers could set and
/// nothing would honour.
pub async fn api_topology() -> (StatusCode, Json<ApiResponse<topology::TopologyGraph>>) {
    match topology::get_topology().await {
        Ok(graph) => ok(graph),
        Err(e) => err(e.to_string()),
    }
}
