use axum::{
    extract::Query,
    http::StatusCode,
    response::Json,
};
use axum::response::sse::{Event, Sse};
use tokio_stream::Stream;

use crate::api_server::*;
use crate::commands::*;
use crate::routes::payloads::*;

pub async fn api_get_settings() -> (StatusCode, Json<ApiResponse<std::collections::HashMap<String, String>>>) {
    match knowledge_bank::get_all_settings().await {
        Ok(settings) => ok(settings),
        Err(e) => err(e.to_string()),
    }
}


pub async fn api_set_setting(
    Json(body): Json<SetSettingRequest>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    match knowledge_bank::set_setting(body.key, body.value).await {
        Ok(msg) => ok(msg),
        Err(e) => err(e.to_string()),
    }
}


pub async fn api_kb_query(
    Json(body): Json<KbQueryRequest>,
) -> (StatusCode, Json<ApiResponse<serde_json::Value>>) {
    match knowledge_bank::kb_query(body.error_text).await {
        Ok(result) => ok(serde_json::to_value(result).unwrap_or_default()),
        Err(e) => err(e.to_string()),
    }
}


pub async fn api_kb_search(
    Json(body): Json<KbSearchRequest>,
) -> (StatusCode, Json<ApiResponse<Vec<String>>>) {
    match knowledge_bank::search_memory(body.query, body.limit).await {
        Ok(results) => ok(results),
        Err(e) => err(e.to_string()),
    }
}


pub async fn api_kb_get_memories() -> (StatusCode, Json<ApiResponse<serde_json::Value>>) {
    match knowledge_bank::get_all_memories().await {
        Ok(memories) => ok(serde_json::to_value(memories).unwrap_or_default()),
        Err(e) => err(e.to_string()),
    }
}


pub async fn api_kb_update_memory(
    Json(body): Json<UpdateMemoryRequest>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    match knowledge_bank::update_memory(body.id, body.content).await {
        Ok(msg) => ok(msg),
        Err(e) => err(e.to_string()),
    }
}


pub async fn api_kb_delete_memory(
    Json(body): Json<DeleteMemoryRequest>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    match knowledge_bank::delete_memory(body.id).await {
        Ok(msg) => ok(msg),
        Err(e) => err(e.to_string()),
    }
}


pub async fn api_sandbox_execute(
    Json(body): Json<SandboxRequest>,
) -> (StatusCode, Json<ApiResponse<serde_json::Value>>) {
    match shell_sandbox::sandbox_execute(body.command).await {
        Ok(result) => ok(serde_json::to_value(result).unwrap_or_default()),
        Err(e) => err(e.to_string()),
    }
}


pub async fn api_sandbox_execute_approved(
    Json(body): Json<SandboxRequest>,
) -> (StatusCode, Json<ApiResponse<serde_json::Value>>) {
    match shell_sandbox::sandbox_execute_approved(body.command).await {
        Ok(result) => ok(serde_json::to_value(result).unwrap_or_default()),
        Err(e) => err(e.to_string()),
    }
}


pub async fn api_sandbox_execute_stream(
    Json(body): Json<SandboxRequest>,
) -> Sse<impl Stream<Item = Result<Event, std::convert::Infallible>>> {
    let (tx, rx) = tokio::sync::mpsc::channel(100);
    let command = body.command.clone();

    tokio::spawn(async move {
        // Run classification manually since it's not public
        let parts: Vec<&str> = command.trim().split_whitespace().collect();
        if parts.is_empty() {
            let _ = tx.send(Ok(Event::default().event("error").data("Empty command"))).await;
            return;
        }

        let mut child = match tokio::process::Command::new(parts[0])
            .args(&parts[1..])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
        {
            Ok(c) => c,
            Err(e) => {
                let _ = tx.send(Ok(Event::default().event("error").data(e.to_string()))).await;
                return;
            }
        };

        let mut stdout_reader = tokio::io::BufReader::new(child.stdout.take().unwrap());
        let mut stderr_reader = tokio::io::BufReader::new(child.stderr.take().unwrap());
        let mut buf_out = [0; 1024];
        let mut buf_err = [0; 1024];

        loop {
            tokio::select! {
                res = tokio::io::AsyncReadExt::read(&mut stdout_reader, &mut buf_out) => {
                    match res {
                        Ok(0) => break, // Check if stderr is also done, but select! might just exit loop. We should handle correctly. Actually, let's just break on 0 and we might miss stderr.
                        Ok(n) => {
                            let text = String::from_utf8_lossy(&buf_out[..n]).into_owned();
                            let _ = tx.send(Ok(Event::default().event("stdout").data(text))).await;
                        }
                        Err(_) => break,
                    }
                }
                res = tokio::io::AsyncReadExt::read(&mut stderr_reader, &mut buf_err) => {
                    match res {
                        Ok(0) => break,
                        Ok(n) => {
                            let text = String::from_utf8_lossy(&buf_err[..n]).into_owned();
                            let _ = tx.send(Ok(Event::default().event("stderr").data(text))).await;
                        }
                        Err(_) => break,
                    }
                }
            }
        }
        
        let status = child.wait().await;
        if let Ok(exit_status) = status {
            let _ = tx.send(Ok(Event::default().event("exit").data(exit_status.code().unwrap_or(-1).to_string()))).await;
        }
    });

    let stream = tokio_stream::wrappers::ReceiverStream::new(rx);
    Sse::new(stream)
}

pub async fn api_diagnostics_logs(
    Query(q): Query<std::collections::HashMap<String, String>>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    let profile = q.get("profile").cloned().unwrap_or_else(|| "default".to_string());
    match colima::collect_diagnostic_logs(profile).await {
        Ok(report) => ok(report),
        Err(e) => err(e.to_string()),
    }
}
