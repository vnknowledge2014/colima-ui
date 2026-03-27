use serde::{Deserialize, Serialize};
use std::process::Command;

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

/// Proxy AI chat requests to various LLM providers via curl
#[tauri::command]
pub async fn ai_chat(request: AiChatRequest) -> Result<String, String> {
    match request.provider.as_str() {
        "anthropic" => call_anthropic(&request),
        "openai" => call_openai(&request),
        "gemini" => call_gemini(&request),
        "ollama-local" => call_ollama(&request, "http://localhost:11434"),
        "ollama-cloud" => {
            let endpoint = if request.endpoint.is_empty() {
                "http://localhost:11434".to_string()
            } else {
                request.endpoint.trim_end_matches('/').to_string()
            };
            call_ollama(&request, &endpoint)
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
        "ollama-local" => list_ollama_models("http://localhost:11434", ""),
        "ollama-cloud" => {
            let ep = if endpoint.is_empty() {
                "http://localhost:11434".to_string()
            } else {
                endpoint.trim_end_matches('/').to_string()
            };
            list_ollama_models(&ep, &api_key)
        }
        "gemini" => list_gemini_models(&api_key),
        "anthropic" => list_anthropic_models(&api_key),
        "openai" => list_openai_models(&api_key),
        _ => Ok("[]".to_string()),
    }
}

fn list_ollama_models(base_url: &str, api_key: &str) -> Result<String, String> {
    let url = format!("{}/api/tags", base_url);
    let mut args = vec!["-s".to_string(), url];
    if !api_key.is_empty() {
        args.push("-H".to_string());
        args.push(format!("Authorization: Bearer {}", api_key));
    }

    let output = Command::new("curl")
        .args(&args)
        .output()
        .map_err(|e| format!("curl failed: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.trim().is_empty() {
        return Ok("[]".to_string());
    }

    let resp: serde_json::Value =
        serde_json::from_str(&stdout).map_err(|e| format!("JSON parse error: {}", e))?;

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

fn list_gemini_models(api_key: &str) -> Result<String, String> {
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models?key={}",
        api_key
    );

    let output = Command::new("curl")
        .args(["-s", &url])
        .output()
        .map_err(|e| format!("curl failed: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.trim().is_empty() {
        return Ok("[]".to_string());
    }

    let resp: serde_json::Value =
        serde_json::from_str(&stdout).map_err(|e| format!("JSON parse error: {}", e))?;

    let models: Vec<String> = resp["models"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|m| {
                    let name = m["name"].as_str()?;
                    // Filter to generative models only, strip "models/" prefix
                    let short = name.strip_prefix("models/").unwrap_or(name);
                    // Only include generateContent-capable models
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

fn list_anthropic_models(api_key: &str) -> Result<String, String> {
    if api_key.is_empty() {
        return Ok("[]".to_string());
    }

    let output = Command::new("curl")
        .args([
            "-s",
            "https://api.anthropic.com/v1/models",
            "-H",
            &format!("x-api-key: {}", api_key),
            "-H",
            "anthropic-version: 2023-06-01",
        ])
        .output()
        .map_err(|e| format!("curl failed: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.trim().is_empty() {
        return Ok("[]".to_string());
    }

    let resp: serde_json::Value =
        serde_json::from_str(&stdout).map_err(|e| format!("JSON parse error: {}", e))?;

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

fn list_openai_models(api_key: &str) -> Result<String, String> {
    if api_key.is_empty() {
        return Ok("[]".to_string());
    }

    let output = Command::new("curl")
        .args([
            "-s",
            "https://api.openai.com/v1/models",
            "-H",
            &format!("Authorization: Bearer {}", api_key),
        ])
        .output()
        .map_err(|e| format!("curl failed: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.trim().is_empty() {
        return Ok("[]".to_string());
    }

    let resp: serde_json::Value =
        serde_json::from_str(&stdout).map_err(|e| format!("JSON parse error: {}", e))?;

    let mut models: Vec<String> = resp["data"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|m| {
                    let id = m["id"].as_str()?;
                    // Filter to chat-capable models (gpt, o1, o3, o4, chatgpt)
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

fn call_anthropic(req: &AiChatRequest) -> Result<String, String> {
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

    let output = Command::new("curl")
        .args([
            "-s",
            "-X",
            "POST",
            "https://api.anthropic.com/v1/messages",
            "-H",
            "Content-Type: application/json",
            "-H",
            &format!("x-api-key: {}", req.api_key),
            "-H",
            "anthropic-version: 2023-06-01",
            "-d",
            &body.to_string(),
        ])
        .output()
        .map_err(|e| format!("curl failed: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "Anthropic API error: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let resp: serde_json::Value =
        serde_json::from_slice(&output.stdout).map_err(|e| format!("JSON parse error: {}", e))?;

    if let Some(err) = resp.get("error") {
        return Err(format!(
            "Anthropic error: {}",
            err["message"].as_str().unwrap_or("unknown error")
        ));
    }

    // Extract text from content blocks
    resp["content"]
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

fn call_openai(req: &AiChatRequest) -> Result<String, String> {
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

    let output = Command::new("curl")
        .args([
            "-s",
            "-X",
            "POST",
            "https://api.openai.com/v1/chat/completions",
            "-H",
            "Content-Type: application/json",
            "-H",
            &format!("Authorization: Bearer {}", req.api_key),
            "-d",
            &body.to_string(),
        ])
        .output()
        .map_err(|e| format!("curl failed: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "OpenAI API error: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let resp: serde_json::Value =
        serde_json::from_slice(&output.stdout).map_err(|e| format!("JSON parse error: {}", e))?;

    if let Some(err) = resp.get("error") {
        return Err(format!(
            "OpenAI error: {}",
            err["message"].as_str().unwrap_or("unknown error")
        ));
    }

    resp["choices"][0]["message"]["content"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| "No response content from OpenAI".to_string())
}

fn call_gemini(req: &AiChatRequest) -> Result<String, String> {
    // Convert messages to Gemini format (contents array)
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

    let output = Command::new("curl")
        .args([
            "-s",
            "-X",
            "POST",
            &url,
            "-H",
            "Content-Type: application/json",
            "-d",
            &body.to_string(),
        ])
        .output()
        .map_err(|e| format!("curl failed: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "Gemini API error: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let resp: serde_json::Value =
        serde_json::from_slice(&output.stdout).map_err(|e| format!("JSON parse error: {}", e))?;

    if let Some(err) = resp.get("error") {
        return Err(format!(
            "Gemini error: {}",
            err["message"].as_str().unwrap_or("unknown error")
        ));
    }

    resp["candidates"][0]["content"]["parts"][0]["text"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| "No response content from Gemini".to_string())
}

fn call_ollama(req: &AiChatRequest, base_url: &str) -> Result<String, String> {
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

    let mut args = vec![
        "-s".to_string(),
        "-X".to_string(),
        "POST".to_string(),
        url,
        "-H".to_string(),
        "Content-Type: application/json".to_string(),
    ];

    // Add auth if API key provided (for cloud Ollama)
    if !req.api_key.is_empty() {
        args.push("-H".to_string());
        args.push(format!("Authorization: Bearer {}", req.api_key));
    }

    args.push("-d".to_string());
    args.push(body.to_string());

    let output = Command::new("curl")
        .args(&args)
        .output()
        .map_err(|e| format!("curl failed: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "Ollama API error: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.trim().is_empty() {
        return Err("Empty response from Ollama — is the server running?".to_string());
    }

    let resp: serde_json::Value = serde_json::from_str(&stdout).map_err(|e| {
        format!(
            "JSON parse error: {} — raw: {}",
            e,
            &stdout[..stdout.len().min(200)]
        )
    })?;

    if let Some(err) = resp.get("error") {
        return Err(format!(
            "Ollama error: {}",
            err.as_str().unwrap_or("unknown")
        ));
    }

    resp["message"]["content"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| {
            format!(
                "No response content from Ollama — raw: {}",
                &stdout[..stdout.len().min(200)]
            )
        })
}
