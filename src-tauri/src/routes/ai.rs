use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::Json,
};
use serde::{Deserialize, Serialize};
use crate::api_server::*;
use crate::commands::*;
use crate::routes::payloads::*;

pub async fn api_ai_chat(
    Json(body): Json<serde_json::Value>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    let provider = body["provider"].as_str().unwrap_or("").to_string();
    let model = body["model"].as_str().unwrap_or("").to_string();
    let api_key = body["api_key"].as_str().unwrap_or("").to_string();
    let endpoint = body["endpoint"].as_str().unwrap_or("").to_string();
    let messages: Vec<crate::commands::ai_chat::ChatMessage> = body["messages"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|m| {
                    Some(crate::commands::ai_chat::ChatMessage {
                        role: m["role"].as_str()?.to_string(),
                        content: m["content"].as_str()?.to_string(),
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    let request = crate::commands::ai_chat::AiChatRequest {
        provider,
        model,
        api_key,
        messages,
        endpoint,
    };

    match crate::commands::ai_chat::ai_chat(request).await {
        Ok(response) => ok(response),
        Err(e) => err(e),
    }
}


pub async fn api_ai_list_models(
    Json(body): Json<serde_json::Value>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    let provider = body["provider"].as_str().unwrap_or("").to_string();
    let api_key = body["api_key"].as_str().unwrap_or("").to_string();
    let endpoint = body["endpoint"].as_str().unwrap_or("").to_string();

    match crate::commands::ai_chat::ai_list_models(provider, api_key, endpoint).await {
        Ok(models) => ok(models),
        Err(e) => err(e),
    }
}


pub async fn api_ai_search(
    Json(body): Json<serde_json::Value>,
) -> (
    StatusCode,
    Json<ApiResponse<Vec<crate::commands::searxng::SearchResult>>>,
) {
    let query = body["query"].as_str().unwrap_or("").to_string();
    let instances: Option<Vec<String>> = body["instances"].as_array().map(|arr| {
        arr.iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect()
    });
    let max_results = body["max_results"].as_u64().map(|n| n as usize);
    let timeout_secs = body["timeout_secs"].as_u64();

    match crate::commands::searxng::searxng_search(query, instances, max_results, timeout_secs)
        .await
    {
        Ok(results) => (
            StatusCode::OK,
            Json(ApiResponse {
                success: true,
                data: Some(results),
                error: None,
            }),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse {
                success: false,
                data: None,
                error: Some(e),
            }),
        ),
    }
}


pub async fn api_ai_fetch_page(
    Json(body): Json<serde_json::Value>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    let url = body["url"].as_str().unwrap_or("").to_string();
    let max_length = body["max_length"].as_u64().map(|n| n as usize);
    let mode = body["mode"].as_str().map(|s| s.to_string());

    match crate::commands::searxng::fetch_page_as_markdown(url, max_length, mode).await {
        Ok(md) => ok(md),
        Err(e) => err(e),
    }
}


pub async fn api_ai_context() -> (StatusCode, Json<ApiResponse<String>>) {
    // Find CONTEXT.md adjacent to the binary (same as Tauri resource_dir behavior)
    let exe_path = std::env::current_exe().unwrap_or_default();
    // In a bundled .app: .../ColimaUI.app/Contents/MacOS/colima-ui
    // Tauri puts resources at: .../ColimaUI.app/Contents/Resources/resources/
    let resource_dir = exe_path
        .parent() // MacOS/
        .and_then(|p| p.parent()) // Contents/
        .map(|p| p.join("Resources").join("resources"))
        .unwrap_or_else(|| exe_path.parent().unwrap_or(&exe_path).to_path_buf());

    let context_path = resource_dir.join("CONTEXT.md");
    match std::fs::read_to_string(&context_path) {
        Ok(content) => ok(content),
        Err(e) => err(format!(
            "Cannot read CONTEXT.md from {:?}: {e}",
            context_path
        )),
    }
}


pub async fn api_cli_chat(
    Json(payload): Json<agent_loop::HeadlessChatRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    match agent_loop::run_headless_agent(payload).await {
        Ok(res) => (StatusCode::OK, Json(serde_json::json!({ "result": res }))),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e }))),
    }
}
