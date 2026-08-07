use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::commands::colima;

#[derive(Debug, Serialize, Deserialize)]
pub struct HeadlessChatRequest {
    pub prompt: String,
    pub provider: Option<String>,
    pub model: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

// Giả lập luồng Agent Loop trong Rust
#[tauri::command]
pub async fn run_headless_agent(
    req: HeadlessChatRequest,
) -> Result<String, String> {
    // 1. Lấy thông tin cấu hình AI
    let _provider = req.provider.clone().unwrap_or_else(|| {
        "openai".to_string()
    });
    let _model = req.model.clone().unwrap_or_else(|| {
        "gpt-3.5-turbo".to_string()
    });
    
    // 2. Nạp system prompt (Giản lược cho Headless)
    let system_prompt = "Bạn là ColimaUI Agent. Hãy phân tích yêu cầu của người dùng. Nếu cần gọi tool, hãy in ra [QUERY: event-name | json_payload]. Nếu cần cấp quyền, in ra [EVENT_APPROVE: ...]. Hãy ngắn gọn.".to_string();
    
    let mut messages = vec![
        ChatMessage {
            role: "system".to_string(),
            content: system_prompt,
        },
        ChatMessage {
            role: "user".to_string(),
            content: req.prompt.clone(),
        }
    ];

    let mut rounds = 0;
    let max_rounds = 3; // Giới hạn số lần gọi tool liên tiếp để tránh infinite loop

    // Vòng lặp Agent Loop
    while rounds < max_rounds {
        // 3. Gọi hàm LLM client từ ai_chat.rs (hiện tại ai_chat đang là placeholder, ta giả lập gọi)
        // Trong thực tế sẽ gọi crate::commands::ai_chat::ai_chat(...)
        // Nhưng hiện tại ta sẽ tạo phản hồi giả để chứng minh luồng
        let llm_response = if req.prompt.contains("list-containers") && rounds == 0 {
            "[QUERY: list-containers]".to_string()
        } else if req.prompt.contains("status") && rounds == 0 {
            "[QUERY: colima-status | {\"profile\": \"default\"}]".to_string()
        } else {
            "Đã hoàn thành tác vụ theo yêu cầu.".to_string()
        };

        messages.push(ChatMessage {
            role: "assistant".to_string(),
            content: llm_response.clone(),
        });

        // 4. Parse output để tìm [QUERY: ...]
        if let Some(query_start) = llm_response.find("[QUERY:") {
            let start_idx = query_start + 7;
            if let Some(end_idx) = llm_response[start_idx..].find(']') {
                let inner = &llm_response[start_idx..start_idx + end_idx].trim();
                let parts: Vec<&str> = inner.split('|').collect();
                let event_name = parts[0].trim();
                let payload_str = if parts.len() > 1 { parts[1].trim() } else { "{}" };
                
                let payload: Value = serde_json::from_str(payload_str).unwrap_or(serde_json::json!({}));
                
                // 5. Route tới Rust function tương ứng (Phase 3 Event Router)
                let tool_result = route_event(event_name, payload).await;
                
                messages.push(ChatMessage {
                    role: "system".to_string(),
                    content: format!("Tool result: {:?}", tool_result),
                });
                
                rounds += 1;
                continue; // Lặp lại để LLM phân tích kết quả
            }
        }
        
        // Không có QUERY, kết thúc loop
        return Ok(llm_response);
    }

    Ok("Max agent rounds reached.".to_string())
}

// Đây là Trái tim của Phase 3: Rust Event Router (Minimal Implementation)
async fn route_event(event: &str, payload: Value) -> Result<String, String> {
    match event {
        "list-containers" => {
            // For headless, we return a mock or call the appropriate API
            Ok(serde_json::json!({"status": "List containers not fully ported"}).to_string())
        }
        "colima-status" => {
            let profile = payload["profile"].as_str().unwrap_or("default");
            let status = colima::instance_status(profile.to_string()).await.map_err(|e| e.to_string())?;
            Ok(serde_json::to_string(&status).unwrap_or_default())
        }
        "start-instance" => {
            let profile = payload["profile"].as_str().unwrap_or("default");
            let config = crate::commands::colima::StartConfig {
                profile: profile.to_string(),
                runtime: "docker".to_string(),
                cpus: 2,
                memory: 2,
                disk: 60,
                vm_type: "vz".to_string(),
                kubernetes: false,
                kubernetes_version: "".to_string(),
                arch: "".to_string(),
                mount_type: "".to_string(),
                mounts: vec![],
                dns: vec![],
                network_address: false,
            };
            colima::start_instance(config).await
        }
        _ => {
            // Đối với 90+ events còn lại, hiện tại trả về lỗi để Orchestrator dùng API trực tiếp
            Err(format!("Event '{}' chưa được port sang Rust Agent Loop. Vui lòng dùng Direct REST API.", event))
        }
    }
}
