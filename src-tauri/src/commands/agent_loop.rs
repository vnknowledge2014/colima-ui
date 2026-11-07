use std::collections::HashMap;

use futures_util::FutureExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::commands::ai_chat::AiChatRequest;
pub use crate::commands::ai_chat::ChatMessage;
use crate::commands::{
    colima, compose, containers, k8s_cluster, kubernetes, lima, models, networks, resolution,
    system, volumes,
};

const MAX_AGENT_ROUNDS: usize = 5;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeadlessChatRequest {
    pub prompt: String,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default, alias = "allow_mutations")]
    pub allow_actions: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedAiConfig {
    pub provider: String,
    pub model: String,
    pub api_key: String,
    pub endpoint: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TagKind {
    Query,
    Event,
    EventApprove,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ToolTag {
    pub kind: TagKind,
    pub event: String,
    pub payload: Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventSafety {
    Safe,
    Mutating,
    Unknown,
}

fn non_empty_trimmed(val: Option<&String>) -> Option<String> {
    val.map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

pub fn default_model_for_provider(provider: &str) -> &'static str {
    match provider {
        "ollama-local" | "ollama-cloud" | "ollama" => "qwen2.5-coder:7b",
        "openai" | "openai-compatible" => "gpt-4o-mini",
        "anthropic" => "claude-sonnet-4-6",
        "gemini" => "gemini-2.5-flash",
        _ => "qwen2.5-coder:7b",
    }
}

pub fn resolve_ai_config(
    req: &HeadlessChatRequest,
    settings: &HashMap<String, String>,
) -> ResolvedAiConfig {
    let raw_provider = non_empty_trimmed(req.provider.as_ref())
        .or_else(|| non_empty_trimmed(settings.get("ai_provider")))
        .unwrap_or_else(|| "ollama-local".to_string());

    let provider = if raw_provider == "ollama" {
        "ollama-local".to_string()
    } else {
        raw_provider
    };

    let scoped_model_key = format!("ai_model.{provider}");
    let model = non_empty_trimmed(req.model.as_ref())
        .or_else(|| non_empty_trimmed(settings.get(&scoped_model_key)))
        .or_else(|| non_empty_trimmed(settings.get("ai_model")))
        .unwrap_or_else(|| default_model_for_provider(&provider).to_string());

    let scoped_key_name = format!("ai_api_key.{provider}");
    let api_key = non_empty_trimmed(settings.get(&scoped_key_name))
        .or_else(|| non_empty_trimmed(settings.get("ai_api_key")))
        .unwrap_or_default();

    let scoped_endpoint_name = format!("ai_endpoint.{provider}");
    let endpoint = non_empty_trimmed(settings.get(&scoped_endpoint_name))
        .or_else(|| non_empty_trimmed(settings.get("ai_endpoint")))
        .unwrap_or_default();

    ResolvedAiConfig {
        provider,
        model,
        api_key,
        endpoint,
    }
}

async fn load_ai_settings() -> HashMap<String, String> {
    let unwind_res =
        std::panic::AssertUnwindSafe(crate::commands::knowledge_bank::get_all_settings())
            .catch_unwind()
            .await;

    if let Ok(Ok(map)) = unwind_res {
        return map;
    }

    // Standalone fallback when `init_knowledge_bank()` was not called in this process.
    let db_path = crate::path_util::app_data_dir().join("knowledge.db");
    if !db_path.exists() {
        return HashMap::new();
    }

    tokio::task::spawn_blocking(move || {
        let mut map = HashMap::new();
        let Ok(conn) = rusqlite::Connection::open_with_flags(
            &db_path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        ) else {
            return map;
        };
        let Ok(mut stmt) = conn.prepare("SELECT setting_key, setting_value FROM app_settings")
        else {
            return map;
        };
        if let Ok(rows) = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        }) {
            for (k, v) in rows.filter_map(Result::ok) {
                map.insert(k, v);
            }
        }
        map
    })
    .await
    .unwrap_or_default()
}

fn build_headless_system_prompt(allow_actions: bool) -> String {
    let mutation_policy = if allow_actions {
        "Mutating actions ARE enabled for this session (`allow_actions=true`). Emit `[EVENT_APPROVE: <event> | <json>]` when the user asks to perform a state-changing operation."
    } else {
        "Mutating actions are DISABLED (`allow_actions=false`). Read-only `[QUERY: ...]` tags run automatically, whereas mutating `[EVENT_APPROVE: ...]` tags will be blocked unless the user re-runs with `--yes`."
    };

    format!(
        "You are ColimaUI's Headless DevOps Agent managing Colima VMs, Docker/containerd, Compose, Kubernetes, and Lima.\n\
         {mutation_policy}\n\n\
         ## Tool Invocation Syntax\n\
         - Read-only inspection: `[QUERY: <event> | <json_object>]`\n\
         - Mutating action: `[EVENT_APPROVE: <event> | <json_object>]`\n\n\
         ## Read-Only SAFE Queries\n\
         - `list-instances` | `{{}}`\n\
         - `colima-status` | `{{\"profile\": \"default\"}}`\n\
         - `list-containers` | `{{\"all\": true}}`\n\
         - `container-logs` | `{{\"id\": \"<container>\", \"lines\": 100}}`\n\
         - `inspect-container` | `{{\"id\": \"<container>\"}}`\n\
         - `list-images` | `{{}}`\n\
         - `inspect-image` | `{{\"id\": \"<image>\"}}`\n\
         - `volume-list` | `{{}}`\n\
         - `network-list` | `{{}}`\n\
         - `compose-list` | `{{}}`\n\
         - `system-df` | `{{}}`\n\
         - `system-check` | `{{}}`\n\
         - `system-host-specs` | `{{}}`\n\
         - `environment-resolutions` | `{{}}`\n\
         - `diagnostic-logs` | `{{\"profile\": \"default\"}}`\n\
         - `k8s-cluster-health` | `{{}}`\n\
         - `k8s-nodes` | `{{}}`\n\
         - `k8s-pods` | `{{\"namespace\": \"all\"}}`\n\
         - `lima-list` | `{{}}`\n\
         - `model-list` | `{{\"profile\": \"default\"}}`\n\n\
         ## Mutating Events\n\
         - `start-instance` | `{{\"profile\": \"default\"}}` (omit cpu/memory/disk/runtime unless explicitly requested so colima.yaml is preserved)\n\
         - `stop-instance` | `{{\"profile\": \"default\"}}`\n\
         - `colima-restart` | `{{\"profile\": \"default\"}}`\n\
         - `start-container` | `{{\"id\": \"<container>\"}}`\n\
         - `stop-container` | `{{\"id\": \"<container>\"}}`\n\
         - `restart-container` | `{{\"id\": \"<container>\"}}`\n\
         - `pause-container` | `{{\"id\": \"<container>\"}}`\n\
         - `unpause-container` | `{{\"id\": \"<container>\"}}`\n\
         - `pull-image` | `{{\"image\": \"<repo:tag>\"}}`\n\
         - `compose-up` | `{{\"dir\": \"<compose_file_or_dir>\", \"detach\": true}}`\n\
         - `compose-down` | `{{\"projectName\": \"<project>\"}}`\n\
         - `compose-restart` | `{{\"projectName\": \"<project>\"}}`\n\
         - `lima-start` | `{{\"name\": \"<vm>\"}}`\n\
         - `lima-stop` | `{{\"name\": \"<vm>\"}}`\n\n\
         ## Rules\n\
         1. Emit ONLY valid tool tags when you need live state or need to run an action.\n\
         2. Once tool results are provided, do NOT emit further tool tags unless another step is needed; answer concisely and accurately in the same language as the user's prompt."
    )
}

fn match_tag_prefix(slice: &str) -> Option<(TagKind, usize)> {
    const PREFIXES: &[(&str, TagKind)] = &[
        ("[EVENT_APPROVE:", TagKind::EventApprove),
        ("[QUERY:", TagKind::Query),
        ("[EVENT:", TagKind::Event),
    ];
    for &(prefix, kind) in PREFIXES {
        if slice
            .get(..prefix.len())
            .is_some_and(|head| head.eq_ignore_ascii_case(prefix))
        {
            return Some((kind, prefix.len()));
        }
    }
    None
}

fn find_matching_closing_bracket(s: &str) -> Option<usize> {
    let mut depth = 1usize;
    let mut in_string = false;
    let mut escaped = false;

    for (idx, ch) in s.char_indices() {
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }

        match ch {
            '"' => in_string = true,
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 {
                    return Some(idx);
                }
            }
            _ => {}
        }
    }
    None
}

/// Extract all `[QUERY: ...]`, `[EVENT: ...]`, and `[EVENT_APPROVE: ...]` tags from an LLM turn.
pub fn extract_tool_tags(text: &str) -> Vec<ToolTag> {
    let mut tags = Vec::new();
    let mut cursor = 0usize;

    while let Some(rel_open) = text[cursor..].find('[') {
        let tag_start = cursor + rel_open;
        let Some((kind, prefix_len)) = match_tag_prefix(&text[tag_start..]) else {
            cursor = tag_start + 1;
            continue;
        };

        let inner_start = tag_start + prefix_len;
        let Some(rel_close) = find_matching_closing_bracket(&text[inner_start..]) else {
            break;
        };

        let inner = text[inner_start..inner_start + rel_close].trim();
        let (event_raw, payload) = match inner.split_once('|') {
            Some((ev, json_part)) => {
                let parsed = serde_json::from_str::<Value>(json_part.trim())
                    .unwrap_or_else(|_| serde_json::json!({}));
                (ev.trim(), parsed)
            }
            None => (inner, serde_json::json!({})),
        };

        if !event_raw.is_empty() {
            tags.push(ToolTag {
                kind,
                event: event_raw.to_string(),
                payload,
            });
        }

        cursor = inner_start + rel_close + 1;
    }

    tags
}

/// Strip all recognized tool tags from text, returning clean prose.
pub fn strip_tool_tags(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut cursor = 0usize;

    while let Some(rel_open) = text[cursor..].find('[') {
        let tag_start = cursor + rel_open;
        let Some((_, prefix_len)) = match_tag_prefix(&text[tag_start..]) else {
            out.push_str(&text[cursor..=tag_start]);
            cursor = tag_start + 1;
            continue;
        };

        out.push_str(&text[cursor..tag_start]);
        let inner_start = tag_start + prefix_len;
        if let Some(rel_close) = find_matching_closing_bracket(&text[inner_start..]) {
            cursor = inner_start + rel_close + 1;
        } else {
            // Drop trailing unclosed tool tag if present.
            cursor = text.len();
            break;
        }
    }

    if cursor < text.len() {
        out.push_str(&text[cursor..]);
    }

    out.trim().to_string()
}

pub fn classify_event(event: &str) -> EventSafety {
    match event {
        "list-instances"
        | "colima-status"
        | "list-containers"
        | "container-logs"
        | "inspect-container"
        | "container-stats"
        | "all-container-stats"
        | "list-images"
        | "inspect-image"
        | "volume-list"
        | "network-list"
        | "compose-list"
        | "compose-logs"
        | "system-df"
        | "system-check"
        | "system-host-specs"
        | "environment-resolutions"
        | "diagnostic-logs"
        | "k8s-cluster-health"
        | "k8s-nodes"
        | "k8s-pods"
        | "lima-list"
        | "model-list" => EventSafety::Safe,

        "start-instance"
        | "stop-instance"
        | "colima-restart"
        | "start-container"
        | "stop-container"
        | "restart-container"
        | "pause-container"
        | "unpause-container"
        | "remove-container"
        | "pull-image"
        | "compose-up"
        | "compose-down"
        | "compose-restart"
        | "lima-start"
        | "lima-stop" => EventSafety::Mutating,

        _ => EventSafety::Unknown,
    }
}

fn str_field<'a>(payload: &'a Value, keys: &[&str]) -> Option<&'a str> {
    for key in keys {
        if let Some(s) = payload.get(*key).and_then(Value::as_str) {
            let trimmed = s.trim();
            if !trimmed.is_empty() {
                return Some(trimmed);
            }
        }
    }
    None
}

fn u32_field(payload: &Value, keys: &[&str], default: u32) -> u32 {
    for key in keys {
        if let Some(n) = payload.get(*key).and_then(Value::as_u64) {
            return u32::try_from(n).unwrap_or(default);
        }
    }
    default
}

/// Build `StartConfig` preserving existing `colima.yaml` (`cpus: 0, memory: 0, disk: 0, runtime: ""` unless specified).
pub fn build_start_config_from_payload(payload: &Value) -> colima::StartConfig {
    let mut config: colima::StartConfig =
        serde_json::from_value(payload.clone()).unwrap_or(colima::StartConfig {
            profile: String::new(),
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
        });

    if config.profile.trim().is_empty() {
        config.profile = "default".to_string();
    }
    config
}

async fn start_colima_preserving_yaml(payload: &Value) -> Result<String, String> {
    let mut config = build_start_config_from_payload(payload);
    let yaml_exists = crate::instance_reader::colima_home()
        .join(&config.profile)
        .join("colima.yaml")
        .exists();

    if !yaml_exists {
        if config.runtime.is_empty() {
            config.runtime = "docker".to_string();
        }
        if config.cpus == 0 {
            config.cpus = 2;
        }
        if config.memory == 0 {
            config.memory = 2;
        }
        if config.disk == 0 {
            config.disk = 60;
        }
    }

    colima::start_instance(config)
        .await
        .map_err(|e| e.to_string())
}

pub fn format_mutation_blocked_note(event: &str, payload: &Value) -> String {
    let payload_compact = serde_json::to_string(payload).unwrap_or_else(|_| "{}".to_string());
    format!(
        "ACTION_BLOCKED: Mutating event '{event}' (payload: {payload_compact}) was NOT executed because `allow_actions` is false. \
         Mutations require `--yes` in CLI mode (e.g. `cui --yes \"<prompt>\"`). \
         Do NOT emit any more tool tags; explain the required action/command clearly to the user and mention `--yes`."
    )
}

/// Gate and execute a single parsed tool tag.
pub async fn dispatch_tool_tag(tag: &ToolTag, allow_actions: bool) -> Result<String, String> {
    match classify_event(&tag.event) {
        EventSafety::Safe => execute_safe_query(&tag.event, &tag.payload).await,
        EventSafety::Mutating => {
            if !allow_actions {
                return Ok(format_mutation_blocked_note(&tag.event, &tag.payload));
            }
            execute_mutating_event(&tag.event, &tag.payload).await
        }
        EventSafety::Unknown => Err(format!(
            "Unsupported event '{}'. Use one of the documented SAFE queries or MUTATING events.",
            tag.event
        )),
    }
}

async fn execute_safe_query(event: &str, payload: &Value) -> Result<String, String> {
    match event {
        "list-instances" => {
            let instances = crate::instance_reader::list_instances_fast();
            serde_json::to_string_pretty(&instances).map_err(|e| e.to_string())
        }
        "colima-status" => {
            let profile = str_field(payload, &["profile", "name"]).unwrap_or("default");
            let status = colima::instance_status(profile.to_string())
                .await
                .map_err(|e| e.to_string())?;
            serde_json::to_string_pretty(&status).map_err(|e| e.to_string())
        }
        "list-containers" => {
            let all = payload.get("all").and_then(Value::as_bool).unwrap_or(true);
            let list = containers::list_containers_cli(all).await?;
            serde_json::to_string_pretty(&list).map_err(|e| e.to_string())
        }
        "container-logs" => {
            let id = str_field(payload, &["id", "container_id", "containerId"])
                .ok_or_else(|| "Missing 'id' for container-logs".to_string())?;
            let lines = u32_field(payload, &["lines", "tail"], 100);
            containers::container_logs(id.to_string(), lines)
                .await
                .map_err(|e| e.to_string())
        }
        "inspect-container" => {
            let id = str_field(payload, &["id", "container_id", "containerId"])
                .ok_or_else(|| "Missing 'id' for inspect-container".to_string())?;
            containers::inspect_container(id.to_string())
                .await
                .map_err(|e| e.to_string())
        }
        "container-stats" => {
            let id = str_field(payload, &["id", "container_id", "containerId"])
                .ok_or_else(|| "Missing 'id' for container-stats".to_string())?;
            containers::container_stats(id.to_string())
                .await
                .map_err(|e| e.to_string())
        }
        "all-container-stats" => containers::all_container_stats()
            .await
            .map_err(|e| e.to_string()),
        "list-images" => {
            let images = crate::services::container::ContainerService::auto_detect()
                .list_images()
                .await
                .map_err(|e| e.to_string())?;
            serde_json::to_string_pretty(&images).map_err(|e| e.to_string())
        }
        "inspect-image" => {
            let id = str_field(payload, &["id", "image_id", "imageId", "image"])
                .ok_or_else(|| "Missing 'id' for inspect-image".to_string())?;
            containers::inspect_image(id.to_string())
                .await
                .map_err(|e| e.to_string())
        }
        "volume-list" => {
            let vols = volumes::list_volumes().await.map_err(|e| e.to_string())?;
            serde_json::to_string_pretty(&vols).map_err(|e| e.to_string())
        }
        "network-list" => {
            let nets = networks::list_networks().await.map_err(|e| e.to_string())?;
            serde_json::to_string_pretty(&nets).map_err(|e| e.to_string())
        }
        "compose-list" => {
            let projects = compose::list_compose_projects()
                .await
                .map_err(|e| e.to_string())?;
            serde_json::to_string_pretty(&projects).map_err(|e| e.to_string())
        }
        "compose-logs" => {
            let project = str_field(payload, &["projectName", "project_name", "dir", "name"])
                .ok_or_else(|| "Missing 'projectName' for compose-logs".to_string())?;
            let lines = u32_field(payload, &["lines", "tail"], 100);
            compose::compose_logs(project.to_string(), lines)
                .await
                .map_err(|e| e.to_string())
        }
        "system-df" => containers::system_df().await.map_err(|e| e.to_string()),
        "system-check" => {
            let info = system::check_system().await.map_err(|e| e.to_string())?;
            serde_json::to_string_pretty(&info).map_err(|e| e.to_string())
        }
        "system-host-specs" => {
            let specs = tokio::task::spawn_blocking(system::detect_host_specs)
                .await
                .map_err(|e| format!("Task join error: {e}"))?;
            serde_json::to_string_pretty(&specs).map_err(|e| e.to_string())
        }
        "environment-resolutions" => {
            let report = tokio::task::spawn_blocking(resolution::get_environment_resolutions)
                .await
                .map_err(|e| format!("Task join error: {e}"))?;
            serde_json::to_string_pretty(&report).map_err(|e| e.to_string())
        }
        "diagnostic-logs" => {
            let profile = str_field(payload, &["profile", "name"]).unwrap_or("default");
            colima::collect_diagnostic_logs(profile.to_string())
                .await
                .map_err(|e| e.to_string())
        }
        "k8s-cluster-health" => k8s_cluster::k8s_cluster_health()
            .await
            .map_err(|e| e.to_string()),
        "k8s-nodes" => kubernetes::k8s_nodes().await.map_err(|e| e.to_string()),
        "k8s-pods" => {
            let ns = str_field(payload, &["namespace", "ns"]).unwrap_or("all");
            let pods = kubernetes::k8s_pods(ns.to_string())
                .await
                .map_err(|e| e.to_string())?;
            serde_json::to_string_pretty(&pods).map_err(|e| e.to_string())
        }
        "lima-list" => {
            let vms = lima::lima_list().await.map_err(|e| e.to_string())?;
            serde_json::to_string_pretty(&vms).map_err(|e| e.to_string())
        }
        "model-list" => {
            let profile = str_field(payload, &["profile"]).unwrap_or("default");
            let runner = str_field(payload, &["runner"]).map(str::to_string);
            let list = models::list_models(profile.to_string(), runner)
                .await
                .map_err(|e| e.to_string())?;
            serde_json::to_string_pretty(&list).map_err(|e| e.to_string())
        }
        _ => Err(format!("Unknown safe query: {event}")),
    }
}

async fn execute_mutating_event(event: &str, payload: &Value) -> Result<String, String> {
    match event {
        "start-instance" => start_colima_preserving_yaml(payload).await,
        "stop-instance" => {
            let profile = str_field(payload, &["profile", "name"]).unwrap_or("default");
            let force = payload
                .get("force")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            colima::stop_instance_cli(profile.to_string(), force)
                .await
                .map_err(|e| e.to_string())
        }
        "colima-restart" => {
            let profile = str_field(payload, &["profile", "name"]).unwrap_or("default");
            let force = payload
                .get("force")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let _ = colima::stop_instance_cli(profile.to_string(), force).await;
            start_colima_preserving_yaml(payload).await?;
            Ok(format!("Instance '{profile}' restarted"))
        }
        "start-container" => {
            let id = str_field(payload, &["id", "container_id", "containerId"])
                .ok_or_else(|| "Missing 'id' for start-container".to_string())?;
            containers::start_container(id.to_string())
                .await
                .map_err(|e| e.to_string())
        }
        "stop-container" => {
            let id = str_field(payload, &["id", "container_id", "containerId"])
                .ok_or_else(|| "Missing 'id' for stop-container".to_string())?;
            containers::stop_container(id.to_string())
                .await
                .map_err(|e| e.to_string())
        }
        "restart-container" => {
            let id = str_field(payload, &["id", "container_id", "containerId"])
                .ok_or_else(|| "Missing 'id' for restart-container".to_string())?;
            containers::restart_container(id.to_string())
                .await
                .map_err(|e| e.to_string())
        }
        "pause-container" => {
            let id = str_field(payload, &["id", "container_id", "containerId"])
                .ok_or_else(|| "Missing 'id' for pause-container".to_string())?;
            containers::pause_container(id.to_string())
                .await
                .map_err(|e| e.to_string())
        }
        "unpause-container" => {
            let id = str_field(payload, &["id", "container_id", "containerId"])
                .ok_or_else(|| "Missing 'id' for unpause-container".to_string())?;
            containers::unpause_container(id.to_string())
                .await
                .map_err(|e| e.to_string())
        }
        "remove-container" => {
            let id = str_field(payload, &["id", "container_id", "containerId"])
                .ok_or_else(|| "Missing 'id' for remove-container".to_string())?;
            let force = payload
                .get("force")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            containers::remove_container(id.to_string(), force)
                .await
                .map_err(|e| e.to_string())
        }
        "pull-image" => {
            let image = str_field(payload, &["image", "image_name", "imageName", "name"])
                .ok_or_else(|| "Missing 'image' for pull-image".to_string())?;
            crate::services::container::ContainerService::auto_detect()
                .pull_image(image)
                .await
                .map_err(|e| e.to_string())
        }
        "compose-up" => {
            let dir = str_field(payload, &["project_dir", "dir", "file"]).unwrap_or("");
            let detach = payload
                .get("detach")
                .and_then(Value::as_bool)
                .unwrap_or(true);
            compose::compose_up(dir.to_string(), detach)
                .await
                .map_err(|e| e.to_string())
        }
        "compose-down" => {
            let project = str_field(payload, &["project_name", "projectName", "name", "dir"])
                .ok_or_else(|| "Missing 'projectName' for compose-down".to_string())?;
            compose::compose_down(project.to_string())
                .await
                .map_err(|e| e.to_string())
        }
        "compose-restart" => {
            let project = str_field(payload, &["project_name", "projectName", "name", "dir"])
                .ok_or_else(|| "Missing 'projectName' for compose-restart".to_string())?;
            compose::compose_restart(project.to_string())
                .await
                .map_err(|e| e.to_string())
        }
        "lima-start" => {
            let name = str_field(payload, &["name", "profile"])
                .ok_or_else(|| "Missing 'name' for lima-start".to_string())?;
            lima::lima_start(name.to_string())
                .await
                .map_err(|e| e.to_string())
        }
        "lima-stop" => {
            let name = str_field(payload, &["name", "profile"])
                .ok_or_else(|| "Missing 'name' for lima-stop".to_string())?;
            lima::lima_stop(name.to_string())
                .await
                .map_err(|e| e.to_string())
        }
        _ => Err(format!("Unknown mutating event: {event}")),
    }
}

#[tauri::command]
pub async fn run_headless_agent(
    req: HeadlessChatRequest,
) -> Result<String, crate::error::ColimaError> {
    if req.prompt.trim().is_empty() {
        return Err(crate::error::ColimaError::validation(
            "Prompt must not be empty",
        ));
    }

    let settings = load_ai_settings().await;
    let resolved = resolve_ai_config(&req, &settings);

    let mut messages = vec![
        ChatMessage {
            role: "system".to_string(),
            content: build_headless_system_prompt(req.allow_actions),
        },
        ChatMessage {
            role: "user".to_string(),
            content: req.prompt.clone(),
        },
    ];

    let mut last_clean_reply = String::new();
    let mut last_tool_summary = String::new();

    for _round in 0..MAX_AGENT_ROUNDS {
        let llm_reply = crate::commands::ai_chat::ai_chat(AiChatRequest {
            provider: resolved.provider.clone(),
            model: resolved.model.clone(),
            api_key: resolved.api_key.clone(),
            messages: messages.clone(),
            endpoint: resolved.endpoint.clone(),
        })
        .await?;

        let tags = extract_tool_tags(&llm_reply);
        let cleaned = strip_tool_tags(&llm_reply);
        if !cleaned.is_empty() {
            last_clean_reply = cleaned.clone();
        }

        if tags.is_empty() {
            return Ok(llm_reply.trim().to_string());
        }

        messages.push(ChatMessage {
            role: "assistant".to_string(),
            content: llm_reply,
        });

        let mut sections = Vec::with_capacity(tags.len());
        for tag in &tags {
            match dispatch_tool_tag(tag, req.allow_actions).await {
                Ok(output) => {
                    sections.push(format!("### {} (ok)\n{}", tag.event, output));
                }
                Err(err) => {
                    sections.push(format!("### {} (error)\n{}", tag.event, err));
                }
            }
        }

        last_tool_summary = sections.join("\n\n");
        messages.push(ChatMessage {
            role: "user".to_string(),
            content: format!(
                "[TOOL_RESULTS]\n{last_tool_summary}\n[/TOOL_RESULTS]\n\
                 Provide the final concise response for the user in their language based on the results above."
            ),
        });
    }

    if !last_clean_reply.is_empty() {
        Ok(last_clean_reply)
    } else {
        Ok(last_tool_summary)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_query_event_and_event_approve_tags_including_nested_brackets() {
        let reply = r#"Let me inspect first.
[QUERY: list-containers | {"all": true}]
[EVENT: colima-status | {"profile": "dev"}]
[EVENT_APPROVE: start-instance | {"profile": "dev", "mounts": ["/Users/me/work:/work:w"], "note": "a]b"}]"#;

        let tags = extract_tool_tags(reply);
        assert_eq!(tags.len(), 3);

        assert_eq!(tags[0].kind, TagKind::Query);
        assert_eq!(tags[0].event, "list-containers");
        assert_eq!(tags[0].payload["all"], true);

        assert_eq!(tags[1].kind, TagKind::Event);
        assert_eq!(tags[1].event, "colima-status");
        assert_eq!(tags[1].payload["profile"], "dev");

        assert_eq!(tags[2].kind, TagKind::EventApprove);
        assert_eq!(tags[2].event, "start-instance");
        assert_eq!(tags[2].payload["profile"], "dev");
        assert_eq!(tags[2].payload["mounts"][0], "/Users/me/work:/work:w");
        assert_eq!(tags[2].payload["note"], "a]b");

        assert_eq!(strip_tool_tags(reply), "Let me inspect first.");
    }

    #[test]
    fn extracts_tag_without_json_payload() {
        let tags = extract_tool_tags("[QUERY: list-instances]");
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].event, "list-instances");
        assert_eq!(tags[0].payload, serde_json::json!({}));
    }

    #[test]
    fn classifies_safe_vs_mutating_events_by_event_name() {
        for safe in [
            "list-instances",
            "colima-status",
            "list-containers",
            "container-logs",
            "inspect-container",
            "list-images",
            "inspect-image",
            "volume-list",
            "network-list",
            "compose-list",
            "system-df",
            "system-check",
            "system-host-specs",
            "environment-resolutions",
            "diagnostic-logs",
            "k8s-cluster-health",
            "k8s-nodes",
            "k8s-pods",
            "lima-list",
            "model-list",
        ] {
            assert_eq!(
                classify_event(safe),
                EventSafety::Safe,
                "expected {safe} to be Safe"
            );
        }

        for mutating in [
            "start-instance",
            "stop-instance",
            "colima-restart",
            "start-container",
            "stop-container",
            "restart-container",
            "pause-container",
            "unpause-container",
            "pull-image",
            "compose-up",
            "compose-down",
            "compose-restart",
            "lima-start",
            "lima-stop",
        ] {
            assert_eq!(
                classify_event(mutating),
                EventSafety::Mutating,
                "expected {mutating} to be Mutating"
            );
        }

        assert_eq!(classify_event("nonexistent-event"), EventSafety::Unknown);
    }

    #[tokio::test]
    async fn blocks_mutating_events_when_allow_actions_is_false_even_in_query_tag() {
        // Even if the LLM mislabels a mutating action as [QUERY: ...], gating must block it.
        let tag = ToolTag {
            kind: TagKind::Query,
            event: "stop-container".to_string(),
            payload: serde_json::json!({"id": "redis-prod"}),
        };

        let out = dispatch_tool_tag(&tag, false)
            .await
            .expect("blocked mutation returns an instructional note to the LLM");
        assert!(out.contains("ACTION_BLOCKED"));
        assert!(out.contains("--yes"));
        assert!(out.contains("stop-container"));
    }

    #[tokio::test]
    async fn executes_safe_environment_resolutions_without_allow_actions() {
        let tag = ToolTag {
            kind: TagKind::Query,
            event: "environment-resolutions".to_string(),
            payload: serde_json::json!({}),
        };

        let out = dispatch_tool_tag(&tag, false)
            .await
            .expect("safe query should execute automatically");
        let parsed: Value = serde_json::from_str(&out).expect("valid JSON report");
        assert!(parsed["rows"].is_array());
    }

    #[test]
    fn start_config_defaults_to_zero_overrides_so_colima_yaml_wins() {
        let cfg = build_start_config_from_payload(&serde_json::json!({"profile": "work"}));
        assert_eq!(cfg.profile, "work");
        assert_eq!(cfg.cpus, 0);
        assert_eq!(cfg.memory, 0);
        assert_eq!(cfg.disk, 0);
        assert_eq!(cfg.runtime, "");
    }

    #[test]
    fn resolves_ai_settings_with_scoped_and_provider_fallbacks() {
        let mut settings = HashMap::new();
        settings.insert("ai_provider".to_string(), "anthropic".to_string());
        settings.insert(
            "ai_model.anthropic".to_string(),
            "claude-opus-4-6".to_string(),
        );
        settings.insert("ai_api_key.anthropic".to_string(), "sk-ant-test".to_string());

        let req = HeadlessChatRequest {
            prompt: "status".to_string(),
            provider: None,
            model: None,
            allow_actions: false,
        };
        let resolved = resolve_ai_config(&req, &settings);
        assert_eq!(resolved.provider, "anthropic");
        assert_eq!(resolved.model, "claude-opus-4-6");
        assert_eq!(resolved.api_key, "sk-ant-test");

        // Explicit CLI override switches provider and picks its sensible default model.
        let override_req = HeadlessChatRequest {
            prompt: "status".to_string(),
            provider: Some("gemini".to_string()),
            model: None,
            allow_actions: false,
        };
        let resolved_gemini = resolve_ai_config(&override_req, &HashMap::new());
        assert_eq!(resolved_gemini.provider, "gemini");
        assert_eq!(resolved_gemini.model, "gemini-2.5-flash");
    }
}
