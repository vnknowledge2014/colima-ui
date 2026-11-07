//! UI Remote Control Bridge (`/api/ui/navigate` & `/api/ui/state`).
//!
//! Allows external agents (`cui`, MCP, A2A, Local HTTP API) and headless agent turns
//! to navigate the live ColimaUI Desktop & Browser UI via dual Tauri IPC + SSE events.

use axum::{http::StatusCode, response::Json};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::api_server::{ok, ApiResponse};
use crate::error::ColimaError;

/// Canonical set of navigable UI pages in ColimaUI.
pub const VALID_UI_PAGES: &[&str] = &[
    "dashboard",
    "instances",
    "environment",
    "containers",
    "images",
    "volumes",
    "networks",
    "compose",
    "topology",
    "activity",
    "security",
    "kubernetes",
    "linux-vms",
    "models",
    "terminal",
    "settings",
    "help",
    "ai-chat",
];

/// Returns `true` if `p` (trimmed, ASCII lowercased) is a valid ColimaUI page ID.
pub fn is_valid_ui_page(p: &str) -> bool {
    let normalized = p.trim().to_ascii_lowercase();
    VALID_UI_PAGES.contains(&normalized.as_str())
}

/// Validate `page`, broadcast a `ui-navigate` event over both Tauri IPC and SSE,
/// and return the emitted JSON payload.
pub fn emit_ui_navigate(
    page: &str,
    section: Option<&str>,
    toast: Option<&str>,
) -> Result<Value, String> {
    let normalized = page.trim().to_ascii_lowercase();
    if !is_valid_ui_page(&normalized) {
        return Err(format!(
            "Invalid UI page '{}'. Expected one of: {}",
            page.trim(),
            VALID_UI_PAGES.join(", ")
        ));
    }

    let clean_section = section.map(str::trim).filter(|s| !s.is_empty());
    let clean_toast = toast.map(str::trim).filter(|s| !s.is_empty());

    let payload = json!({
        "page": normalized,
        "section": clean_section,
        "toast": clean_toast,
    });

    crate::sse::emit_all("ui-navigate", &payload);
    Ok(payload)
}

#[derive(Debug, Default, Deserialize)]
pub struct UiNavigateRequest {
    #[serde(default)]
    pub page: String,
    #[serde(default)]
    pub section: Option<String>,
    #[serde(default)]
    pub toast: Option<String>,
}

#[allow(clippy::unused_async)]
pub async fn api_ui_navigate(
    Json(body): Json<UiNavigateRequest>,
) -> (StatusCode, Json<ApiResponse<Value>>) {
    match emit_ui_navigate(&body.page, body.section.as_deref(), body.toast.as_deref()) {
        Ok(payload) => ok(payload),
        Err(msg) => (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse {
                success: false,
                data: None,
                error: Some(ColimaError::validation(msg)),
            }),
        ),
    }
}

#[allow(clippy::unused_async)]
pub async fn api_ui_state() -> (StatusCode, Json<ApiResponse<Value>>) {
    ok(json!({
        "valid_pages": VALID_UI_PAGES,
        "a2a_default_session_id": crate::commands::ai_chat::A2A_DEFAULT_CONVERSATION_ID,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_and_emits_ui_navigate_over_sse() {
        assert!(is_valid_ui_page("dashboard"));
        assert!(is_valid_ui_page("  Kubernetes "));
        assert!(is_valid_ui_page("ai-chat"));
        assert!(!is_valid_ui_page("nonexistent-page"));

        let (mut rx, _guard) = crate::sse::subscribe_topics(&["ui-navigate".to_string()]);
        let payload = emit_ui_navigate("  security ", Some("scan"), Some("Scanning image"))
            .expect("valid ui-navigate");
        assert_eq!(payload["page"], "security");
        assert_eq!(payload["section"], "scan");
        assert_eq!(payload["toast"], "Scanning image");

        let msg = rx.try_recv().expect("SSE received ui-navigate");
        assert_eq!(msg.event, "ui-navigate");

        assert!(emit_ui_navigate("not-a-page", None, None).is_err());
    }
}
