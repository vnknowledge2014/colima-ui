//! Native Agent-to-Agent (A2A v0.2.1) Protocol Engine for ColimaUI.
//!
//! Exposes:
//! - Agent Card builder (`GET /.well-known/agent.json`)
//! - Bounded in-memory task store (`MAX_A2A_TASKS = 128`) with live `AbortHandle` cancellation
//! - JSON-RPC 2.0 dispatcher (`POST /a2a`) supporting `message/send`, `tasks/send`,
//!   `tasks/get`, and `tasks/cancel`.

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{LazyLock, Mutex};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::task::AbortHandle;

use crate::commands::agent_loop::{
    run_headless_agent_detailed, HeadlessChatRequest, ToolExecutionRecord,
};
use crate::commands::ai_chat::A2A_DEFAULT_CONVERSATION_ID;

/// Maximum number of recent A2A tasks retained in process memory.
pub const MAX_A2A_TASKS: usize = 128;

pub const A2A_PROTOCOL_VERSION: &str = "0.2.1";
pub const A2A_DEFAULT_URL: &str = "http://127.0.0.1:11420/a2a";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct A2aStateTransition {
    pub state: String,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A2aPart {
    #[serde(rename = "type")]
    pub part_type: String,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl A2aPart {
    pub fn text(content: impl Into<String>) -> Self {
        Self {
            part_type: "text".to_string(),
            kind: "text".to_string(),
            text: Some(content.into()),
            data: None,
        }
    }

    pub fn data(val: Value) -> Self {
        Self {
            part_type: "data".to_string(),
            kind: "data".to_string(),
            text: None,
            data: Some(val),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A2aMessage {
    pub role: String,
    pub parts: Vec<A2aPart>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A2aTaskStatus {
    pub state: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<A2aMessage>,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A2aArtifact {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub parts: Vec<A2aPart>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct A2aTask {
    pub id: String,
    pub session_id: String,
    pub status: A2aTaskStatus,
    #[serde(default)]
    pub history: Vec<A2aMessage>,
    #[serde(default)]
    pub artifacts: Vec<A2aArtifact>,
    #[serde(default)]
    pub state_transition_history: Vec<A2aStateTransition>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Value>,
}

static A2A_TASKS: LazyLock<Mutex<VecDeque<A2aTask>>> =
    LazyLock::new(|| Mutex::new(VecDeque::with_capacity(MAX_A2A_TASKS)));

static ACTIVE_ABORTS: LazyLock<Mutex<HashMap<String, AbortHandle>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339()
}

fn next_task_id() -> String {
    static TASK_SEQ: AtomicU64 = AtomicU64::new(1);
    let seq = TASK_SEQ.fetch_add(1, Ordering::Relaxed);
    format!("a2a-task-{}-{seq}", chrono::Utc::now().timestamp_millis())
}

/// Build the A2A v0.2.1 Agent Card served at `GET /.well-known/agent.json`.
pub fn build_agent_card() -> Value {
    json!({
        "name": "ColimaUI DevOps & Container Security Agent",
        "description": "Local-first DevOps & Container Security AI agent managing Colima VMs, Docker/containerd workloads, Compose stacks, Kubernetes/Kind clusters, Lima VMs, Trivy/Falco security audits, and live ColimaUI navigation.",
        "version": env!("CARGO_PKG_VERSION"),
        "url": A2A_DEFAULT_URL,
        "protocolVersion": A2A_PROTOCOL_VERSION,
        "capabilities": {
            "streaming": false,
            "pushNotifications": false,
            "stateTransitionHistory": true
        },
        "authentication": {
            "schemes": ["Bearer"]
        },
        "defaultInputModes": ["text/plain", "application/json"],
        "defaultOutputModes": ["text/plain", "application/json"],
        "skills": [
            {
                "id": "colima-vm-lifecycle",
                "name": "Colima & Environment Lifecycle",
                "description": "Inspect, start, stop, configure (colima.yaml), and diagnose Colima profiles and XDG/socket environment provenance.",
                "tags": ["colima", "vm", "lima", "xdg", "diagnostics"],
                "examples": [
                    "Check Colima VM status and XDG environment resolutions",
                    "Show colima.yaml configuration for the default profile"
                ],
                "inputModes": ["text/plain", "application/json"],
                "outputModes": ["text/plain", "application/json"]
            },
            {
                "id": "docker-workloads",
                "name": "Docker Containers, Images, Volumes & Networks",
                "description": "Inspect and manage running containers, logs, stats, images, volumes, networks, and live topology graphs.",
                "tags": ["docker", "containers", "images", "volumes", "networks", "topology"],
                "examples": [
                    "List all containers and inspect logs for failing services",
                    "Show the Docker container and network topology"
                ],
                "inputModes": ["text/plain", "application/json"],
                "outputModes": ["text/plain", "application/json"]
            },
            {
                "id": "compose-orchestration",
                "name": "Docker Compose Orchestration & Autofix",
                "description": "Manage Compose stacks, run offline schema + Knowledge Bank diagnostics, and propose/apply reversible YAML autofixes.",
                "tags": ["compose", "docker-compose", "diagnose", "autofix"],
                "examples": [
                    "List active Docker Compose projects and check service status",
                    "Diagnose issues in ./docker-compose.yml"
                ],
                "inputModes": ["text/plain", "application/json"],
                "outputModes": ["text/plain", "application/json"]
            },
            {
                "id": "kubernetes-cluster",
                "name": "Kubernetes & Kind Cluster Management",
                "description": "Inspect K8s contexts, nodes, namespaces, pods, deployments, CRDs, events, logs, and manage rollouts, scaling, and port-forwards.",
                "tags": ["kubernetes", "k8s", "k3s", "kind", "kubectl"],
                "examples": [
                    "Check Kubernetes cluster health and list pods across all namespaces",
                    "Fetch recent warning events in kube-system"
                ],
                "inputModes": ["text/plain", "application/json"],
                "outputModes": ["text/plain", "application/json"]
            },
            {
                "id": "container-security",
                "name": "Container Vulnerability, Posture & Runtime Security",
                "description": "Run local Trivy CVE scans, SBOM exports, configuration rule-pack audits, prioritized remediation triage, Falco runtime checks, and isolated Detonation sessions.",
                "tags": ["security", "trivy", "sbom", "falco", "detonation", "cve"],
                "examples": [
                    "Check Falco runtime detection state and recent security score history",
                    "Suggest hardened base-image alternatives for node:latest"
                ],
                "inputModes": ["text/plain", "application/json"],
                "outputModes": ["text/plain", "application/json"]
            },
            {
                "id": "ui-remote-control",
                "name": "Live Desktop & Web UI Remote Control",
                "description": "Navigate the ColimaUI Desktop and Browser interface to any of the 18 pages with contextual section focus and toast notifications.",
                "tags": ["ui", "navigation", "remote-control", "sse"],
                "examples": [
                    "Open the Security page in ColimaUI",
                    "Navigate to the Topology view and summarize active containers"
                ],
                "inputModes": ["text/plain", "application/json"],
                "outputModes": ["text/plain", "application/json"]
            }
        ]
    })
}

/// Insert or replace a task in the bounded task store (`MAX_A2A_TASKS = 128`).
pub fn upsert_task(task: A2aTask, abort_handle: Option<AbortHandle>) {
    {
        let mut store = A2A_TASKS.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(pos) = store.iter().position(|t| t.id == task.id) {
            store[pos] = task.clone();
        } else {
            while store.len() >= MAX_A2A_TASKS {
                let Some(evicted) = store.pop_front() else {
                    break;
                };
                if let Ok(mut aborts) = ACTIVE_ABORTS.lock() {
                    aborts.remove(&evicted.id);
                }
            }
            store.push_back(task.clone());
        }
    }

    if let Ok(mut aborts) = ACTIVE_ABORTS.lock() {
        match abort_handle {
            Some(handle) => {
                aborts.insert(task.id, handle);
            }
            None => {
                aborts.remove(&task.id);
            }
        }
    }
}

/// Retrieve a snapshot of a stored task by `id`.
pub fn get_task(task_id: &str) -> Option<A2aTask> {
    let store = A2A_TASKS.lock().unwrap_or_else(|e| e.into_inner());
    store.iter().find(|t| t.id == task_id).cloned()
}

/// Cancel a task by `id`, aborting its active Tokio task if still running.
pub fn cancel_task(task_id: &str) -> Option<A2aTask> {
    let maybe_abort = ACTIVE_ABORTS
        .lock()
        .ok()
        .and_then(|mut aborts| aborts.remove(task_id));
    if let Some(handle) = maybe_abort {
        handle.abort();
    }

    let mut store = A2A_TASKS.lock().unwrap_or_else(|e| e.into_inner());
    let task = store.iter_mut().find(|t| t.id == task_id)?;
    let ts = now_rfc3339();
    if task.status.state != "canceled" {
        task.status = A2aTaskStatus {
            state: "canceled".to_string(),
            message: Some(A2aMessage {
                role: "agent".to_string(),
                parts: vec![A2aPart::text("Task canceled by client request.")],
            }),
            timestamp: ts.clone(),
        };
        task.state_transition_history.push(A2aStateTransition {
            state: "canceled".to_string(),
            timestamp: ts,
        });
    }
    Some(task.clone())
}

fn jsonrpc_ok(id: Value, result: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result,
    })
}

fn jsonrpc_err(id: Value, code: i64, message: impl Into<String>) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": code,
            "message": message.into(),
        }
    })
}

/// Extract text prompt from `params.message.parts` (`type: "text"` or `kind: "text"`)
/// or fallback `params.prompt` / `params.message.content`.
pub fn extract_prompt_from_params(params: &Value) -> Option<String> {
    if let Some(parts) = params
        .get("message")
        .and_then(|m| m.get("parts"))
        .and_then(Value::as_array)
    {
        let mut texts = Vec::new();
        for part in parts {
            let is_text = part
                .get("type")
                .or_else(|| part.get("kind"))
                .and_then(Value::as_str)
                .is_none_or(|k| k.eq_ignore_ascii_case("text"));
            if !is_text {
                continue;
            }
            if let Some(trimmed) = part
                .get("text")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                texts.push(trimmed);
            }
        }
        if !texts.is_empty() {
            return Some(texts.join("\n"));
        }
    }

    if let Some(content) = params
        .get("message")
        .and_then(|m| m.get("content"))
        .and_then(Value::as_str)
    {
        let trimmed = content.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }

    if let Some(prompt) = params.get("prompt").and_then(Value::as_str) {
        let trimmed = prompt.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }

    None
}

fn extract_meta_bool(params: &Value, camel: &str, snake: &str) -> bool {
    let from_meta = params
        .get("metadata")
        .or_else(|| params.get("message").and_then(|m| m.get("metadata")));

    from_meta
        .and_then(|m| m.get(camel).or_else(|| m.get(snake)))
        .or_else(|| params.get(camel).or_else(|| params.get(snake)))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

fn extract_meta_str(params: &Value, key: &str) -> Option<String> {
    let from_meta = params
        .get("metadata")
        .or_else(|| params.get("message").and_then(|m| m.get("metadata")));

    from_meta
        .and_then(|m| m.get(key))
        .or_else(|| params.get(key))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

fn build_artifacts(
    tools_executed: &[ToolExecutionRecord],
    navigated_to: Option<&str>,
    provider: &str,
    model: &str,
) -> Vec<A2aArtifact> {
    vec![A2aArtifact {
        name: "execution-metadata".to_string(),
        description: Some("Executed ColimaUI tools, UI navigation, and model metadata".to_string()),
        parts: vec![A2aPart::data(json!({
            "tools_executed": tools_executed,
            "navigated_to": navigated_to,
            "provider": provider,
            "model": model,
        }))],
    }]
}

async fn handle_send(req_id: Value, params: &Value) -> Value {
    let Some(prompt) = extract_prompt_from_params(params) else {
        return jsonrpc_err(
            req_id,
            -32602,
            "Invalid params: missing text prompt in params.message.parts or params.prompt",
        );
    };

    let task_id = params
        .get("id")
        .or_else(|| params.get("taskId"))
        .or_else(|| params.get("message").and_then(|m| m.get("taskId")))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map_or_else(next_task_id, str::to_string);

    let session_id = params
        .get("sessionId")
        .or_else(|| params.get("session_id"))
        .or_else(|| params.get("contextId"))
        .or_else(|| params.get("message").and_then(|m| m.get("contextId")))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(A2A_DEFAULT_CONVERSATION_ID)
        .to_string();

    let allow_actions = extract_meta_bool(params, "allowActions", "allow_actions");
    let mirror_ui = extract_meta_bool(params, "mirrorUi", "mirror_ui");
    let provider = extract_meta_str(params, "provider");
    let model = extract_meta_str(params, "model");

    let submitted_ts = now_rfc3339();
    let user_msg = A2aMessage {
        role: "user".to_string(),
        parts: vec![A2aPart::text(prompt.clone())],
    };

    let initial_task = A2aTask {
        id: task_id.clone(),
        session_id: session_id.clone(),
        status: A2aTaskStatus {
            state: "working".to_string(),
            message: None,
            timestamp: submitted_ts.clone(),
        },
        history: vec![user_msg.clone()],
        artifacts: Vec::new(),
        state_transition_history: vec![
            A2aStateTransition {
                state: "submitted".to_string(),
                timestamp: submitted_ts.clone(),
            },
            A2aStateTransition {
                state: "working".to_string(),
                timestamp: submitted_ts,
            },
        ],
        metadata: Some(json!({
            "allowActions": allow_actions,
            "mirrorUi": mirror_ui,
        })),
    };

    let headless_req = HeadlessChatRequest {
        prompt,
        provider,
        model,
        allow_actions,
        session_id: Some(session_id.clone()),
        mirror_ui,
    };

    let join_handle = tokio::spawn(run_headless_agent_detailed(headless_req));
    upsert_task(initial_task, Some(join_handle.abort_handle()));

    let join_res = join_handle.await;

    // Remove finished abort handle.
    if let Ok(mut aborts) = ACTIVE_ABORTS.lock() {
        aborts.remove(&task_id);
    }

    match join_res {
        Ok(Ok(outcome)) => {
            let needs_input = outcome.reply.contains("ACTION_BLOCKED")
                || outcome.tools_executed.iter().any(|t| t.status == "blocked");
            let final_state = if needs_input {
                "input-required"
            } else {
                "completed"
            };
            let done_ts = now_rfc3339();
            let agent_msg = A2aMessage {
                role: "agent".to_string(),
                parts: vec![A2aPart::text(outcome.reply.clone())],
            };
            let artifacts = build_artifacts(
                &outcome.tools_executed,
                outcome.navigated_to.as_deref(),
                &outcome.provider,
                &outcome.model,
            );

            let mut transitions = get_task(&task_id)
                .map(|t| t.state_transition_history)
                .unwrap_or_default();
            transitions.push(A2aStateTransition {
                state: final_state.to_string(),
                timestamp: done_ts.clone(),
            });

            let completed_task = A2aTask {
                id: task_id,
                session_id: outcome.session_id,
                status: A2aTaskStatus {
                    state: final_state.to_string(),
                    message: Some(agent_msg.clone()),
                    timestamp: done_ts,
                },
                history: vec![user_msg, agent_msg],
                artifacts,
                state_transition_history: transitions,
                metadata: Some(json!({
                    "allowActions": allow_actions,
                    "mirrorUi": mirror_ui,
                    "provider": outcome.provider,
                    "model": outcome.model,
                    "navigatedTo": outcome.navigated_to,
                })),
            };
            upsert_task(completed_task.clone(), None);
            jsonrpc_ok(
                req_id,
                serde_json::to_value(completed_task).unwrap_or_else(|_| json!({})),
            )
        }
        Ok(Err(err_msg)) => {
            let fail_ts = now_rfc3339();
            let fail_msg = A2aMessage {
                role: "agent".to_string(),
                parts: vec![A2aPart::text(err_msg)],
            };
            let mut transitions = get_task(&task_id)
                .map(|t| t.state_transition_history)
                .unwrap_or_default();
            transitions.push(A2aStateTransition {
                state: "failed".to_string(),
                timestamp: fail_ts.clone(),
            });

            let failed_task = A2aTask {
                id: task_id,
                session_id,
                status: A2aTaskStatus {
                    state: "failed".to_string(),
                    message: Some(fail_msg.clone()),
                    timestamp: fail_ts,
                },
                history: vec![user_msg, fail_msg],
                artifacts: Vec::new(),
                state_transition_history: transitions,
                metadata: Some(json!({
                    "allowActions": allow_actions,
                    "mirrorUi": mirror_ui,
                })),
            };
            upsert_task(failed_task.clone(), None);
            jsonrpc_ok(
                req_id,
                serde_json::to_value(failed_task).unwrap_or_else(|_| json!({})),
            )
        }
        Err(join_err) if join_err.is_cancelled() => {
            let canceled = cancel_task(&task_id).unwrap_or_else(|| A2aTask {
                id: task_id,
                session_id,
                status: A2aTaskStatus {
                    state: "canceled".to_string(),
                    message: None,
                    timestamp: now_rfc3339(),
                },
                history: vec![user_msg],
                artifacts: Vec::new(),
                state_transition_history: Vec::new(),
                metadata: None,
            });
            jsonrpc_ok(
                req_id,
                serde_json::to_value(canceled).unwrap_or_else(|_| json!({})),
            )
        }
        Err(join_err) => jsonrpc_err(req_id, -32603, format!("Internal task error: {join_err}")),
    }
}

/// Handle a JSON-RPC 2.0 request sent to `POST /a2a`.
pub async fn handle_a2a_jsonrpc(body: Value) -> Value {
    let req_id = body.get("id").cloned().unwrap_or(Value::Null);
    let jsonrpc_ver = body.get("jsonrpc").and_then(Value::as_str).unwrap_or("");
    if jsonrpc_ver != "2.0" {
        return jsonrpc_err(req_id, -32600, "Invalid Request: expected jsonrpc '2.0'");
    }

    let Some(method) = body.get("method").and_then(Value::as_str) else {
        return jsonrpc_err(req_id, -32600, "Invalid Request: missing 'method'");
    };
    let params = body.get("params").cloned().unwrap_or_else(|| json!({}));

    match method {
        "message/send" | "tasks/send" => handle_send(req_id, &params).await,
        "tasks/get" => {
            let Some(id) = params
                .get("id")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
            else {
                return jsonrpc_err(req_id, -32602, "Invalid params: missing task 'id'");
            };
            match get_task(id) {
                Some(task) => {
                    jsonrpc_ok(req_id, serde_json::to_value(task).unwrap_or_else(|_| json!({})))
                }
                None => jsonrpc_err(req_id, -32001, format!("Task not found: {id}")),
            }
        }
        "tasks/cancel" => {
            let Some(id) = params
                .get("id")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
            else {
                return jsonrpc_err(req_id, -32602, "Invalid params: missing task 'id'");
            };
            match cancel_task(id) {
                Some(task) => {
                    jsonrpc_ok(req_id, serde_json::to_value(task).unwrap_or_else(|_| json!({})))
                }
                None => jsonrpc_err(req_id, -32001, format!("Task not found: {id}")),
            }
        }
        other => jsonrpc_err(req_id, -32601, format!("Method not found: {other}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_card_contains_required_a2a_metadata_and_six_skills() {
        let card = build_agent_card();
        assert_eq!(card["name"], "ColimaUI DevOps & Container Security Agent");
        assert_eq!(card["version"], env!("CARGO_PKG_VERSION"));
        assert_eq!(card["url"], "http://127.0.0.1:11420/a2a");
        assert_eq!(card["protocolVersion"], "0.2.1");
        assert_eq!(card["capabilities"]["streaming"], false);
        assert_eq!(card["capabilities"]["pushNotifications"], false);
        assert_eq!(card["capabilities"]["stateTransitionHistory"], true);
        assert_eq!(card["authentication"]["schemes"], json!(["Bearer"]));
        assert_eq!(card["skills"].as_array().map(Vec::len), Some(6));
    }

    #[test]
    fn extracts_text_from_both_type_and_kind_a2a_message_parts() {
        let by_type = json!({
            "message": {
                "role": "user",
                "parts": [{"type": "text", "text": "Check Colima status"}]
            }
        });
        assert_eq!(
            extract_prompt_from_params(&by_type).as_deref(),
            Some("Check Colima status")
        );

        let by_kind = json!({
            "message": {
                "role": "user",
                "parts": [{"kind": "text", "text": "List containers"}]
            }
        });
        assert_eq!(
            extract_prompt_from_params(&by_kind).as_deref(),
            Some("List containers")
        );
    }

    #[tokio::test]
    async fn task_store_bounds_capacity_and_supports_get_and_cancel() {
        for idx in 0..(MAX_A2A_TASKS + 10) {
            let id = format!("cap-task-{idx}");
            upsert_task(
                A2aTask {
                    id,
                    session_id: A2A_DEFAULT_CONVERSATION_ID.to_string(),
                    status: A2aTaskStatus {
                        state: "working".to_string(),
                        message: None,
                        timestamp: now_rfc3339(),
                    },
                    history: Vec::new(),
                    artifacts: Vec::new(),
                    state_transition_history: Vec::new(),
                    metadata: None,
                },
                None,
            );
        }

        {
            let store = A2A_TASKS.lock().unwrap();
            assert!(store.len() <= MAX_A2A_TASKS);
        }

        let target_id = format!("cap-task-{}", MAX_A2A_TASKS + 9);
        let get_resp = handle_a2a_jsonrpc(json!({
            "jsonrpc": "2.0",
            "id": 10,
            "method": "tasks/get",
            "params": { "id": target_id }
        }))
        .await;
        assert_eq!(get_resp["result"]["id"], target_id);
        assert_eq!(get_resp["result"]["status"]["state"], "working");

        let cancel_resp = handle_a2a_jsonrpc(json!({
            "jsonrpc": "2.0",
            "id": 11,
            "method": "tasks/cancel",
            "params": { "id": target_id }
        }))
        .await;
        assert_eq!(cancel_resp["result"]["status"]["state"], "canceled");

        let missing_resp = handle_a2a_jsonrpc(json!({
            "jsonrpc": "2.0",
            "id": 12,
            "method": "tasks/get",
            "params": { "id": "does-not-exist-xyz" }
        }))
        .await;
        assert_eq!(missing_resp["error"]["code"], -32001);

        let unknown_method = handle_a2a_jsonrpc(json!({
            "jsonrpc": "2.0",
            "id": 13,
            "method": "tasks/nonexistent"
        }))
        .await;
        assert_eq!(unknown_method["error"]["code"], -32601);
    }
}
