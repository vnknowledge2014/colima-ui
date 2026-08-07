//! Authentication infrastructure for the HTTP API server.
//!
//! Generates a CSPRNG-based auth token and provides middleware for Axum.

use axum::{
    http::{header, StatusCode},
    middleware::Next,
};
use std::sync::OnceLock;

static API_TOKEN: OnceLock<String> = OnceLock::new();

/// Get the API token, generating a cryptographically random one on first call.
/// Using a CSPRNG (instead of the previous timestamp+pid scheme) prevents an
/// attacker from guessing/brute-forcing the token within a short time window.
pub fn get_api_token() -> String {
    API_TOKEN
        .get_or_init(|| {
            use rand::RngCore;
            let mut bytes = [0u8; 32];
            rand::rngs::OsRng.fill_bytes(&mut bytes);
            let hex: String = bytes.iter().map(|b| format!("{:02x}", b)).collect();
            format!("colima-{}", hex)
        })
        .clone()
}

/// Auth middleware — rejects requests without a valid Bearer token.
/// SSE /api/events endpoint uses query param ?token= instead of header.
pub async fn auth_middleware(
    req: axum::extract::Request,
    next: Next,
) -> Result<axum::response::Response, StatusCode> {
    let expected = get_api_token();
    let path = req.uri().path();

    // SSE endpoints use query param because EventSource API can't set headers
    if path == "/api/events" || path.ends_with("/stream") {
        let query = req.uri().query().unwrap_or("");
        if query.contains(&format!("token={}", expected)) {
            return Ok(next.run(req).await);
        }
        return Err(StatusCode::UNAUTHORIZED);
    }

    // Regular endpoints use Authorization header
    if let Some(auth) = req.headers().get(header::AUTHORIZATION) {
        if let Ok(auth_str) = auth.to_str() {
            if auth_str == format!("Bearer {}", expected) {
                return Ok(next.run(req).await);
            }
        }
    }

    Err(StatusCode::UNAUTHORIZED)
}
