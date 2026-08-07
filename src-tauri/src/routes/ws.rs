use crate::terminal_session::SharedSessionManager;
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::Json,
};
use serde::{Deserialize, Serialize};
use crate::api_server::*;
use crate::commands::*;
use crate::routes::payloads::*;

pub async fn api_terminal_create(
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


pub async fn api_terminal_write(
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


pub async fn api_terminal_read(
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


pub async fn api_terminal_close(
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


pub async fn api_terminal_resize(
    State(_mgr): State<SharedSessionManager>,
    Json(_params): Json<serde_json::Value>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    // Resize not supported in pipe mode, but don't error
    ok("ok".to_string())
}
