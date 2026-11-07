//! Axum HTTP handlers for the Native A2A Protocol endpoints:
//! - `GET /.well-known/agent.json`
//! - `POST /a2a`

use axum::{http::StatusCode, response::Json};
use serde_json::Value;

#[allow(clippy::unused_async)]
pub async fn api_a2a_agent_card() -> (StatusCode, Json<Value>) {
    (StatusCode::OK, Json(crate::a2a::build_agent_card()))
}

pub async fn api_a2a_rpc(Json(body): Json<Value>) -> (StatusCode, Json<Value>) {
    let response = crate::a2a::handle_a2a_jsonrpc(body).await;
    (StatusCode::OK, Json(response))
}
