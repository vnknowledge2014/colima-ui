use axum::{
    http::StatusCode,
    response::Json,
};
use crate::api_server::*;
use crate::commands::*;


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
        Err(e) => err(e.to_string()),
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
        Err(e) => err(e.to_string()),
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
                error: Some(e.to_string().into()),
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
        Err(e) => err(e.to_string()),
    }
}


// ===== Chat history & conversations =====
//
// These mirror the `ai_chat_*` Tauri commands. Browser mode reaches the same
// SQLite store through them; without these routes the AI panel silently kept no
// history outside the desktop app.

fn conversation_id_of(params: &std::collections::HashMap<String, String>) -> Option<String> {
    params.get("conversationId").or_else(|| params.get("conversation_id")).cloned()
}

pub async fn api_ai_load_history(
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> (
    StatusCode,
    Json<ApiResponse<Vec<crate::commands::ai_chat::AiChatHistoryMessage>>>,
) {
    match crate::commands::ai_chat::ai_chat_load_history(conversation_id_of(&params)).await {
        Ok(messages) => (
            StatusCode::OK,
            Json(ApiResponse { success: true, data: Some(messages), error: None }),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse { success: false, data: None, error: Some(e.to_string().into()) }),
        ),
    }
}

pub async fn api_ai_save_message(
    Json(body): Json<serde_json::Value>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    let message = crate::commands::ai_chat::AiChatHistoryMessage {
        id: body["message"]["id"].as_str().unwrap_or("").to_string(),
        role: body["message"]["role"].as_str().unwrap_or("").to_string(),
        content: body["message"]["content"].as_str().unwrap_or("").to_string(),
    };
    let conversation_id = body["conversationId"]
        .as_str()
        .or_else(|| body["conversation_id"].as_str())
        .map(|s| s.to_string());

    match crate::commands::ai_chat::ai_chat_save_message(message, conversation_id).await {
        Ok(()) => ok(String::new()),
        Err(e) => err(e.to_string()),
    }
}

pub async fn api_ai_clear_history(
    Json(body): Json<serde_json::Value>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    let conversation_id = body["conversationId"]
        .as_str()
        .or_else(|| body["conversation_id"].as_str())
        .map(|s| s.to_string());

    match crate::commands::ai_chat::ai_chat_clear_history(conversation_id).await {
        Ok(()) => ok(String::new()),
        Err(e) => err(e.to_string()),
    }
}

pub async fn api_ai_list_conversations() -> (
    StatusCode,
    Json<ApiResponse<Vec<crate::commands::ai_chat::AiConversation>>>,
) {
    match crate::commands::ai_chat::ai_chat_list_conversations().await {
        Ok(conversations) => (
            StatusCode::OK,
            Json(ApiResponse { success: true, data: Some(conversations), error: None }),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse { success: false, data: None, error: Some(e.to_string().into()) }),
        ),
    }
}

pub async fn api_ai_create_conversation(
    Json(body): Json<serde_json::Value>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    let id = body["id"].as_str().unwrap_or("").to_string();
    let title = body["title"].as_str().unwrap_or("").to_string();

    match crate::commands::ai_chat::ai_chat_create_conversation(id, title).await {
        Ok(()) => ok(String::new()),
        Err(e) => err(e.to_string()),
    }
}

pub async fn api_ai_rename_conversation(
    Json(body): Json<serde_json::Value>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    let id = body["id"].as_str().unwrap_or("").to_string();
    let title = body["title"].as_str().unwrap_or("").to_string();

    match crate::commands::ai_chat::ai_chat_rename_conversation(id, title).await {
        Ok(()) => ok(String::new()),
        Err(e) => err(e.to_string()),
    }
}

pub async fn api_ai_delete_conversation(
    Json(body): Json<serde_json::Value>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    let id = body["id"].as_str().unwrap_or("").to_string();

    match crate::commands::ai_chat::ai_chat_delete_conversation(id).await {
        Ok(()) => ok(String::new()),
        Err(e) => err(e.to_string()),
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
        .map_or_else(
            || exe_path.parent().unwrap_or(&exe_path).to_path_buf(),
            |p| p.join("Resources").join("resources"),
        );

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
