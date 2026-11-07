//! Native Model Context Protocol (MCP) Server over `stdin` / `stdout`.
//!
//! Speaks JSON-RPC 2.0 supporting both newline-delimited JSON (MCP 2024-11-05+ stdio)
//! and `Content-Length:` header framing so every MCP client works out of the box.
//! Never writes diagnostic logs to `stdout`; all non-protocol diagnostics go to `stderr`.

use std::fs;
use std::io;
use std::time::Duration;

use reqwest::{Client, StatusCode};
use serde_json::{json, Value};
use tokio::io::{
    AsyncBufRead, AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader,
};

use crate::commands::{
    agent_loop::{self, HeadlessChatRequest},
    colima, compose, containers, k8s_cluster, kubernetes, networks, resolution, volumes,
};

const DEFAULT_PROTOCOL_VERSION: &str = "2024-11-05";
const SUPPORTED_PROTOCOL_VERSIONS: &[&str] = &[
    "2024-11-05",
    "2025-03-26",
    "2025-06-18",
    "2025-11-25",
];
const MAX_FRAME_BYTES: usize = 32 * 1024 * 1024;
const PORT_RANGE: std::ops::RangeInclusive<u16> = 11420..=11429;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FramingStyle {
    JsonLines,
    ContentLength,
}

#[derive(Debug, Clone)]
struct DesktopApiTarget {
    client: Client,
    base_url: String,
    token: String,
}

enum ProxyOutcome {
    Unavailable,
    Completed { text: String, is_error: bool },
}

/// Entrypoint invoked by `cui mcp` and `colimaui mcp`.
pub async fn run_stdio_server() -> Result<(), String> {
    crate::shell_env::adopt();
    crate::path_util::fix_path_env();

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    serve_mcp_stream(stdin, stdout)
        .await
        .map_err(|e| format!("MCP stdio transport error: {e}"))
}

/// Serve MCP requests from `reader` and write JSON-RPC responses to `writer`.
pub async fn serve_mcp_stream<R, W>(reader: R, mut writer: W) -> io::Result<()>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let mut buf_reader = BufReader::new(reader);
    while let Some((raw_payload, style)) = read_mcp_frame(&mut buf_reader).await? {
        if let Some(response) = handle_raw_jsonrpc(&raw_payload).await {
            write_mcp_frame(&mut writer, &response, style).await?;
        }
    }
    Ok(())
}

/// Read a single MCP message frame, auto-detecting JSON-lines vs `Content-Length` headers.
pub async fn read_mcp_frame<R>(reader: &mut R) -> io::Result<Option<(String, FramingStyle)>>
where
    R: AsyncBufRead + Unpin,
{
    let mut line = String::new();
    loop {
        line.clear();
        let bytes_read = reader.read_line(&mut line).await?;
        if bytes_read == 0 {
            return Ok(None);
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if trimmed.starts_with('{') || trimmed.starts_with('[') {
            return Ok(Some((trimmed.to_string(), FramingStyle::JsonLines)));
        }

        if let Some(first_len) = parse_content_length_header(trimmed) {
            let content_length = consume_remaining_headers(reader, Some(first_len)).await?;
            let body = read_exact_utf8(reader, content_length).await?;
            return Ok(Some((body, FramingStyle::ContentLength)));
        }

        // Handle optional other HTTP-like headers preceding `Content-Length:` (e.g. `Content-Type:`).
        if trimmed.contains(':') && !trimmed.starts_with('"') {
            let content_length = consume_remaining_headers(reader, None).await?;
            let body = read_exact_utf8(reader, content_length).await?;
            return Ok(Some((body, FramingStyle::ContentLength)));
        }

        // Unrecognized raw line: hand off to JSON-RPC parser so it emits -32700 Parse error.
        return Ok(Some((trimmed.to_string(), FramingStyle::JsonLines)));
    }
}

fn parse_content_length_header(line: &str) -> Option<usize> {
    let (name, value) = line.split_once(':')?;
    if !name.trim().eq_ignore_ascii_case("content-length") {
        return None;
    }
    value.trim().parse::<usize>().ok()
}

async fn consume_remaining_headers<R>(
    reader: &mut R,
    mut content_length: Option<usize>,
) -> io::Result<usize>
where
    R: AsyncBufRead + Unpin,
{
    let mut header_line = String::new();
    loop {
        header_line.clear();
        let n = reader.read_line(&mut header_line).await?;
        if n == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "EOF while reading MCP headers",
            ));
        }
        let trimmed = header_line.trim();
        if trimmed.is_empty() {
            break;
        }
        if let Some(parsed) = parse_content_length_header(trimmed) {
            content_length = Some(parsed);
        }
    }

    let len = content_length.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "Missing Content-Length header in framed MCP message",
        )
    })?;

    if len > MAX_FRAME_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("MCP frame exceeds maximum allowed size ({len} bytes)"),
        ));
    }

    Ok(len)
}

async fn read_exact_utf8<R>(reader: &mut R, len: usize) -> io::Result<String>
where
    R: AsyncRead + Unpin,
{
    let mut buf = vec![0u8; len];
    reader.read_exact(&mut buf).await?;
    String::from_utf8(buf).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

/// Write a single JSON-RPC response frame matching the client's framing style.
pub async fn write_mcp_frame<W>(
    writer: &mut W,
    payload: &Value,
    style: FramingStyle,
) -> io::Result<()>
where
    W: AsyncWrite + Unpin,
{
    let encoded = serde_json::to_string(payload)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    match style {
        FramingStyle::JsonLines => {
            writer.write_all(encoded.as_bytes()).await?;
            writer.write_all(b"\n").await?;
        }
        FramingStyle::ContentLength => {
            let header = format!("Content-Length: {}\r\n\r\n", encoded.len());
            writer.write_all(header.as_bytes()).await?;
            writer.write_all(encoded.as_bytes()).await?;
        }
    }
    writer.flush().await
}

fn jsonrpc_ok(id: Value, result: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result,
    })
}

fn jsonrpc_error(id: Value, code: i64, message: impl Into<String>) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": code,
            "message": message.into(),
        }
    })
}

/// Parse and handle a raw JSON-RPC 2.0 string. Returns `None` for notifications.
pub async fn handle_raw_jsonrpc(raw: &str) -> Option<Value> {
    match serde_json::from_str::<Value>(raw) {
        Ok(val) => handle_jsonrpc_value(val).await,
        Err(err) => Some(jsonrpc_error(
            Value::Null,
            -32700,
            format!("Parse error: {err}"),
        )),
    }
}

/// Handle a parsed JSON-RPC 2.0 message value. Returns `None` for notifications.
pub async fn handle_jsonrpc_value(msg: Value) -> Option<Value> {
    let Value::Object(map) = msg else {
        return Some(jsonrpc_error(
            Value::Null,
            -32600,
            "Invalid Request: expected JSON object",
        ));
    };

    let has_id = map.contains_key("id");
    let id = map.get("id").cloned().unwrap_or(Value::Null);
    let method = map.get("method").and_then(Value::as_str).unwrap_or("");

    // Notifications (no `id` field or `notifications/*` method) MUST NOT produce a response frame.
    if !has_id || method.starts_with("notifications/") {
        return None;
    }

    if method.is_empty() {
        return Some(jsonrpc_error(
            id,
            -32600,
            "Invalid Request: missing 'method'",
        ));
    }

    let params = map.get("params").cloned().unwrap_or_else(|| json!({}));

    match method {
        "initialize" => Some(jsonrpc_ok(id, handle_initialize(&params))),
        "ping" => Some(jsonrpc_ok(id, json!({}))),
        "tools/list" => Some(jsonrpc_ok(id, json!({ "tools": mcp_tool_definitions() }))),
        "tools/call" => {
            let Some(tool_name) = params
                .get("name")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
            else {
                return Some(jsonrpc_error(
                    id,
                    -32602,
                    "Invalid params: 'name' is required for tools/call",
                ));
            };
            let arguments = params
                .get("arguments")
                .cloned()
                .unwrap_or_else(|| json!({}));
            let (text, is_error) = execute_mcp_tool(tool_name, &arguments).await;
            Some(jsonrpc_ok(
                id,
                json!({
                    "content": [
                        {
                            "type": "text",
                            "text": text
                        }
                    ],
                    "isError": is_error
                }),
            ))
        }
        _ => Some(jsonrpc_error(
            id,
            -32601,
            format!("Method not found: {method}"),
        )),
    }
}

fn handle_initialize(params: &Value) -> Value {
    let requested = params
        .get("protocolVersion")
        .and_then(Value::as_str)
        .unwrap_or("");
    let protocol_version = if SUPPORTED_PROTOCOL_VERSIONS.contains(&requested) {
        requested
    } else {
        DEFAULT_PROTOCOL_VERSION
    };

    json!({
        "protocolVersion": protocol_version,
        "capabilities": {
            "tools": {
                "listChanged": false
            }
        },
        "serverInfo": {
            "name": "colima-ui",
            "version": env!("CARGO_PKG_VERSION")
        }
    })
}

/// Return all 15 strongly-typed MCP tool definitions.
pub fn mcp_tool_definitions() -> Vec<Value> {
    vec![
        json!({
            "name": "colima_list_instances",
            "description": "List all Colima VM instances/profiles with status, architecture, CPU, memory, disk, runtime, and IP address.",
            "inputSchema": {
                "type": "object",
                "properties": {},
                "required": []
            }
        }),
        json!({
            "name": "colima_instance_action",
            "description": "Start, stop, or restart a Colima VM instance while preserving existing colima.yaml settings.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": {
                        "type": "string",
                        "enum": ["start", "stop", "restart"],
                        "description": "Lifecycle action to perform on the Colima instance."
                    },
                    "profile": {
                        "type": "string",
                        "default": "default",
                        "description": "Colima profile name (defaults to 'default')."
                    }
                },
                "required": ["action"]
            }
        }),
        json!({
            "name": "docker_list_containers",
            "description": "List containers across the active Docker or containerd runtime.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "all": {
                        "type": "boolean",
                        "default": true,
                        "description": "Include stopped containers when true (default: true)."
                    }
                },
                "required": []
            }
        }),
        json!({
            "name": "docker_container_action",
            "description": "Execute a lifecycle action (start, stop, restart, pause, unpause, remove) on a container.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": {
                        "type": "string",
                        "enum": ["start", "stop", "restart", "pause", "unpause", "remove"],
                        "description": "Action to run on the target container."
                    },
                    "container_id": {
                        "type": "string",
                        "description": "Container ID or container name."
                    }
                },
                "required": ["action", "container_id"]
            }
        }),
        json!({
            "name": "docker_container_logs",
            "description": "Retrieve recent stdout/stderr log lines from a container.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "container_id": {
                        "type": "string",
                        "description": "Container ID or container name."
                    },
                    "tail": {
                        "type": "integer",
                        "default": 100,
                        "minimum": 1,
                        "description": "Number of recent log lines to fetch (default: 100)."
                    }
                },
                "required": ["container_id"]
            }
        }),
        json!({
            "name": "docker_list_images",
            "description": "List local container images with repository, tag, ID, and size.",
            "inputSchema": {
                "type": "object",
                "properties": {},
                "required": []
            }
        }),
        json!({
            "name": "docker_pull_image",
            "description": "Pull a container image from a registry.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "image": {
                        "type": "string",
                        "description": "Image reference to pull (e.g. 'nginx:alpine')."
                    }
                },
                "required": ["image"]
            }
        }),
        json!({
            "name": "docker_list_volumes",
            "description": "List Docker or containerd volumes.",
            "inputSchema": {
                "type": "object",
                "properties": {},
                "required": []
            }
        }),
        json!({
            "name": "docker_list_networks",
            "description": "List Docker or containerd networks.",
            "inputSchema": {
                "type": "object",
                "properties": {},
                "required": []
            }
        }),
        json!({
            "name": "compose_list_projects",
            "description": "List Docker Compose projects and their current status.",
            "inputSchema": {
                "type": "object",
                "properties": {},
                "required": []
            }
        }),
        json!({
            "name": "k8s_list_pods",
            "description": "List Kubernetes pods in a specific namespace or across all namespaces.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "namespace": {
                        "type": "string",
                        "description": "Optional Kubernetes namespace (defaults to all namespaces)."
                    }
                },
                "required": []
            }
        }),
        json!({
            "name": "k8s_cluster_status",
            "description": "Inspect Kubernetes cluster health, node readiness, and cluster summary.",
            "inputSchema": {
                "type": "object",
                "properties": {},
                "required": []
            }
        }),
        json!({
            "name": "environment_resolutions",
            "description": "Inspect host DevOps binary resolutions (Colima, Docker, Lima, kubectl, Helm) and environment duality diagnostics.",
            "inputSchema": {
                "type": "object",
                "properties": {},
                "required": []
            }
        }),
        json!({
            "name": "system_diagnostics",
            "description": "Collect Colima VM and container daemon diagnostic logs for troubleshooting.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "profile": {
                        "type": "string",
                        "description": "Optional Colima profile name (defaults to 'default')."
                    }
                },
                "required": []
            }
        }),
        json!({
            "name": "colima_ai_ask",
            "description": "Ask ColimaUI's headless AI DevOps agent to inspect or manage local container/VM infrastructure.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "prompt": {
                        "type": "string",
                        "description": "Natural-language question or DevOps instruction."
                    },
                    "allow_actions": {
                        "type": "boolean",
                        "default": false,
                        "description": "Allow the AI agent to run mutating actions when true (default: false)."
                    }
                },
                "required": ["prompt"]
            }
        }),
    ]
}

// ============================================================
// Dual-Mode Tool Execution (Desktop HTTP Proxy -> Lib Fallback)
// ============================================================

fn parse_recorded_port(raw: &str) -> Option<u16> {
    raw.trim()
        .parse::<u16>()
        .ok()
        .filter(|port| PORT_RANGE.contains(port))
}

async fn probe_health(client: &Client, port: u16) -> bool {
    let url = format!("http://127.0.0.1:{port}/api/health");
    client
        .get(&url)
        .timeout(Duration::from_millis(250))
        .send()
        .await
        .is_ok_and(|r| r.status().is_success())
}

async fn discover_desktop_api() -> Option<DesktopApiTarget> {
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
            return Some(DesktopApiTarget {
                client,
                base_url: format!("http://127.0.0.1:{port}"),
                token,
            });
        }
    }

    for port in PORT_RANGE {
        if recorded == Some(port) {
            continue;
        }
        if probe_health(&client, port).await {
            return Some(DesktopApiTarget {
                client,
                base_url: format!("http://127.0.0.1:{port}"),
                token,
            });
        }
    }

    None
}

fn unwrap_api_response_body(status: StatusCode, body_text: &str) -> ProxyOutcome {
    if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
        return ProxyOutcome::Unavailable;
    }

    if let Ok(parsed) = serde_json::from_str::<Value>(body_text) {
        if let Some(success) = parsed.get("success").and_then(Value::as_bool) {
            if success {
                if let Some(reply) = parsed.get("reply").and_then(Value::as_str) {
                    return ProxyOutcome::Completed {
                        text: reply.to_string(),
                        is_error: false,
                    };
                }
                if let Some(data) = parsed.get("data") {
                    let rendered = match data {
                        Value::String(s) => s.clone(),
                        other => {
                            serde_json::to_string_pretty(other).unwrap_or_else(|_| other.to_string())
                        }
                    };
                    return ProxyOutcome::Completed {
                        text: rendered,
                        is_error: false,
                    };
                }
            } else {
                let err_msg = parsed.get("error").map_or_else(
                    || body_text.to_string(),
                    |err_val| {
                        err_val
                            .as_str()
                            .map(str::to_string)
                            .or_else(|| {
                                err_val
                                    .get("detail")
                                    .and_then(Value::as_str)
                                    .map(str::to_string)
                            })
                            .unwrap_or_else(|| err_val.to_string())
                    },
                );
                return ProxyOutcome::Completed {
                    text: err_msg,
                    is_error: true,
                };
            }
        }
    }

    ProxyOutcome::Completed {
        text: body_text.to_string(),
        is_error: !status.is_success(),
    }
}

async fn send_proxy_request(req: reqwest::RequestBuilder, timeout: Duration) -> ProxyOutcome {
    let Ok(resp) = req.timeout(timeout).send().await else {
        return ProxyOutcome::Unavailable;
    };
    let status = resp.status();
    let Ok(body_text) = resp.text().await else {
        return ProxyOutcome::Unavailable;
    };
    unwrap_api_response_body(status, &body_text)
}

fn build_colima_start_config(profile: &str) -> colima::StartConfig {
    let normalized_profile = if profile.trim().is_empty() {
        "default".to_string()
    } else {
        profile.trim().to_string()
    };
    let yaml_exists = crate::instance_reader::colima_home()
        .join(&normalized_profile)
        .join("colima.yaml")
        .exists();

    if yaml_exists {
        colima::StartConfig {
            profile: normalized_profile,
            runtime: String::new(),
            cpus: 0,
            memory: 0,
            disk: 0,
            vm_type: String::new(),
            kubernetes: false,
            kubernetes_version: String::new(),
            arch: String::new(),
            mount_type: String::new(),
            mounts: Vec::new(),
            dns: Vec::new(),
            network_address: false,
        }
    } else {
        colima::StartConfig {
            profile: normalized_profile,
            runtime: "docker".to_string(),
            cpus: 2,
            memory: 2,
            disk: 60,
            vm_type: String::new(),
            kubernetes: false,
            kubernetes_version: String::new(),
            arch: String::new(),
            mount_type: String::new(),
            mounts: Vec::new(),
            dns: Vec::new(),
            network_address: false,
        }
    }
}

fn required_str<'a>(args: &'a Value, key: &str) -> Result<&'a str, String> {
    args.get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| format!("Missing or empty required parameter '{key}'"))
}

fn optional_str<'a>(args: &'a Value, key: &str) -> Option<&'a str> {
    args.get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

fn to_tool_tuple(res: Result<String, String>) -> (String, bool) {
    match res {
        Ok(text) => (text, false),
        Err(err) => (err, true),
    }
}

fn percent_encode_component(raw: &str) -> String {
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

fn build_query_url(base: &str, path: &str, params: &[(&str, &str)]) -> String {
    if params.is_empty() {
        return format!("{base}{path}");
    }
    let query_str: Vec<String> = params
        .iter()
        .map(|(k, v)| format!("{}={}", percent_encode_component(k), percent_encode_component(v)))
        .collect();
    format!("{base}{path}?{}", query_str.join("&"))
}

async fn try_proxy_tool(target: &DesktopApiTarget, name: &str, args: &Value) -> ProxyOutcome {
    let auth = format!("Bearer {}", target.token);
    let base = &target.base_url;

    match name {
        "colima_list_instances" => {
            let req = target
                .client
                .get(format!("{base}/api/instances"))
                .header("Authorization", auth);
            send_proxy_request(req, Duration::from_secs(10)).await
        }
        "colima_instance_action" => {
            let action = args.get("action").and_then(Value::as_str).unwrap_or("");
            let profile = optional_str(args, "profile").unwrap_or("default");
            match action {
                "start" => {
                    let cfg = build_colima_start_config(profile);
                    let req = target
                        .client
                        .post(format!("{base}/api/instances/start"))
                        .header("Authorization", auth)
                        .json(&cfg);
                    send_proxy_request(req, Duration::from_secs(180)).await
                }
                "stop" => {
                    let url = build_query_url(
                        base,
                        "/api/instances/stop",
                        &[("profile", profile), ("force", "false")],
                    );
                    let req = target.client.post(url).header("Authorization", auth);
                    send_proxy_request(req, Duration::from_secs(60)).await
                }
                "restart" => {
                    let stop_url = build_query_url(
                        base,
                        "/api/instances/stop",
                        &[("profile", profile), ("force", "false")],
                    );
                    let stop_req = target.client.post(stop_url).header("Authorization", &auth);
                    let _ = send_proxy_request(stop_req, Duration::from_secs(60)).await;
                    let cfg = build_colima_start_config(profile);
                    let start_req = target
                        .client
                        .post(format!("{base}/api/instances/start"))
                        .header("Authorization", auth)
                        .json(&cfg);
                    send_proxy_request(start_req, Duration::from_secs(180)).await
                }
                _ => ProxyOutcome::Unavailable,
            }
        }
        "docker_list_containers" => {
            let all = args.get("all").and_then(Value::as_bool).unwrap_or(true);
            let url = build_query_url(
                base,
                "/api/containers",
                &[("all", if all { "true" } else { "false" })],
            );
            let req = target.client.get(url).header("Authorization", auth);
            send_proxy_request(req, Duration::from_secs(15)).await
        }
        "docker_container_action" => {
            let action = args.get("action").and_then(Value::as_str).unwrap_or("");
            let container_id = args
                .get("container_id")
                .and_then(Value::as_str)
                .unwrap_or("");
            let path = format!("/api/containers/{action}");
            let url = build_query_url(base, &path, &[("containerId", container_id)]);
            let req = target.client.post(url).header("Authorization", auth);
            send_proxy_request(req, Duration::from_secs(30)).await
        }
        "docker_container_logs" => {
            let container_id = args
                .get("container_id")
                .and_then(Value::as_str)
                .unwrap_or("");
            let tail = args
                .get("tail")
                .and_then(Value::as_u64)
                .and_then(|n| u32::try_from(n).ok())
                .unwrap_or(100)
                .max(1);
            let tail_str = tail.to_string();
            let url = build_query_url(
                base,
                "/api/containers/logs",
                &[("containerId", container_id), ("lines", tail_str.as_str())],
            );
            let req = target.client.get(url).header("Authorization", auth);
            send_proxy_request(req, Duration::from_secs(15)).await
        }
        "docker_list_images" => {
            let req = target
                .client
                .get(format!("{base}/api/images"))
                .header("Authorization", auth);
            send_proxy_request(req, Duration::from_secs(15)).await
        }
        "docker_pull_image" => {
            let image = args.get("image").and_then(Value::as_str).unwrap_or("");
            let url = build_query_url(base, "/api/images/pull", &[("imageName", image)]);
            let req = target.client.post(url).header("Authorization", auth);
            send_proxy_request(req, Duration::from_secs(300)).await
        }
        "docker_list_volumes" => {
            let req = target
                .client
                .get(format!("{base}/api/volumes"))
                .header("Authorization", auth);
            send_proxy_request(req, Duration::from_secs(15)).await
        }
        "docker_list_networks" => {
            let req = target
                .client
                .get(format!("{base}/api/networks"))
                .header("Authorization", auth);
            send_proxy_request(req, Duration::from_secs(15)).await
        }
        "compose_list_projects" => {
            let req = target
                .client
                .get(format!("{base}/api/compose"))
                .header("Authorization", auth);
            send_proxy_request(req, Duration::from_secs(15)).await
        }
        "k8s_list_pods" => {
            let ns = optional_str(args, "namespace").unwrap_or("all");
            let url = build_query_url(base, "/api/k8s/pods", &[("namespace", ns)]);
            let req = target.client.get(url).header("Authorization", auth);
            send_proxy_request(req, Duration::from_secs(15)).await
        }
        "k8s_cluster_status" => {
            let req = target
                .client
                .get(format!("{base}/api/k8s/cluster-health"))
                .header("Authorization", auth);
            send_proxy_request(req, Duration::from_secs(15)).await
        }
        "environment_resolutions" => {
            let req = target
                .client
                .get(format!("{base}/api/environment/resolutions"))
                .header("Authorization", auth);
            send_proxy_request(req, Duration::from_secs(10)).await
        }
        "system_diagnostics" => {
            let profile = optional_str(args, "profile").unwrap_or("default");
            let url = build_query_url(base, "/api/diagnostics/logs", &[("profile", profile)]);
            let req = target.client.get(url).header("Authorization", auth);
            send_proxy_request(req, Duration::from_secs(20)).await
        }
        "colima_ai_ask" => {
            let prompt = args.get("prompt").and_then(Value::as_str).unwrap_or("");
            let allow_actions = args
                .get("allow_actions")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let req = target
                .client
                .post(format!("{base}/api/cli/chat"))
                .header("Authorization", auth)
                .json(&json!({
                    "prompt": prompt,
                    "allow_actions": allow_actions,
                }));
            send_proxy_request(req, Duration::from_secs(120)).await
        }
        _ => ProxyOutcome::Unavailable,
    }
}

/// Validate arguments, attempt Desktop Local HTTP API proxy when running, and fall back to `colima_ui_lib`.
pub async fn execute_mcp_tool(name: &str, args: &Value) -> (String, bool) {
    // 1. Validate tool name and required arguments upfront before network I/O.
    match name {
        "colima_list_instances"
        | "docker_list_containers"
        | "docker_list_images"
        | "docker_list_volumes"
        | "docker_list_networks"
        | "compose_list_projects"
        | "k8s_list_pods"
        | "k8s_cluster_status"
        | "environment_resolutions"
        | "system_diagnostics" => {}
        "colima_instance_action" => {
            let action = match required_str(args, "action") {
                Ok(a) => a,
                Err(e) => return (e, true),
            };
            if !matches!(action, "start" | "stop" | "restart") {
                return (
                    format!("Invalid action '{action}'. Expected one of: start, stop, restart."),
                    true,
                );
            }
        }
        "docker_container_action" => {
            let action = match required_str(args, "action") {
                Ok(a) => a,
                Err(e) => return (e, true),
            };
            if !matches!(
                action,
                "start" | "stop" | "restart" | "pause" | "unpause" | "remove"
            ) {
                return (
                    format!(
                        "Invalid action '{action}'. Expected one of: start, stop, restart, pause, unpause, remove."
                    ),
                    true,
                );
            }
            if let Err(e) = required_str(args, "container_id") {
                return (e, true);
            }
        }
        "docker_container_logs" => {
            if let Err(e) = required_str(args, "container_id") {
                return (e, true);
            }
        }
        "docker_pull_image" => {
            if let Err(e) = required_str(args, "image") {
                return (e, true);
            }
        }
        "colima_ai_ask" => {
            if let Err(e) = required_str(args, "prompt") {
                return (e, true);
            }
        }
        _ => return (format!("Unknown tool: {name}"), true),
    }

    // 2. If ColimaUI Desktop HTTP server is reachable, proxy through it so Activity feed & UI stay synced.
    if let Some(target) = discover_desktop_api().await {
        if let ProxyOutcome::Completed { text, is_error } = try_proxy_tool(&target, name, args).await
        {
            return (text, is_error);
        }
    }

    // 3. Direct library fallback when ColimaUI Desktop is closed.
    to_tool_tuple(execute_direct_fallback(name, args).await)
}

async fn execute_direct_fallback(name: &str, args: &Value) -> Result<String, String> {
    match name {
        "colima_list_instances" => {
            let instances = crate::instance_reader::list_instances_fast();
            serde_json::to_string_pretty(&instances).map_err(|e| e.to_string())
        }
        "colima_instance_action" => {
            let action = required_str(args, "action")?;
            let profile = optional_str(args, "profile").unwrap_or("default");
            match action {
                "start" => colima::start_instance(build_colima_start_config(profile))
                    .await
                    .map_err(|e| e.to_string()),
                "stop" => colima::stop_instance_cli(profile.to_string(), false)
                    .await
                    .map_err(|e| e.to_string()),
                "restart" => {
                    let _ = colima::stop_instance_cli(profile.to_string(), false).await;
                    colima::start_instance(build_colima_start_config(profile))
                        .await
                        .map_err(|e| e.to_string())?;
                    Ok(format!("Colima instance '{profile}' restarted"))
                }
                _ => Err(format!("Unsupported action: {action}")),
            }
        }
        "docker_list_containers" => {
            let all = args.get("all").and_then(Value::as_bool).unwrap_or(true);
            let list = containers::list_containers_cli(all).await?;
            serde_json::to_string_pretty(&list).map_err(|e| e.to_string())
        }
        "docker_container_action" => {
            let action = required_str(args, "action")?;
            let container_id = required_str(args, "container_id")?.to_string();
            match action {
                "start" => containers::start_container(container_id)
                    .await
                    .map_err(|e| e.to_string()),
                "stop" => containers::stop_container(container_id)
                    .await
                    .map_err(|e| e.to_string()),
                "restart" => containers::restart_container(container_id)
                    .await
                    .map_err(|e| e.to_string()),
                "pause" => containers::pause_container(container_id)
                    .await
                    .map_err(|e| e.to_string()),
                "unpause" => containers::unpause_container(container_id)
                    .await
                    .map_err(|e| e.to_string()),
                "remove" => containers::remove_container(container_id, false)
                    .await
                    .map_err(|e| e.to_string()),
                _ => Err(format!("Unsupported container action: {action}")),
            }
        }
        "docker_container_logs" => {
            let container_id = required_str(args, "container_id")?.to_string();
            let tail = args
                .get("tail")
                .and_then(Value::as_u64)
                .and_then(|n| u32::try_from(n).ok())
                .unwrap_or(100)
                .max(1);
            containers::container_logs(container_id, tail)
                .await
                .map_err(|e| e.to_string())
        }
        "docker_list_images" => {
            let images = crate::services::container::ContainerService::auto_detect()
                .list_images()
                .await
                .map_err(|e| e.to_string())?;
            serde_json::to_string_pretty(&images).map_err(|e| e.to_string())
        }
        "docker_pull_image" => {
            let image = required_str(args, "image")?;
            crate::services::container::ContainerService::auto_detect()
                .pull_image(image)
                .await
                .map_err(|e| e.to_string())
        }
        "docker_list_volumes" => {
            let vols = volumes::list_volumes().await.map_err(|e| e.to_string())?;
            serde_json::to_string_pretty(&vols).map_err(|e| e.to_string())
        }
        "docker_list_networks" => {
            let nets = networks::list_networks().await.map_err(|e| e.to_string())?;
            serde_json::to_string_pretty(&nets).map_err(|e| e.to_string())
        }
        "compose_list_projects" => {
            let projects = compose::list_compose_projects()
                .await
                .map_err(|e| e.to_string())?;
            serde_json::to_string_pretty(&projects).map_err(|e| e.to_string())
        }
        "k8s_list_pods" => {
            let ns = optional_str(args, "namespace").unwrap_or("all");
            let pods = kubernetes::k8s_pods(ns.to_string())
                .await
                .map_err(|e| e.to_string())?;
            serde_json::to_string_pretty(&pods).map_err(|e| e.to_string())
        }
        "k8s_cluster_status" => k8s_cluster::k8s_cluster_health()
            .await
            .map_err(|e| e.to_string()),
        "environment_resolutions" => {
            let report = tokio::task::spawn_blocking(resolution::get_environment_resolutions)
                .await
                .map_err(|e| format!("Task join error: {e}"))?;
            serde_json::to_string_pretty(&report).map_err(|e| e.to_string())
        }
        "system_diagnostics" => {
            let profile = optional_str(args, "profile").unwrap_or("default");
            colima::collect_diagnostic_logs(profile.to_string())
                .await
                .map_err(|e| e.to_string())
        }
        "colima_ai_ask" => {
            let prompt = required_str(args, "prompt")?.to_string();
            let allow_actions = args
                .get("allow_actions")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            agent_loop::run_headless_agent(HeadlessChatRequest {
                prompt,
                provider: None,
                model: None,
                allow_actions,
            })
            .await
            .map_err(|e| e.to_string())
        }
        _ => Err(format!("Unknown tool: {name}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn initialize_echoes_supported_version_and_falls_back_cleanly() {
        let req = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-03-26",
                "capabilities": {},
                "clientInfo": { "name": "test-host", "version": "1.0.0" }
            }
        });
        let resp = handle_jsonrpc_value(req).await.expect("initialize response");
        assert_eq!(resp["id"], 1);
        assert_eq!(resp["result"]["protocolVersion"], "2025-03-26");
        assert_eq!(resp["result"]["capabilities"]["tools"]["listChanged"], false);
        assert_eq!(resp["result"]["serverInfo"]["name"], "colima-ui");
        assert_eq!(
            resp["result"]["serverInfo"]["version"],
            env!("CARGO_PKG_VERSION")
        );

        let fallback_req = json!({
            "jsonrpc": "2.0",
            "id": "init-2",
            "method": "initialize",
            "params": { "protocolVersion": "1999-01-01" }
        });
        let fallback_resp = handle_jsonrpc_value(fallback_req)
            .await
            .expect("fallback initialize response");
        assert_eq!(fallback_resp["id"], "init-2");
        assert_eq!(fallback_resp["result"]["protocolVersion"], "2024-11-05");
    }

    #[tokio::test]
    async fn notifications_do_not_emit_response_frames() {
        let initialized = json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        });
        assert!(handle_jsonrpc_value(initialized).await.is_none());

        let generic_notification = json!({
            "jsonrpc": "2.0",
            "method": "some/notification",
            "params": {}
        });
        assert!(handle_jsonrpc_value(generic_notification).await.is_none());
    }

    #[tokio::test]
    async fn ping_returns_empty_object() {
        let req = json!({
            "jsonrpc": "2.0",
            "id": 42,
            "method": "ping"
        });
        let resp = handle_jsonrpc_value(req).await.expect("ping response");
        assert_eq!(resp["id"], 42);
        assert_eq!(resp["result"], json!({}));
    }

    #[tokio::test]
    async fn tools_list_returns_all_15_well_typed_tools() {
        let req = json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/list"
        });
        let resp = handle_jsonrpc_value(req).await.expect("tools/list response");
        let tools = resp["result"]["tools"]
            .as_array()
            .expect("tools must be an array");

        let expected_names = [
            "colima_list_instances",
            "colima_instance_action",
            "docker_list_containers",
            "docker_container_action",
            "docker_container_logs",
            "docker_list_images",
            "docker_pull_image",
            "docker_list_volumes",
            "docker_list_networks",
            "compose_list_projects",
            "k8s_list_pods",
            "k8s_cluster_status",
            "environment_resolutions",
            "system_diagnostics",
            "colima_ai_ask",
        ];

        assert_eq!(tools.len(), 15);
        let actual_names: Vec<&str> = tools
            .iter()
            .map(|t| t["name"].as_str().expect("tool name string"))
            .collect();
        assert_eq!(actual_names, expected_names);

        for tool in tools {
            let desc = tool["description"].as_str().expect("tool description");
            assert!(!desc.trim().is_empty());
            assert_eq!(tool["inputSchema"]["type"], "object");
            assert!(tool["inputSchema"]["properties"].is_object());
            assert!(tool["inputSchema"]["required"].is_array());
        }
    }

    #[tokio::test]
    async fn tools_call_handles_environment_resolutions_and_unknown_tools() {
        let ok_call = json!({
            "jsonrpc": "2.0",
            "id": 10,
            "method": "tools/call",
            "params": {
                "name": "environment_resolutions",
                "arguments": {}
            }
        });
        let ok_resp = handle_jsonrpc_value(ok_call)
            .await
            .expect("tools/call response");
        assert_eq!(ok_resp["id"], 10);
        assert_eq!(ok_resp["result"]["isError"], false);
        let text = ok_resp["result"]["content"][0]["text"]
            .as_str()
            .expect("content text");
        let parsed: Value = serde_json::from_str(text).expect("valid JSON environment report");
        assert!(parsed.get("rows").is_some());

        let unknown_call = json!({
            "jsonrpc": "2.0",
            "id": 11,
            "method": "tools/call",
            "params": {
                "name": "nonexistent_tool",
                "arguments": {}
            }
        });
        let unknown_resp = handle_jsonrpc_value(unknown_call)
            .await
            .expect("unknown tool response");
        assert_eq!(unknown_resp["id"], 11);
        assert_eq!(unknown_resp["result"]["isError"], true);
        assert!(unknown_resp["result"]["content"][0]["text"]
            .as_str()
            .unwrap_or("")
            .contains("Unknown tool"));
    }

    #[tokio::test]
    async fn unknown_jsonrpc_method_returns_minus_32601() {
        let req = json!({
            "jsonrpc": "2.0",
            "id": 99,
            "method": "unknown/method"
        });
        let resp = handle_jsonrpc_value(req)
            .await
            .expect("method not found response");
        assert_eq!(resp["id"], 99);
        assert_eq!(resp["error"]["code"], -32601);
    }

    #[tokio::test]
    async fn stream_framing_supports_json_lines_and_content_length() {
        let cl_body = r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#;
        let notif_line = r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#;
        let jl_body = r#"{"jsonrpc":"2.0","id":2,"method":"unknown/op"}"#;

        let input = format!(
            "Content-Length: {}\r\n\r\n{}{}\n{}\n",
            cl_body.len(),
            cl_body,
            notif_line,
            jl_body
        );

        let mut output = Vec::new();
        serve_mcp_stream(std::io::Cursor::new(input.into_bytes()), &mut output)
            .await
            .expect("serve_mcp_stream succeeds");

        let mut reader = BufReader::new(std::io::Cursor::new(output));
        let (first_payload, first_style) = read_mcp_frame(&mut reader)
            .await
            .expect("read first frame")
            .expect("first frame present");
        assert_eq!(first_style, FramingStyle::ContentLength);
        let first_json: Value = serde_json::from_str(&first_payload).unwrap();
        assert_eq!(first_json["id"], 1);
        assert_eq!(first_json["result"], json!({}));

        let (second_payload, second_style) = read_mcp_frame(&mut reader)
            .await
            .expect("read second frame")
            .expect("second frame present");
        assert_eq!(second_style, FramingStyle::JsonLines);
        let second_json: Value = serde_json::from_str(&second_payload).unwrap();
        assert_eq!(second_json["id"], 2);
        assert_eq!(second_json["error"]["code"], -32601);

        assert!(read_mcp_frame(&mut reader).await.unwrap().is_none());
    }
}
