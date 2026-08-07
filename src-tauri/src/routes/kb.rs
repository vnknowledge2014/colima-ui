use axum::{
    extract::Query,
    http::StatusCode,
    response::Json,
};

use crate::api_server::*;
use crate::commands::*;
use crate::routes::payloads::*;

pub async fn api_get_settings() -> (StatusCode, Json<ApiResponse<std::collections::HashMap<String, String>>>) {
    match knowledge_bank::get_all_settings().await {
        Ok(settings) => ok(settings),
        Err(e) => err(e),
    }
}


pub async fn api_set_setting(
    Json(body): Json<SetSettingRequest>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    match knowledge_bank::set_setting(body.key, body.value).await {
        Ok(msg) => ok(msg),
        Err(e) => err(e),
    }
}


pub async fn api_kb_query(
    Json(body): Json<KbQueryRequest>,
) -> (StatusCode, Json<ApiResponse<serde_json::Value>>) {
    match knowledge_bank::kb_query(body.error_text).await {
        Ok(result) => ok(serde_json::to_value(result).unwrap_or_default()),
        Err(e) => err(e),
    }
}


pub async fn api_kb_search(
    Json(body): Json<KbSearchRequest>,
) -> (StatusCode, Json<ApiResponse<Vec<String>>>) {
    match knowledge_bank::search_memory(body.query, body.limit).await {
        Ok(results) => ok(results),
        Err(e) => err(e),
    }
}


pub async fn api_kb_get_memories() -> (StatusCode, Json<ApiResponse<serde_json::Value>>) {
    match knowledge_bank::get_all_memories().await {
        Ok(memories) => ok(serde_json::to_value(memories).unwrap_or_default()),
        Err(e) => err(e),
    }
}


pub async fn api_kb_update_memory(
    Json(body): Json<UpdateMemoryRequest>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    match knowledge_bank::update_memory(body.id, body.content).await {
        Ok(msg) => ok(msg),
        Err(e) => err(e),
    }
}


pub async fn api_kb_delete_memory(
    Json(body): Json<DeleteMemoryRequest>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    match knowledge_bank::delete_memory(body.id).await {
        Ok(msg) => ok(msg),
        Err(e) => err(e),
    }
}


pub async fn api_sandbox_execute(
    Json(body): Json<SandboxRequest>,
) -> (StatusCode, Json<ApiResponse<serde_json::Value>>) {
    match shell_sandbox::sandbox_execute(body.command).await {
        Ok(result) => ok(serde_json::to_value(result).unwrap_or_default()),
        Err(e) => err(e),
    }
}


pub async fn api_sandbox_execute_approved(
    Json(body): Json<SandboxRequest>,
) -> (StatusCode, Json<ApiResponse<serde_json::Value>>) {
    match shell_sandbox::sandbox_execute_approved(body.command).await {
        Ok(result) => ok(serde_json::to_value(result).unwrap_or_default()),
        Err(e) => err(e),
    }
}


pub async fn api_diagnostics_logs(
    Query(q): Query<std::collections::HashMap<String, String>>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    let profile = q.get("profile").cloned().unwrap_or_else(|| "default".to_string());
    match colima::collect_diagnostic_logs(profile).await {
        Ok(report) => ok(report),
        Err(e) => err(e),
    }
}
