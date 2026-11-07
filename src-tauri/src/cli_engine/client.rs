//! Dual-transport HTTP client for talking to a running ColimaUI Desktop instance.
//!
//! Probes `~/.colima-ui/api_port` first (250ms `/api/health` check), falls back to
//! scanning `11420..=11429` on `127.0.0.1`, and reads `~/.colima-ui/api_token`.
//! When the Desktop server is unreachable or unauthenticated, callers fall back
//! seamlessly to direct in-process `colima_ui_lib` execution.

use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use reqwest::{Client, StatusCode};
use serde_json::{json, Value};

/// Port range used by `api_server::start_api_server`.
pub const PORT_RANGE: std::ops::RangeInclusive<u16> = 11420..=11429;

/// Parse a recorded port string and accept it only if it lies inside [`PORT_RANGE`].
pub fn parse_recorded_port(raw: &str) -> Option<u16> {
    raw.trim()
        .parse::<u16>()
        .ok()
        .filter(|port| PORT_RANGE.contains(port))
}

/// Inspect `~/.colima-ui/api_token` and `~/.colima-ui/api_port` metadata for `doctor`.
#[derive(Debug, Clone)]
pub struct CredentialStatus {
    pub data_dir: PathBuf,
    pub token_file_present: bool,
    pub token_non_empty: bool,
    pub token_permissions_0600: bool,
    pub recorded_port: Option<u16>,
}

pub fn inspect_credentials() -> CredentialStatus {
    let data_dir = crate::path_util::app_data_dir();
    let token_path = data_dir.join("api_token");
    let port_path = data_dir.join("api_port");

    let meta = fs::metadata(&token_path).ok();
    let token_file_present = meta.is_some();

    #[cfg(unix)]
    let token_permissions_0600 = meta.as_ref().is_some_and(|m| {
        use std::os::unix::fs::PermissionsExt;
        (m.permissions().mode() & 0o777) == 0o600
    });
    #[cfg(not(unix))]
    let token_permissions_0600 = token_file_present;

    let token_non_empty = fs::read_to_string(&token_path)
        .ok()
        .is_some_and(|s| !s.trim().is_empty());

    let recorded_port = fs::read_to_string(&port_path)
        .ok()
        .and_then(|raw| parse_recorded_port(&raw));

    CredentialStatus {
        data_dir,
        token_file_present,
        token_non_empty,
        token_permissions_0600,
        recorded_port,
    }
}

pub fn percent_encode_component(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for byte in raw.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            _ => {
                use std::fmt::Write;
                let _ = write!(&mut out, "%{byte:02X}");
            }
        }
    }
    out
}

pub fn build_query_url(base: &str, path: &str, params: &[(&str, &str)]) -> String {
    if params.is_empty() {
        return format!("{base}{path}");
    }
    let encoded: Vec<String> = params
        .iter()
        .map(|(k, v)| format!("{}={}", percent_encode_component(k), percent_encode_component(v)))
        .collect();
    format!("{base}{path}?{}", encoded.join("&"))
}

/// Outcome of an HTTP request to the local ColimaUI Desktop API.
#[derive(Debug)]
pub enum HttpOutcome {
    /// Desktop API could not be reached, rejected auth, or lacked the route — caller may fall back.
    Unavailable(String),
    /// Desktop API answered with `success: true` (unwrapped `data` value).
    Ok(Value),
    /// Desktop API answered with an explicit application error (`success: false`).
    ApiErr(String),
}

#[derive(Debug, Clone)]
pub struct DesktopClient {
    client: Client,
    pub base_url: String,
    pub port: u16,
    pub token: String,
}

async fn probe_health(client: &Client, port: u16) -> bool {
    let url = format!("http://127.0.0.1:{port}/api/health");
    client
        .get(&url)
        .timeout(Duration::from_millis(250))
        .send()
        .await
        .is_ok_and(|resp| resp.status().is_success())
}

/// Probe active ColimaUI port (even if `api_token` is missing, useful for `doctor`).
pub async fn probe_active_port() -> Option<u16> {
    let client = Client::new();
    let recorded = fs::read_to_string(crate::path_util::app_data_dir().join("api_port"))
        .ok()
        .and_then(|raw| parse_recorded_port(&raw));

    if let Some(port) = recorded {
        if probe_health(&client, port).await {
            return Some(port);
        }
    }

    for port in PORT_RANGE {
        if recorded == Some(port) {
            continue;
        }
        if probe_health(&client, port).await {
            return Some(port);
        }
    }
    None
}

impl DesktopClient {
    /// Discover a live ColimaUI Desktop HTTP server and load `~/.colima-ui/api_token`.
    pub async fn discover() -> Option<Self> {
        let data_dir = crate::path_util::app_data_dir();
        let token = fs::read_to_string(data_dir.join("api_token"))
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())?;

        let client = Client::new();
        let recorded = fs::read_to_string(data_dir.join("api_port"))
            .ok()
            .and_then(|raw| parse_recorded_port(&raw));

        if let Some(port) = recorded {
            if probe_health(&client, port).await {
                return Some(Self {
                    client,
                    base_url: format!("http://127.0.0.1:{port}"),
                    port,
                    token,
                });
            }
        }

        for port in PORT_RANGE {
            if recorded == Some(port) {
                continue;
            }
            if probe_health(&client, port).await {
                return Some(Self {
                    client,
                    base_url: format!("http://127.0.0.1:{port}"),
                    port,
                    token,
                });
            }
        }

        None
    }

    pub async fn get(
        &self,
        path: &str,
        query: &[(&str, &str)],
        timeout: Duration,
    ) -> HttpOutcome {
        let url = build_query_url(&self.base_url, path, query);
        let req = self
            .client
            .get(url)
            .header("Authorization", format!("Bearer {}", self.token))
            .timeout(timeout);
        self.send_and_unwrap(req).await
    }

    pub async fn post_json(
        &self,
        path: &str,
        query: &[(&str, &str)],
        body: &Value,
        timeout: Duration,
    ) -> HttpOutcome {
        let url = build_query_url(&self.base_url, path, query);
        let req = self
            .client
            .post(url)
            .header("Authorization", format!("Bearer {}", self.token))
            .json(body)
            .timeout(timeout);
        self.send_and_unwrap(req).await
    }

    pub async fn post_empty(
        &self,
        path: &str,
        query: &[(&str, &str)],
        timeout: Duration,
    ) -> HttpOutcome {
        let url = build_query_url(&self.base_url, path, query);
        let req = self
            .client
            .post(url)
            .header("Authorization", format!("Bearer {}", self.token))
            .timeout(timeout);
        self.send_and_unwrap(req).await
    }

    async fn send_and_unwrap(&self, req: reqwest::RequestBuilder) -> HttpOutcome {
        let resp = match req.send().await {
            Ok(r) => r,
            Err(e) => return HttpOutcome::Unavailable(format!("Connection failed: {e}")),
        };

        let status = resp.status();
        if matches!(
            status,
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN | StatusCode::NOT_FOUND
        ) {
            return HttpOutcome::Unavailable(format!("HTTP {status}"));
        }

        let text = match resp.text().await {
            Ok(t) => t,
            Err(e) => return HttpOutcome::Unavailable(format!("Failed reading body: {e}")),
        };

        unwrap_response_json(status, &text)
    }

    /// Connect to `/api/events` SSE stream and deliver parsed events to `on_event`.
    /// Returns `Ok(collected_count)` when `limit` is reached or stream closes.
    pub async fn stream_sse<F>(
        &self,
        topics: Option<&str>,
        filter: Option<&str>,
        limit: Option<usize>,
        timeout: Option<Duration>,
        mut on_event: F,
    ) -> Result<Vec<Value>, String>
    where
        F: FnMut(&Value),
    {
        let mut params = vec![("token", self.token.as_str())];
        if let Some(t) = topics.map(str::trim).filter(|s| !s.is_empty()) {
            params.push(("topics", t));
        }
        let url = build_query_url(&self.base_url, "/api/events", &params);

        let mut req = self
            .client
            .get(url)
            .header("Accept", "text/event-stream")
            .header("Authorization", format!("Bearer {}", self.token));

        if let Some(t) = timeout {
            req = req.timeout(t);
        }

        let mut resp = req
            .send()
            .await
            .map_err(|e| format!("Failed to connect to /api/events: {e}"))?;

        if !resp.status().is_success() {
            return Err(format!("/api/events returned HTTP {}", resp.status()));
        }

        let filter_lower = filter
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_ascii_lowercase);

        let mut collected = Vec::new();
        let mut line_buf = String::new();
        let mut pending_bytes: Vec<u8> = Vec::new();
        let mut current_event = String::from("message");
        let mut current_data_lines: Vec<String> = Vec::new();

        loop {
            if limit.is_some_and(|max| collected.len() >= max) {
                break;
            }

            let next_chunk = if let Some(wait) = timeout {
                match tokio::time::timeout(wait, resp.chunk()).await {
                    Ok(res) => res.map_err(|e| format!("SSE stream read error: {e}"))?,
                    Err(_) => break,
                }
            } else {
                resp.chunk()
                    .await
                    .map_err(|e| format!("SSE stream read error: {e}"))?
            };

            let Some(chunk) = next_chunk else {
                break;
            };

            pending_bytes.extend_from_slice(&chunk);
            let valid_up_to = match std::str::from_utf8(&pending_bytes) {
                Ok(valid) => {
                    line_buf.push_str(valid);
                    pending_bytes.len()
                }
                Err(err) => {
                    let valid_len = err.valid_up_to();
                    if valid_len > 0 {
                        if let Ok(valid) = std::str::from_utf8(&pending_bytes[..valid_len]) {
                            line_buf.push_str(valid);
                        }
                    }
                    valid_len
                }
            };
            if valid_up_to > 0 {
                pending_bytes.drain(..valid_up_to);
            }

            while let Some(newline_pos) = line_buf.find('\n') {
                let raw_line = line_buf[..newline_pos].trim_end_matches('\r').to_string();
                line_buf.drain(..=newline_pos);

                if raw_line.is_empty() {
                    if !current_data_lines.is_empty() {
                        let data_joined = current_data_lines.join("\n");
                        let parsed_data = serde_json::from_str::<Value>(&data_joined)
                            .unwrap_or_else(|_| Value::String(data_joined.clone()));
                        let envelope = json!({
                            "event": current_event,
                            "data": parsed_data,
                        });

                        let matches_filter = match &filter_lower {
                            Some(needle) => {
                                current_event.to_ascii_lowercase().contains(needle)
                                    || data_joined.to_ascii_lowercase().contains(needle)
                            }
                            None => true,
                        };

                        if matches_filter {
                            on_event(&envelope);
                            collected.push(envelope);
                        }

                        current_event = String::from("message");
                        current_data_lines.clear();

                        if limit.is_some_and(|max| collected.len() >= max) {
                            return Ok(collected);
                        }
                    }
                    continue;
                }

                if raw_line.starts_with(':') {
                    continue;
                }

                if let Some(ev) = raw_line.strip_prefix("event:") {
                    current_event = ev.trim().to_string();
                } else if let Some(data) = raw_line.strip_prefix("data:") {
                    current_data_lines.push(data.trim_start().to_string());
                }
            }
        }

        Ok(collected)
    }
}

fn extract_error_string(err_val: &Value, fallback: &str) -> String {
    err_val
        .as_str()
        .map(str::to_string)
        .or_else(|| {
            err_val
                .get("detail")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .or_else(|| {
            err_val
                .get("message")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or_else(|| {
            if err_val.is_null() {
                fallback.to_string()
            } else {
                err_val.to_string()
            }
        })
}

pub fn unwrap_response_json(status: StatusCode, text: &str) -> HttpOutcome {
    if let Ok(parsed) = serde_json::from_str::<Value>(text) {
        if let Some(success) = parsed.get("success").and_then(Value::as_bool) {
            if success {
                if let Some(reply) = parsed.get("reply").and_then(Value::as_str) {
                    return HttpOutcome::Ok(json!({
                        "reply": reply,
                        "raw": parsed,
                    }));
                }
                return HttpOutcome::Ok(parsed.get("data").cloned().unwrap_or(Value::Null));
            }
            let msg = parsed
                .get("error")
                .map_or_else(|| text.to_string(), |v| extract_error_string(v, text));
            return HttpOutcome::ApiErr(msg);
        }
        if status.is_success() {
            return HttpOutcome::Ok(parsed);
        }
        return HttpOutcome::ApiErr(text.to_string());
    }

    if status.is_success() {
        HttpOutcome::Ok(Value::String(text.to_string()))
    } else {
        HttpOutcome::ApiErr(format!("HTTP {status}: {text}"))
    }
}
