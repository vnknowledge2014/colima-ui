use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiChatRequest {
    pub provider: String, // "anthropic" | "openai" | "gemini" | "ollama-local" | "ollama-cloud"
    pub model: String,
    pub api_key: String,
    pub messages: Vec<ChatMessage>,
    #[serde(default)]
    pub endpoint: String, // custom endpoint for ollama-cloud
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
pub async fn ai_chat(request: AiChatRequest) -> Result<String, String> {
    match request.provider.as_str() {
        "anthropic" => call_anthropic(&request).await,
        "openai" => call_openai(&request).await,
        "gemini" => call_gemini(&request).await,
        "ollama-local" => call_ollama(&request, "http://localhost:11434").await,
        "ollama-cloud" => {
            let endpoint = if request.endpoint.is_empty() {
                "http://localhost:11434".to_string()
            } else {
                request.endpoint.trim_end_matches('/').to_string()
            };
            call_ollama(&request, &endpoint).await
        }
        _ => Err(format!("Unknown provider: {}", request.provider)),
    }
}

/// List available models for a provider dynamically
#[tauri::command]
pub async fn ai_list_models(
    provider: String,
    api_key: String,
    endpoint: String,
) -> Result<String, String> {
    match provider.as_str() {
        "ollama-local" => list_ollama_models("http://localhost:11434", "").await,
        "ollama-cloud" => {
            let ep = if endpoint.is_empty() {
                "http://localhost:11434".to_string()
            } else {
                endpoint.trim_end_matches('/').to_string()
            };
            list_ollama_models(&ep, &api_key).await
        }
        "gemini" => list_gemini_models(&api_key).await,
        "anthropic" => list_anthropic_models(&api_key).await,
        "openai" => list_openai_models(&api_key).await,
        _ => Ok("[]".to_string()),
    }
}

// ===== Model listing =====

async fn list_ollama_models(base_url: &str, api_key: &str) -> Result<String, String> {
    let url = format!("{}/api/tags", base_url);
    let mut req = http_client().get(&url);
    if !api_key.is_empty() {
        req = req.header("Authorization", format!("Bearer {}", api_key));
    }

    let resp = req.send().await.map_err(|e| format!("Request failed: {}", e))?;
    let body = resp.text().await.map_err(|e| format!("Read error: {}", e))?;

    if body.trim().is_empty() {
        return Ok("[]".to_string());
    }

    let resp: serde_json::Value =
        serde_json::from_str(&body).map_err(|e| format!("JSON parse error: {}", e))?;

    let models: Vec<String> = resp["models"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|m| m["name"].as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    serde_json::to_string(&models).map_err(|e| format!("Serialize error: {}", e))
}

async fn list_gemini_models(api_key: &str) -> Result<String, String> {
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models?key={}",
        api_key
    );

    let resp = http_client()
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;
    let body = resp.text().await.map_err(|e| format!("Read error: {}", e))?;

    if body.trim().is_empty() {
        return Ok("[]".to_string());
    }

    let resp: serde_json::Value =
        serde_json::from_str(&body).map_err(|e| format!("JSON parse error: {}", e))?;

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

    serde_json::to_string(&models).map_err(|e| format!("Serialize error: {}", e))
}

async fn list_anthropic_models(api_key: &str) -> Result<String, String> {
    if api_key.is_empty() {
        return Ok("[]".to_string());
    }

    let resp = http_client()
        .get("https://api.anthropic.com/v1/models")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;
    let body = resp.text().await.map_err(|e| format!("Read error: {}", e))?;

    if body.trim().is_empty() {
        return Ok("[]".to_string());
    }

    let resp: serde_json::Value =
        serde_json::from_str(&body).map_err(|e| format!("JSON parse error: {}", e))?;

    let models: Vec<String> = resp["data"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|m| m["id"].as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    serde_json::to_string(&models).map_err(|e| format!("Serialize error: {}", e))
}

async fn list_openai_models(api_key: &str) -> Result<String, String> {
    if api_key.is_empty() {
        return Ok("[]".to_string());
    }

    let resp = http_client()
        .get("https://api.openai.com/v1/models")
        .header("Authorization", format!("Bearer {}", api_key))
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;
    let body = resp.text().await.map_err(|e| format!("Read error: {}", e))?;

    if body.trim().is_empty() {
        return Ok("[]".to_string());
    }

    let resp: serde_json::Value =
        serde_json::from_str(&body).map_err(|e| format!("JSON parse error: {}", e))?;

    let mut models: Vec<String> = resp["data"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|m| {
                    let id = m["id"].as_str()?;
                    if id.starts_with("gpt-")
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

    serde_json::to_string(&models).map_err(|e| format!("Serialize error: {}", e))
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

    let resp = http_client()
        .post("https://api.anthropic.com/v1/messages")
        .header("Content-Type", "application/json")
        .header("x-api-key", &req.api_key)
        .header("anthropic-version", "2023-06-01")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    let resp_body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("JSON parse error: {}", e))?;

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

    let resp = http_client()
        .post("https://api.openai.com/v1/chat/completions")
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", req.api_key))
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    let resp_body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("JSON parse error: {}", e))?;

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

    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
        req.model, req.api_key
    );

    let resp = http_client()
        .post(&url)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    let resp_body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("JSON parse error: {}", e))?;

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
        .map_err(|e| format!("Request failed: {} — is the server running?", e))?;

    let resp_text = resp.text().await.map_err(|e| format!("Read error: {}", e))?;

    if resp_text.trim().is_empty() {
        return Err("Empty response from Ollama — is the server running?".to_string());
    }

    let resp_body: serde_json::Value = serde_json::from_str(&resp_text).map_err(|e| {
        format!(
            "JSON parse error: {} — raw: {}",
            e,
            &resp_text[..resp_text.len().min(200)]
        )
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
            format!(
                "No response content from Ollama — raw: {}",
                &resp_text[..resp_text.len().min(200)]
            )
        })
}
