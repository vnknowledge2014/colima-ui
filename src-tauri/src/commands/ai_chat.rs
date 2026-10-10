use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiChatRequest {
    // "anthropic" | "openai" | "openai-compatible" | "gemini" | "ollama-local" | "ollama-cloud"
    pub provider: String,
    pub model: String,
    pub api_key: String,
    pub messages: Vec<ChatMessage>,
    #[serde(default)]
    pub endpoint: String, // base URL for ollama-cloud and openai-compatible
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiChatHistoryMessage {
    pub id: String,
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiConversation {
    pub id: String,
    pub title: String,
    pub updated_at: i64,
    pub message_count: i64,
    /// First user message, trimmed — the list needs a preview even for threads
    /// the user never renamed.
    pub preview: String,
}

/// The thread every message lands in until the user starts a new one. Also the
/// bucket pre-thread history is migrated into, so it must always exist.
pub const DEFAULT_CONVERSATION_ID: &str = "default";

/// Create the row on demand. Messages carry a `conversation_id` the panel picks
/// before the thread has ever been named, so an insert must never fail on a
/// missing parent.
fn ensure_conversation(conn: &rusqlite::Connection, id: &str) -> Result<(), String> {
    conn.execute(
        "INSERT OR IGNORE INTO chat_conversations (id, title) VALUES (?1, '')",
        rusqlite::params![id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn ai_chat_list_conversations() -> Result<Vec<AiConversation>, crate::error::ColimaError> {
    async move {
    let conn = crate::commands::knowledge_bank::get_db().lock().unwrap();
    ensure_conversation(&conn, DEFAULT_CONVERSATION_ID)?;

    let mut stmt = conn
        .prepare(
            "SELECT c.id,
                    c.title,
                    c.updated_at,
                    (SELECT COUNT(*) FROM chat_messages m WHERE m.conversation_id = c.id),
                    COALESCE((SELECT m.content FROM chat_messages m
                              WHERE m.conversation_id = c.id AND m.role = 'user'
                              ORDER BY m.created_at ASC LIMIT 1), '')
             FROM chat_conversations c
             ORDER BY c.updated_at DESC",
        )
        .map_err(|e| e.to_string())?;

    let conversations = stmt
        .query_map([], |row| {
            let preview: String = row.get(4)?;
            Ok(AiConversation {
                id: row.get(0)?,
                title: row.get(1)?,
                updated_at: row.get(2)?,
                message_count: row.get(3)?,
                preview: preview.chars().take(120).collect(),
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .collect();

    Ok(conversations)
    }
    .await.map_err(|e: String| crate::error::ColimaError::from(e))
}

#[tauri::command]
pub async fn ai_chat_create_conversation(id: String, title: String) -> Result<(), crate::error::ColimaError> {
    async move {
    let conn = crate::commands::knowledge_bank::get_db().lock().unwrap();
    conn.execute(
        "INSERT OR IGNORE INTO chat_conversations (id, title) VALUES (?1, ?2)",
        rusqlite::params![id, title],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
    }
    .await.map_err(|e: String| crate::error::ColimaError::from(e))
}

#[tauri::command]
pub async fn ai_chat_rename_conversation(id: String, title: String) -> Result<(), crate::error::ColimaError> {
    async move {
    let conn = crate::commands::knowledge_bank::get_db().lock().unwrap();
    conn.execute(
        "UPDATE chat_conversations SET title = ?2 WHERE id = ?1",
        rusqlite::params![id, title],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
    }
    .await.map_err(|e: String| crate::error::ColimaError::from(e))
}

#[tauri::command]
pub async fn ai_chat_delete_conversation(id: String) -> Result<(), crate::error::ColimaError> {
    async move {
    let conn = crate::commands::knowledge_bank::get_db().lock().unwrap();
    conn.execute(
        "DELETE FROM chat_messages WHERE conversation_id = ?1",
        rusqlite::params![id],
    )
    .map_err(|e| e.to_string())?;
    // The default thread is the landing spot for every new message, so it is
    // emptied rather than removed.
    if id != DEFAULT_CONVERSATION_ID {
        conn.execute(
            "DELETE FROM chat_conversations WHERE id = ?1",
            rusqlite::params![id],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
    }
    .await.map_err(|e: String| crate::error::ColimaError::from(e))
}

#[tauri::command]
pub async fn ai_chat_load_history(
    conversation_id: Option<String>,
) -> Result<Vec<AiChatHistoryMessage>, crate::error::ColimaError> {
    async move {
    let conversation_id = conversation_id.unwrap_or_else(|| DEFAULT_CONVERSATION_ID.to_string());
    let conn = crate::commands::knowledge_bank::get_db().lock().unwrap();
    let mut stmt = conn
        .prepare(
            "SELECT id, role, content FROM chat_messages
             WHERE conversation_id = ?1 ORDER BY created_at ASC",
        )
        .map_err(|e| e.to_string())?;

    let messages = stmt
        .query_map(rusqlite::params![conversation_id], |row| {
            Ok(AiChatHistoryMessage {
                id: row.get(0)?,
                role: row.get(1)?,
                content: row.get(2)?,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .collect();

    Ok(messages)
    }
    .await.map_err(|e: String| crate::error::ColimaError::from(e))
}

#[tauri::command]
pub async fn ai_chat_save_message(
    message: AiChatHistoryMessage,
    conversation_id: Option<String>,
) -> Result<(), crate::error::ColimaError> {
    async move {
    let conversation_id = conversation_id.unwrap_or_else(|| DEFAULT_CONVERSATION_ID.to_string());
    let conn = crate::commands::knowledge_bank::get_db().lock().unwrap();
    ensure_conversation(&conn, &conversation_id)?;
    conn.execute(
        "INSERT INTO chat_messages (id, role, content, conversation_id) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(id) DO UPDATE SET content = excluded.content",
        rusqlite::params![message.id, message.role, message.content, conversation_id],
    )
    .map_err(|e| e.to_string())?;
    // Drives the ordering of the conversation list.
    conn.execute(
        "UPDATE chat_conversations SET updated_at = strftime('%s', 'now') WHERE id = ?1",
        rusqlite::params![conversation_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
    }
    .await.map_err(|e: String| crate::error::ColimaError::from(e))
}

#[tauri::command]
pub async fn ai_chat_clear_history(
    conversation_id: Option<String>,
) -> Result<(), crate::error::ColimaError> {
    async move {
    let conversation_id = conversation_id.unwrap_or_else(|| DEFAULT_CONVERSATION_ID.to_string());
    let conn = crate::commands::knowledge_bank::get_db().lock().unwrap();
    conn.execute(
        "DELETE FROM chat_messages WHERE conversation_id = ?1",
        rusqlite::params![conversation_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
    }
    .await.map_err(|e: String| crate::error::ColimaError::from(e))
}

/// Shared HTTP client — connection pooling across requests
fn http_client() -> reqwest::Client {
    use std::sync::OnceLock;
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT
        .get_or_init(|| {
            reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(120))
                .build()
                .unwrap_or_default()
        })
        .clone()
}

/// Proxy AI chat requests to various LLM providers via reqwest (no subprocess overhead)
#[tauri::command]
pub async fn ai_chat(request: AiChatRequest) -> Result<String, crate::error::ColimaError> {
    async move {
    match request.provider.as_str() {
        "anthropic" => call_anthropic(&request).await,
        // A compatible gateway differs from OpenAI only in its base URL, which
        // `call_openai` already reads from the request.
        "openai" | "openai-compatible" => call_openai(&request).await,
        "gemini" => call_gemini(&request).await,
        "ollama-local" => call_ollama(&request, "http://localhost:11434").await,
        "ollama-cloud" => {
            let endpoint = ollama_cloud_base(&request.endpoint)?;
            call_ollama(&request, &endpoint).await
        }
        _ => Err(format!("Unknown provider: {}", request.provider)),
    }
    }
    .await.map_err(|e: String| crate::error::ColimaError::from(e))
}

/// List available models for a provider dynamically
#[tauri::command]
pub async fn ai_list_models(     provider: String,     api_key: String,     endpoint: String, ) -> Result<String, crate::error::ColimaError> {
    async move {
    match provider.as_str() {
        "ollama-local" => list_ollama_models("http://localhost:11434", "").await,
        "ollama-cloud" => {
            let ep = ollama_cloud_base(&endpoint)?;
            list_ollama_models(&ep, &api_key).await
        }
        "gemini" => list_gemini_models(&api_key, &endpoint).await,
        "anthropic" => list_anthropic_models(&api_key, &endpoint).await,
        "openai" => list_openai_models(&api_key, &endpoint, true).await,
        // A gateway's catalogue is its own — Llama, Mistral, DeepSeek and the
        // rest would all be dropped by the OpenAI-only name filter.
        "openai-compatible" => list_openai_models(&api_key, &endpoint, false).await,
        _ => Ok("[]".to_string()),
    }
    }
    .await.map_err(|e: String| crate::error::ColimaError::from(e))
}

/// Strip the `/api/...` suffix Ollama's own docs quote, so the caller can append
/// its own path exactly once.
///
/// Mirrors `normalizeOllamaBaseUrl` in `src/lib/aiProviderConfig.ts`.
fn normalize_ollama_base(raw: &str) -> String {
    let trimmed = raw.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return String::new();
    }

    let lowered = trimmed.to_ascii_lowercase();
    for suffix in ["/api/chat", "/api/tags", "/api/generate", "/api/show", "/api"] {
        if let Some(prefix) = lowered.strip_suffix(suffix) {
            return trimmed[..prefix.len()].trim_end_matches('/').to_string();
        }
    }

    trimmed.to_string()
}

/// Resolve the host for `ollama-cloud`, refusing to guess one.
///
/// This used to fall back to `http://localhost:11434`, which made the provider
/// indistinguishable from `ollama-local`: a blank field quietly answered from
/// the local daemon, or failed naming a host the user never chose.
fn ollama_cloud_base(endpoint: &str) -> Result<String, String> {
    let base = normalize_ollama_base(endpoint);
    if base.is_empty() {
        return Err(
            "Ollama Cloud needs an Endpoint URL. Set it in Settings → AI Provider (e.g. https://ollama.com)."
                .to_string(),
        );
    }
    Ok(base)
}

/// Reduce a user-supplied endpoint to the base a request path is appended to.
///
/// Mirrors `normalizeOpenAiBaseUrl` in `src/lib/aiProviderConfig.ts`: the
/// streaming path runs in the webview and the non-streaming path runs here, and
/// one endpoint setting feeds both — they cannot disagree about what it means.
fn normalize_openai_base(raw: &str) -> String {
    let trimmed = raw.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return String::new();
    }

    let without_completions = match trimmed.to_ascii_lowercase().strip_suffix("/chat/completions") {
        Some(prefix) => trimmed[..prefix.len()].trim_end_matches('/'),
        None => trimmed,
    };

    // No path after the authority means the version segment was left off.
    let has_path = match without_completions.find("://") {
        Some(scheme_end) => without_completions[scheme_end + 3..].contains('/'),
        None => without_completions.contains('/'),
    };

    if has_path {
        without_completions.to_string()
    } else {
        format!("{}/v1", without_completions)
    }
}

// ===== Model listing =====

async fn list_ollama_models(base_url: &str, api_key: &str) -> Result<String, String> {
    let url = format!("{}/api/tags", base_url);
    let mut req = http_client().get(&url);
    if !api_key.is_empty() {
        req = req.header("Authorization", format!("Bearer {}", api_key));
    }

    let resp = req.send().await.map_err(|e| crate::redact::redact_err("Request failed", e))?;
    let body = resp.text().await.map_err(|e| crate::redact::redact_err("Read error", e))?;

    if body.trim().is_empty() {
        return Ok("[]".to_string());
    }

    let resp: serde_json::Value =
        serde_json::from_str(&body).map_err(|e| crate::redact::redact_err("JSON parse error", e))?;

    let models: Vec<String> = resp["models"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|m| m["name"].as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    serde_json::to_string(&models).map_err(|e| crate::redact::redact_err("Serialize error", e))
}

async fn list_gemini_models(api_key: &str, endpoint: &str) -> Result<String, String> {
    let base = if endpoint.is_empty() {
        "https://generativelanguage.googleapis.com/v1beta"
    } else {
        endpoint.trim_end_matches('/')
    };
    // The key travels in a header, never the URL. reqwest embeds the full URL
    // in its error Display, so a key in the query string ends up in the toast
    // the user copies into a bug report.
    let url = format!("{}/models", base);

    let resp = http_client()
        .get(&url)
        .header("x-goog-api-key", api_key)
        .send()
        .await
        .map_err(|e| crate::redact::redact_err("Request failed", e))?;
    let body = resp
        .text()
        .await
        .map_err(|e| crate::redact::redact_err("Read error", e))?;

    if body.trim().is_empty() {
        return Ok("[]".to_string());
    }

    let resp: serde_json::Value =
        serde_json::from_str(&body).map_err(|e| crate::redact::redact_err("JSON parse error", e))?;

    let models: Vec<String> = resp["models"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|m| {
                    let name = m["name"].as_str()?;
                    let short = name.strip_prefix("models/").unwrap_or(name);
                    let methods = m["supportedGenerationMethods"].as_array()?;
                    if methods
                        .iter()
                        .any(|v| v.as_str() == Some("generateContent"))
                    {
                        Some(short.to_string())
                    } else {
                        None
                    }
                })
                .collect()
        })
        .unwrap_or_default();

    serde_json::to_string(&models).map_err(|e| crate::redact::redact_err("Serialize error", e))
}

async fn list_anthropic_models(api_key: &str, endpoint: &str) -> Result<String, String> {
    if api_key.is_empty() {
        return Ok("[]".to_string());
    }
    
    let base = if endpoint.is_empty() {
        "https://api.anthropic.com/v1"
    } else {
        endpoint.trim_end_matches('/')
    };
    let url = format!("{}/models", base);

    let resp = http_client()
        .get(&url)
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .send()
        .await
        .map_err(|e| crate::redact::redact_err("Request failed", e))?;
    let body = resp.text().await.map_err(|e| crate::redact::redact_err("Read error", e))?;

    if body.trim().is_empty() {
        return Ok("[]".to_string());
    }

    let resp: serde_json::Value =
        serde_json::from_str(&body).map_err(|e| crate::redact::redact_err("JSON parse error", e))?;

    let models: Vec<String> = resp["data"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|m| m["id"].as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    serde_json::to_string(&models).map_err(|e| crate::redact::redact_err("Serialize error", e))
}

/// List models from an OpenAI-shaped `/v1/models` catalogue.
///
/// `only_official` keeps the OpenAI name filter; a compatible gateway serves
/// models under any vendor's name, so filtering there returns nothing.
async fn list_openai_models(
    api_key: &str,
    endpoint: &str,
    only_official: bool,
) -> Result<String, String> {
    // A self-hosted gateway needs no credential, so the key is only mandatory
    // when the request is going to OpenAI itself.
    if api_key.is_empty() && endpoint.is_empty() {
        return Ok("[]".to_string());
    }

    let base = if endpoint.is_empty() {
        "https://api.openai.com/v1".to_string()
    } else {
        normalize_openai_base(endpoint)
    };
    let url = format!("{}/models", base);

    let mut req = http_client().get(&url);
    if !api_key.is_empty() {
        req = req.header("Authorization", format!("Bearer {}", api_key));
    }

    let resp = req
        .send()
        .await
        .map_err(|e| crate::redact::redact_err("Request failed", e))?;
    let body = resp.text().await.map_err(|e| crate::redact::redact_err("Read error", e))?;

    if body.trim().is_empty() {
        return Ok("[]".to_string());
    }

    let resp: serde_json::Value =
        serde_json::from_str(&body).map_err(|e| crate::redact::redact_err("JSON parse error", e))?;

    let mut models: Vec<String> = resp["data"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|m| {
                    let id = m["id"].as_str()?;
                    if !only_official
                        || id.starts_with("gpt-")
                        || id.starts_with("o1")
                        || id.starts_with("o3")
                        || id.starts_with("o4")
                        || id.starts_with("chatgpt")
                    {
                        Some(id.to_string())
                    } else {
                        None
                    }
                })
                .collect()
        })
        .unwrap_or_default();

    models.sort();
    models.reverse(); // newest first (gpt-5 before gpt-4)

    serde_json::to_string(&models).map_err(|e| crate::redact::redact_err("Serialize error", e))
}

// ===== Chat implementations =====

async fn call_anthropic(req: &AiChatRequest) -> Result<String, String> {
    let messages: Vec<serde_json::Value> = req
        .messages
        .iter()
        .filter(|m| m.role != "system")
        .map(|m| {
            serde_json::json!({
                "role": m.role,
                "content": m.content
            })
        })
        .collect();

    let system_msg = req
        .messages
        .iter()
        .find(|m| m.role == "system")
        .map(|m| m.content.clone())
        .unwrap_or_default();

    let body = serde_json::json!({
        "model": req.model,
        "max_tokens": 4096,
        "system": system_msg,
        "messages": messages
    });

    let base = if req.endpoint.is_empty() {
        "https://api.anthropic.com/v1"
    } else {
        req.endpoint.trim_end_matches('/')
    };
    let url = format!("{}/messages", base);

    let resp = http_client()
        .post(&url)
        .header("Content-Type", "application/json")
        .header("x-api-key", &req.api_key)
        .header("anthropic-version", "2023-06-01")
        .json(&body)
        .send()
        .await
        .map_err(|e| crate::redact::redact_err("Request failed", e))?;

    let resp_body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| crate::redact::redact_err("JSON parse error", e))?;

    if let Some(err) = resp_body.get("error") {
        return Err(format!(
            "Anthropic error: {}",
            err["message"].as_str().unwrap_or("unknown error")
        ));
    }

    resp_body["content"]
        .as_array()
        .and_then(|blocks| {
            blocks
                .iter()
                .filter(|b| b["type"] == "text")
                .map(|b| b["text"].as_str().unwrap_or(""))
                .collect::<Vec<_>>()
                .first()
                .map(|s| s.to_string())
        })
        .ok_or_else(|| "No response content from Anthropic".to_string())
}

async fn call_openai(req: &AiChatRequest) -> Result<String, String> {
    let messages: Vec<serde_json::Value> = req
        .messages
        .iter()
        .map(|m| {
            serde_json::json!({
                "role": m.role,
                "content": m.content
            })
        })
        .collect();

    let body = serde_json::json!({
        "model": req.model,
        "messages": messages,
        "max_tokens": 4096
    });

    let base = if req.endpoint.is_empty() {
        "https://api.openai.com/v1".to_string()
    } else {
        normalize_openai_base(&req.endpoint)
    };
    let url = format!("{}/chat/completions", base);

    let resp = http_client()
        .post(&url)
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", req.api_key))
        .json(&body)
        .send()
        .await
        .map_err(|e| crate::redact::redact_err("Request failed", e))?;

    let resp_body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| crate::redact::redact_err("JSON parse error", e))?;

    if let Some(err) = resp_body.get("error") {
        return Err(format!(
            "OpenAI error: {}",
            err["message"].as_str().unwrap_or("unknown error")
        ));
    }

    resp_body["choices"][0]["message"]["content"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| "No response content from OpenAI".to_string())
}

async fn call_gemini(req: &AiChatRequest) -> Result<String, String> {
    let mut contents: Vec<serde_json::Value> = Vec::new();
    let mut system_instruction = String::new();

    for msg in &req.messages {
        match msg.role.as_str() {
            "system" => {
                system_instruction = msg.content.clone();
            }
            "user" => {
                contents.push(serde_json::json!({
                    "role": "user",
                    "parts": [{"text": msg.content}]
                }));
            }
            "assistant" => {
                contents.push(serde_json::json!({
                    "role": "model",
                    "parts": [{"text": msg.content}]
                }));
            }
            _ => {}
        }
    }

    let mut body = serde_json::json!({
        "contents": contents,
        "generationConfig": {
            "maxOutputTokens": 4096
        }
    });

    if !system_instruction.is_empty() {
        body["systemInstruction"] = serde_json::json!({
            "parts": [{"text": system_instruction}]
        });
    }

    let base = if req.endpoint.is_empty() {
        "https://generativelanguage.googleapis.com/v1beta"
    } else {
        req.endpoint.trim_end_matches('/')
    };
    // Key in a header, not the query string — see list_gemini_models.
    let url = format!("{}/models/{}:generateContent", base, req.model);

    let resp = http_client()
        .post(&url)
        .header("Content-Type", "application/json")
        .header("x-goog-api-key", &req.api_key)
        .json(&body)
        .send()
        .await
        .map_err(|e| crate::redact::redact_err("Request failed", e))?;

    let resp_body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| crate::redact::redact_err("JSON parse error", e))?;

    if let Some(err) = resp_body.get("error") {
        return Err(format!(
            "Gemini error: {}",
            err["message"].as_str().unwrap_or("unknown error")
        ));
    }

    resp_body["candidates"][0]["content"]["parts"][0]["text"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| "No response content from Gemini".to_string())
}

async fn call_ollama(req: &AiChatRequest, base_url: &str) -> Result<String, String> {
    let messages: Vec<serde_json::Value> = req
        .messages
        .iter()
        .map(|m| {
            serde_json::json!({
                "role": m.role,
                "content": m.content
            })
        })
        .collect();

    let body = serde_json::json!({
        "model": req.model,
        "messages": messages,
        "stream": false
    });

    let url = format!("{}/api/chat", base_url);

    let mut request = http_client()
        .post(&url)
        .header("Content-Type", "application/json");

    // Add auth if API key provided (for cloud Ollama)
    if !req.api_key.is_empty() {
        request = request.header("Authorization", format!("Bearer {}", req.api_key));
    }

    let resp = request
        .json(&body)
        .send()
        .await
        .map_err(|e| crate::redact::redact_err("Request failed (is the server running?)", e))?;

    let resp_text = resp.text().await.map_err(|e| crate::redact::redact_err("Read error", e))?;

    if resp_text.trim().is_empty() {
        return Err("Empty response from Ollama — is the server running?".to_string());
    }

    let resp_body: serde_json::Value = serde_json::from_str(&resp_text).map_err(|e| {
        // Take chars, not bytes: byte slicing panics mid-codepoint on UTF-8.
        // The body is redacted because a server can echo credentials back.
        let preview: String = resp_text.chars().take(200).collect();
        crate::redact::redact(&format!("JSON parse error: {} — raw: {}", e, preview))
    })?;

    if let Some(err) = resp_body.get("error") {
        return Err(format!(
            "Ollama error: {}",
            err.as_str().unwrap_or("unknown")
        ));
    }

    resp_body["message"]["content"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| {
            let preview: String = resp_text.chars().take(200).collect();
            crate::redact::redact(&format!("No response content from Ollama — raw: {}", preview))
        })
}

#[cfg(test)]
mod tests {
    use super::{normalize_ollama_base, ollama_cloud_base, normalize_openai_base};

    #[test]
    fn an_ollama_host_keeps_one_api_path_however_it_was_pasted() {
        for input in [
            "https://ollama.com",
            "https://ollama.com/",
            "https://ollama.com/api",
            "https://ollama.com/api/chat",
            "https://ollama.com/api/tags",
            "  https://ollama.com/api/chat  ",
        ] {
            assert_eq!(normalize_ollama_base(input), "https://ollama.com", "input: {input:?}");
        }
        // A port is part of the host, not a path to strip.
        assert_eq!(
            normalize_ollama_base("http://192.168.1.10:11434/api/chat"),
            "http://192.168.1.10:11434"
        );
    }

    #[test]
    fn ollama_cloud_refuses_to_guess_a_host_instead_of_using_the_local_daemon() {
        // The old fallback answered from localhost:11434, making this provider
        // silently identical to ollama-local.
        let err = ollama_cloud_base("").expect_err("a blank endpoint must not resolve");
        assert!(err.contains("Endpoint URL"), "unhelpful message: {err}");
        assert!(!err.contains("localhost"), "must not point at the local daemon: {err}");

        assert!(ollama_cloud_base("   ").is_err());
        assert_eq!(ollama_cloud_base("https://ollama.com/api").unwrap(), "https://ollama.com");
    }

    /// The same table is asserted in `src/lib/aiProviderConfig.test.ts`. The two
    /// implementations feed one endpoint setting and must not diverge.
    #[test]
    fn every_form_of_a_proxy_url_reduces_to_one_base() {
        for input in [
            "http://localhost:4000",
            "http://localhost:4000/",
            "http://localhost:4000/v1",
            "http://localhost:4000/v1/",
            "http://localhost:4000/v1/chat/completions",
            "  http://localhost:4000/v1  ",
        ] {
            assert_eq!(normalize_openai_base(input), "http://localhost:4000/v1", "input: {input:?}");
        }
    }

    #[test]
    fn a_gateway_mounted_under_a_path_keeps_it() {
        assert_eq!(
            normalize_openai_base("https://openrouter.ai/api/v1"),
            "https://openrouter.ai/api/v1"
        );
        assert_eq!(
            normalize_openai_base("https://api.groq.com/openai/v1/chat/completions"),
            "https://api.groq.com/openai/v1"
        );
    }

    #[test]
    fn an_empty_endpoint_stays_empty_so_the_caller_picks_its_default() {
        assert_eq!(normalize_openai_base(""), "");
        assert_eq!(normalize_openai_base("   "), "");
    }
}
