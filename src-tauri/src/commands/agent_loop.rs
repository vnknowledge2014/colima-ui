use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicU64, Ordering};

use futures_util::FutureExt;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::commands::ai_chat::{AiChatHistoryMessage, AiChatRequest, A2A_DEFAULT_CONVERSATION_ID};
pub use crate::commands::ai_chat::ChatMessage;
use crate::commands::{
    activity_feed, alerts, colima, colima_config, compose, compose_autofix, compose_autofix_apply,
    compose_diagnose, containers, detonation, embedded_resources, falco_bridge, k8s_cluster,
    k8s_resources, kind, knowledge_bank, kubernetes, lima, models, networks, resolution,
    security_catalog, security_history, security_rules, security_scan, security_triage, self_heal,
    shell_sandbox, system, topology, volumes,
};

const MAX_AGENT_ROUNDS: usize = 5;
const MAX_HISTORY_MESSAGES: usize = 20; // Up to 10 clean user/assistant turn pairs

/// Canonical AI provider IDs supported across Desktop, Web, CLI, MCP, and A2A.
pub const CANONICAL_AI_PROVIDERS: &[&str] = &[
    "anthropic",
    "openai",
    "gemini",
    "openai-compatible",
    "ollama-local",
    "ollama-cloud",
];

pub fn is_canonical_ai_provider(provider: &str) -> bool {
    CANONICAL_AI_PROVIDERS.contains(&provider.trim())
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HeadlessChatRequest {
    pub prompt: String,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default, alias = "allow_mutations")]
    pub allow_actions: bool,
    #[serde(default, alias = "conversation_id", alias = "sessionId")]
    pub session_id: Option<String>,
    #[serde(default, alias = "mirrorUi")]
    pub mirror_ui: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolExecutionRecord {
    pub kind: String,
    pub name: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeadlessChatOutcome {
    pub reply: String,
    pub session_id: String,
    pub provider: String,
    pub model: String,
    pub tools_executed: Vec<ToolExecutionRecord>,
    pub navigated_to: Option<String>,
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

impl TagKind {
    pub fn as_str(self) -> &'static str {
        match self {
            TagKind::Query => "query",
            TagKind::Event => "event",
            TagKind::EventApprove => "event_approve",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ToolTag {
    pub kind: TagKind,
    pub event: String,
    pub payload: Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectiveKind {
    ReadReference,
    QueryState,
    Diagnose,
    Run,
    RunApprove,
    Learn,
    LearnReasoning,
    RememberPreference,
    Navigate,
}

impl DirectiveKind {
    pub fn as_str(self) -> &'static str {
        match self {
            DirectiveKind::ReadReference => "read_reference",
            DirectiveKind::QueryState => "query_state",
            DirectiveKind::Diagnose => "diagnose",
            DirectiveKind::Run => "run",
            DirectiveKind::RunApprove => "run_approve",
            DirectiveKind::Learn => "learn",
            DirectiveKind::LearnReasoning => "learn_reasoning",
            DirectiveKind::RememberPreference => "remember_preference",
            DirectiveKind::Navigate => "navigate",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectiveTag {
    pub kind: DirectiveKind,
    pub arg: String,
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

pub fn is_sensitive_setting_key(key: &str) -> bool {
    let lower = key.to_ascii_lowercase();
    lower.contains("api_key")
        || lower.contains("apikey")
        || lower.contains("token")
        || lower.contains("secret")
        || lower.contains("password")
}

/// Redact any setting key matching `api_key|token|secret|password` (case-insensitive)
/// to `"[REDACTED]"` so secrets never enter LLM context or tool outputs.
pub fn redact_ai_settings_map(settings: &HashMap<String, String>) -> Value {
    let mut out = BTreeMap::new();
    for (k, v) in settings {
        if is_sensitive_setting_key(k) {
            out.insert(k.clone(), Value::String("[REDACTED]".to_string()));
        } else {
            out.insert(k.clone(), Value::String(v.clone()));
        }
    }
    json!(out)
}

async fn load_ai_settings() -> HashMap<String, String> {
    let _ = crate::commands::ai_chat::ensure_knowledge_bank_initialized();

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

fn build_headless_system_prompt(app_context: &str, allow_actions: bool) -> String {
    let mutation_policy = if allow_actions {
        "Mutating actions ARE enabled for this session (`allow_actions=true`). Emit `[EVENT_APPROVE: <event> | <json>]` or `[RUN_APPROVE: <cmd>]` when the user asks to perform a state-changing operation."
    } else {
        "Mutating actions are DISABLED (`allow_actions=false`). Read-only `[QUERY: ...]`, `[READ_REFERENCE: ...]`, `[QUERY_STATE]`, `[DIAGNOSE]`, `[NAVIGATE: <page>]`, and safe `[RUN: <cmd>]` tags run automatically, whereas mutating `[EVENT_APPROVE: ...]` / `[RUN_APPROVE: ...]` tags will be blocked unless the caller enables `allow_actions` (`--yes` in CLI)."
    };

    format!(
        "{app_context}\n\n\
         ## Headless & A2A Execution Addendum\n\
         {mutation_policy}\n\n\
         ### Supported Tool & Directive Tags\n\
         - Read-only App Event query: `[QUERY: <event> | <json_object>]`\n\
         - State-changing App Event: `[EVENT_APPROVE: <event> | <json_object>]`\n\
         - Load bundled skill/reference doc: `[READ_REFERENCE: <relative_path>]`\n\
         - Snapshot live host/VM/container/compose/k8s state: `[QUERY_STATE]`\n\
         - Collect Colima diagnostic logs: `[DIAGNOSE]`\n\
         - Run sandboxed diagnostic CLI command: `[RUN: <cmd>]`\n\
         - Run state-changing CLI command (gated by `allow_actions`): `[RUN_APPROVE: <cmd>]`\n\
         - Navigate ColimaUI UI page: `[NAVIGATE: <page>]` or `[QUERY: navigate | {{\"page\": \"<page>\", \"section\": \"...\", \"toast\": \"...\"}}]`\n\
         - Persist learned fix / reasoning / preference: `[LEARN: <pattern> | <fix>]`, `[LEARN_REASONING: <insight>]`, `[REMEMBER_PREFERENCE: <pref>]`\n\n\
         ### Execution Rules\n\
         1. Emit valid tool tags when you need live state, reference docs, or requested mutations.\n\
         2. Once `[TOOL_RESULTS]` are provided, synthesize a clear, accurate final answer in the user's language without repeating unnecessary tool tags."
    )
}

/// Filter raw stored history into at most `max_messages` clean, strictly alternating
/// `user` / `assistant` messages starting with `user` and ending with `assistant`.
pub fn sanitize_history_turns(
    raw: &[AiChatHistoryMessage],
    max_messages: usize,
) -> Vec<ChatMessage> {
    let mut cleaned: Vec<ChatMessage> = Vec::new();

    for item in raw {
        let role = item.role.trim();
        if role != "user" && role != "assistant" {
            continue;
        }
        let text = strip_tool_tags(&item.content);
        let trimmed = text.trim();
        if trimmed.is_empty()
            || trimmed.starts_with("[TOOL_RESULTS]")
            || trimmed.starts_with("System Observation:")
        {
            continue;
        }

        if let Some(last) = cleaned.last_mut().filter(|m| m.role == role) {
            last.content.push_str("\n\n");
            last.content.push_str(trimmed);
            continue;
        }

        cleaned.push(ChatMessage {
            role: role.to_string(),
            content: trimmed.to_string(),
        });
    }

    // Ensure history starts with `"user"`.
    while cleaned.first().is_some_and(|m| m.role != "user") {
        cleaned.remove(0);
    }
    // Ensure history ends with `"assistant"` so appending the new `"user"` prompt
    // strictly alternates `user -> assistant -> ... -> assistant -> user`.
    while cleaned.last().is_some_and(|m| m.role != "assistant") {
        cleaned.pop();
    }

    if cleaned.len() > max_messages {
        let mut start = cleaned.len() - max_messages;
        if cleaned.get(start).is_some_and(|m| m.role != "user") {
            start += 1;
        }
        cleaned = cleaned.split_off(start);
        while cleaned.last().is_some_and(|m| m.role != "assistant") {
            cleaned.pop();
        }
    }

    cleaned
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

enum RawBracketTag {
    PipeTool(usize),
    DirectiveWithArg(DirectiveKind, usize),
    DirectiveBare(DirectiveKind, usize),
    PresentationOnly(usize),
}

fn match_any_bracket_tag(slice: &str) -> Option<RawBracketTag> {
    if let Some((_, len)) = match_tag_prefix(slice) {
        return Some(RawBracketTag::PipeTool(len));
    }

    const DIRECTIVE_PREFIXES: &[(&str, DirectiveKind)] = &[
        ("[READ_REFERENCE:", DirectiveKind::ReadReference),
        ("[RUN_APPROVE:", DirectiveKind::RunApprove),
        ("[RUN:", DirectiveKind::Run),
        ("[LEARN_REASONING:", DirectiveKind::LearnReasoning),
        ("[LEARN:", DirectiveKind::Learn),
        ("[REMEMBER_PREFERENCE:", DirectiveKind::RememberPreference),
        ("[NAVIGATE:", DirectiveKind::Navigate),
        ("[DIAGNOSE:", DirectiveKind::Diagnose),
    ];
    for &(prefix, kind) in DIRECTIVE_PREFIXES {
        if slice
            .get(..prefix.len())
            .is_some_and(|head| head.eq_ignore_ascii_case(prefix))
        {
            return Some(RawBracketTag::DirectiveWithArg(kind, prefix.len()));
        }
    }

    const BARE_DIRECTIVES: &[(&str, DirectiveKind)] = &[
        ("[QUERY_APP_STATE]", DirectiveKind::QueryState),
        ("[QUERY_STATE]", DirectiveKind::QueryState),
        ("[DIAGNOSE]", DirectiveKind::Diagnose),
    ];
    for &(exact, kind) in BARE_DIRECTIVES {
        if slice
            .get(..exact.len())
            .is_some_and(|head| head.eq_ignore_ascii_case(exact))
        {
            return Some(RawBracketTag::DirectiveBare(kind, exact.len()));
        }
    }

    for ui_prefix in ["[SUGGEST:", "[ASK_CHOICE:"] {
        if slice
            .get(..ui_prefix.len())
            .is_some_and(|head| head.eq_ignore_ascii_case(ui_prefix))
        {
            return Some(RawBracketTag::PresentationOnly(ui_prefix.len()));
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
                    .unwrap_or_else(|_| json!({}));
                (ev.trim(), parsed)
            }
            None => (inner, json!({})),
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

/// Extract non-JSON-pipe directive tags (`[READ_REFERENCE: ...]`, `[QUERY_STATE]`,
/// `[DIAGNOSE]`, `[RUN: ...]`, `[RUN_APPROVE: ...]`, `[LEARN: ...]`,
/// `[LEARN_REASONING: ...]`, `[REMEMBER_PREFERENCE: ...]`, `[NAVIGATE: ...]`).
pub fn extract_directive_tags(text: &str) -> Vec<DirectiveTag> {
    let mut out = Vec::new();
    let mut cursor = 0usize;

    while let Some(rel_open) = text[cursor..].find('[') {
        let tag_start = cursor + rel_open;
        match match_any_bracket_tag(&text[tag_start..]) {
            Some(RawBracketTag::DirectiveBare(kind, exact_len)) => {
                out.push(DirectiveTag {
                    kind,
                    arg: String::new(),
                });
                cursor = tag_start + exact_len;
            }
            Some(RawBracketTag::DirectiveWithArg(kind, prefix_len)) => {
                let inner_start = tag_start + prefix_len;
                let Some(rel_close) = find_matching_closing_bracket(&text[inner_start..]) else {
                    break;
                };
                let arg = text[inner_start..inner_start + rel_close].trim().to_string();
                if !arg.is_empty() || matches!(kind, DirectiveKind::Diagnose) {
                    out.push(DirectiveTag { kind, arg });
                }
                cursor = inner_start + rel_close + 1;
            }
            Some(RawBracketTag::PipeTool(prefix_len) | RawBracketTag::PresentationOnly(prefix_len)) => {
                let inner_start = tag_start + prefix_len;
                if let Some(rel_close) = find_matching_closing_bracket(&text[inner_start..]) {
                    cursor = inner_start + rel_close + 1;
                } else {
                    break;
                }
            }
            None => {
                cursor = tag_start + 1;
            }
        }
    }

    out
}

/// Strip all recognized tool and directive tags from text, returning clean prose.
pub fn strip_tool_tags(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut cursor = 0usize;

    while let Some(rel_open) = text[cursor..].find('[') {
        let tag_start = cursor + rel_open;
        match match_any_bracket_tag(&text[tag_start..]) {
            Some(RawBracketTag::DirectiveBare(_, exact_len)) => {
                out.push_str(&text[cursor..tag_start]);
                cursor = tag_start + exact_len;
            }
            Some(
                RawBracketTag::PipeTool(prefix_len)
                | RawBracketTag::DirectiveWithArg(_, prefix_len)
                | RawBracketTag::PresentationOnly(prefix_len),
            ) => {
                out.push_str(&text[cursor..tag_start]);
                let inner_start = tag_start + prefix_len;
                if let Some(rel_close) = find_matching_closing_bracket(&text[inner_start..]) {
                    cursor = inner_start + rel_close + 1;
                } else {
                    cursor = text.len();
                    break;
                }
            }
            None => {
                out.push_str(&text[cursor..=tag_start]);
                cursor = tag_start + 1;
            }
        }
    }

    if cursor < text.len() {
        out.push_str(&text[cursor..]);
    }

    out.trim().to_string()
}

pub fn classify_event(event: &str) -> EventSafety {
    match event {
        // Existing 23 SAFE queries
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
        | "model-list"
        // Expanded SAFE queries (ColimaUI v0.3.0 full parity)
        | "container-top"
        | "volume-inspect"
        | "network-inspect"
        | "compose-ps"
        | "compose-diagnose"
        | "compose-autofix-history"
        | "k8s-list-contexts"
        | "k8s-current-context"
        | "k8s-list-namespaces"
        | "k8s-list-resources"
        | "k8s-describe"
        | "k8s-yaml"
        | "k8s-logs"
        | "k8s-container-logs"
        | "k8s-pod-containers"
        | "k8s-events"
        | "k8s-crds"
        | "k8s-crd-resources"
        | "k8s-port-forward-list"
        | "kind-list"
        | "lima-info"
        | "lima-templates"
        | "system-check-tool"
        | "ai-config-status"
        | "list-presets"
        | "colima-config-get"
        | "topology-get"
        | "security-history"
        | "security-rules"
        | "security-alternatives"
        | "falco-state"
        | "falco-events"
        | "detonation-list"
        | "alerts-rules"
        | "alerts-events"
        | "self-heal-rules"
        | "self-heal-log"
        | "activity-feed"
        | "navigate" => EventSafety::Safe,

        // Existing 15 MUTATING events
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
        | "lima-stop"
        // Expanded MUTATING events (gated by `allow_actions`)
        | "create-instance"
        | "delete-instance"
        | "colima-config-apply"
        | "run-container"
        | "rename-container"
        | "container-exec"
        | "remove-image"
        | "prune-images"
        | "tag-image"
        | "volume-create"
        | "volume-remove"
        | "volume-prune"
        | "network-create"
        | "network-remove"
        | "network-prune"
        | "system-prune"
        | "compose-stop"
        | "compose-start"
        | "compose-autofix-propose"
        | "compose-autofix-apply"
        | "compose-autofix-undo"
        | "k8s-set-context"
        | "k8s-apply"
        | "k8s-delete"
        | "k8s-restart-resource"
        | "k8s-scale"
        | "k8s-generic-scale"
        | "k8s-node-action"
        | "k8s-port-forward-start"
        | "k8s-port-forward-stop"
        | "k8s-exec"
        | "kind-create"
        | "kind-delete"
        | "lima-create"
        | "lima-delete"
        | "lima-shell"
        | "model-pull"
        | "model-serve"
        | "model-delete"
        | "security-scan"
        | "security-sbom"
        | "security-audit"
        | "security-triage"
        | "detonation-start"
        | "detonation-cancel"
        | "ai-update-config"
        | "save-preset"
        | "delete-preset"
        | "cli-exec" => EventSafety::Mutating,

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

fn i64_opt_field(payload: &Value, keys: &[&str]) -> Option<i64> {
    for key in keys {
        if let Some(n) = payload.get(*key).and_then(Value::as_i64) {
            return Some(n);
        }
    }
    None
}

fn string_vec_field(payload: &Value, keys: &[&str]) -> Vec<String> {
    for key in keys {
        if let Some(arr) = payload.get(*key).and_then(Value::as_array) {
            return arr
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect();
        }
    }
    Vec::new()
}

fn parse_security_level(payload: &Value) -> Option<security_rules::Level> {
    let raw = str_field(payload, &["level", "strictness"])?;
    match raw.to_ascii_lowercase().as_str() {
        "l1" | "1" => Some(security_rules::Level::L1),
        "l2" | "2" => Some(security_rules::Level::L2),
        "l3" | "3" => Some(security_rules::Level::L3),
        _ => None,
    }
}

fn parse_sbom_format(payload: &Value) -> security_scan::SbomFormat {
    match str_field(payload, &["format"])
        .unwrap_or("cyclone_dx")
        .to_ascii_lowercase()
        .as_str()
    {
        "spdx" | "spdx-json" | "spdx_json" => security_scan::SbomFormat::Spdx,
        _ => security_scan::SbomFormat::CycloneDx,
    }
}

fn default_scan_id(payload: &Value) -> String {
    if let Some(id) = str_field(payload, &["scanId", "scan_id"]) {
        return id.to_string();
    }
    static SCAN_SEQ: AtomicU64 = AtomicU64::new(1);
    let seq = SCAN_SEQ.fetch_add(1, Ordering::Relaxed);
    format!("agent-scan-{}-{seq}", chrono::Utc::now().timestamp_millis())
}

fn format_exec_result(res: &shell_sandbox::ExecResult) -> String {
    let mut out = String::new();
    if !res.stdout.trim().is_empty() {
        out.push_str(res.stdout.trim_end());
    }
    if !res.stderr.trim().is_empty() {
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str("stderr:\n");
        out.push_str(res.stderr.trim_end());
    }
    if out.is_empty() {
        format!("(exit code {})", res.exit_code)
    } else {
        format!("{out}\n(exit code {})", res.exit_code)
    }
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
         Mutations require `--yes` in CLI mode (e.g. `cui --yes \"<prompt>\"`) or `allowActions: true` in A2A/API requests. \
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

async fn collect_live_state_snapshot() -> String {
    let host = tokio::task::spawn_blocking(system::detect_host_specs)
        .await
        .ok();
    let instances = crate::instance_reader::list_instances_fast();
    let containers_list = containers::list_containers_cli(true)
        .await
        .unwrap_or_default();
    let compose_list = compose::list_compose_projects()
        .await
        .unwrap_or_default();
    let k8s_ctx = k8s_cluster::k8s_current_context()
        .await
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    serde_json::to_string_pretty(&json!({
        "host_specs": host,
        "colima_instances": instances,
        "containers": containers_list,
        "compose_projects": compose_list,
        "kubernetes_current_context": k8s_ctx,
    }))
    .unwrap_or_else(|_| "{}".to_string())
}

/// Execute a non-JSON-pipe directive tag (`[READ_REFERENCE: ...]`, `[QUERY_STATE]`,
/// `[DIAGNOSE]`, `[RUN: ...]`, `[RUN_APPROVE: ...]`, `[LEARN: ...]`,
/// `[LEARN_REASONING: ...]`, `[REMEMBER_PREFERENCE: ...]`, `[NAVIGATE: ...]`).
///
/// Returns `Ok(Some(observation))` when an observation block should be fed back into
/// the next LLM round, or `Ok(None)` for passive persistence/UI tags (`[LEARN*]`,
/// `[REMEMBER_PREFERENCE]`, `[NAVIGATE]`).
pub async fn dispatch_directive_tag(
    tag: &DirectiveTag,
    allow_actions: bool,
    navigated_to: &mut Option<String>,
) -> Result<Option<String>, String> {
    match tag.kind {
        DirectiveKind::ReadReference => {
            let content = embedded_resources::read_reference_any(&tag.arg)?;
            Ok(Some(content))
        }
        DirectiveKind::QueryState => Ok(Some(collect_live_state_snapshot().await)),
        DirectiveKind::Diagnose => {
            let profile = if tag.arg.trim().is_empty() {
                "default"
            } else {
                tag.arg.trim()
            };
            let logs = colima::collect_diagnostic_logs(profile.to_string())
                .await
                .map_err(|e| e.to_string())?;
            Ok(Some(logs))
        }
        DirectiveKind::Run => {
            let cmd = tag.arg.trim();
            if cmd.is_empty() {
                return Err("Empty command in [RUN]".to_string());
            }
            match shell_sandbox::sandbox_execute(cmd.to_string()).await {
                Ok(res) => Ok(Some(format_exec_result(&res))),
                Err(e) => {
                    let msg = e.to_string();
                    if msg.contains("needs_approval:") {
                        if !allow_actions {
                            return Ok(Some(format_mutation_blocked_note(
                                "run",
                                &json!({ "command": cmd }),
                            )));
                        }
                        let res = shell_sandbox::sandbox_execute_approved(cmd.to_string())
                            .await
                            .map_err(|err| err.to_string())?;
                        Ok(Some(format_exec_result(&res)))
                    } else {
                        Err(msg)
                    }
                }
            }
        }
        DirectiveKind::RunApprove => {
            let cmd = tag.arg.trim();
            if cmd.is_empty() {
                return Err("Empty command in [RUN_APPROVE]".to_string());
            }
            if !allow_actions {
                return Ok(Some(format_mutation_blocked_note(
                    "run_approve",
                    &json!({ "command": cmd }),
                )));
            }
            let res = shell_sandbox::sandbox_execute_approved(cmd.to_string())
                .await
                .map_err(|e| e.to_string())?;
            Ok(Some(format_exec_result(&res)))
        }
        DirectiveKind::Learn => {
            let _ = crate::commands::ai_chat::ensure_knowledge_bank_initialized();
            let (pattern, solution) = match tag.arg.split_once('|') {
                Some((p, s)) => (p.trim().to_string(), s.trim().to_string()),
                None => ("general".to_string(), tag.arg.trim().to_string()),
            };
            if !solution.is_empty() {
                let _ = knowledge_bank::kb_learn(pattern, solution).await;
            }
            Ok(None)
        }
        DirectiveKind::LearnReasoning => {
            let _ = crate::commands::ai_chat::ensure_knowledge_bank_initialized();
            if !tag.arg.trim().is_empty() {
                let _ = knowledge_bank::add_memory(
                    "reasoning".to_string(),
                    tag.arg.trim().to_string(),
                )
                .await;
            }
            Ok(None)
        }
        DirectiveKind::RememberPreference => {
            let _ = crate::commands::ai_chat::ensure_knowledge_bank_initialized();
            if !tag.arg.trim().is_empty() {
                let _ = knowledge_bank::add_memory(
                    "preference".to_string(),
                    tag.arg.trim().to_string(),
                )
                .await;
            }
            Ok(None)
        }
        DirectiveKind::Navigate => {
            let (page_part, section_part) = match tag.arg.split_once('|') {
                Some((p, s)) => (p.trim(), Some(s.trim())),
                None => (tag.arg.trim(), None),
            };
            let emitted = crate::routes::ui::emit_ui_navigate(page_part, section_part, None)?;
            if let Some(p) = emitted.get("page").and_then(Value::as_str) {
                *navigated_to = Some(p.to_string());
            }
            Ok(None)
        }
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
        "container-top" => {
            let id = str_field(payload, &["id", "container_id", "containerId"])
                .ok_or_else(|| "Missing 'id' for container-top".to_string())?;
            containers::container_top(id.to_string())
                .await
                .map_err(|e| e.to_string())
        }
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
        "volume-inspect" => {
            let name = str_field(payload, &["name", "volume", "id"])
                .ok_or_else(|| "Missing 'name' for volume-inspect".to_string())?;
            volumes::inspect_volume(name.to_string())
                .await
                .map_err(|e| e.to_string())
        }
        "network-list" => {
            let nets = networks::list_networks().await.map_err(|e| e.to_string())?;
            serde_json::to_string_pretty(&nets).map_err(|e| e.to_string())
        }
        "network-inspect" => {
            let name = str_field(payload, &["name", "network", "id"])
                .ok_or_else(|| "Missing 'name' for network-inspect".to_string())?;
            networks::inspect_network(name.to_string())
                .await
                .map_err(|e| e.to_string())
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
        "compose-ps" => {
            let project = str_field(payload, &["projectName", "project_name", "dir", "name"])
                .ok_or_else(|| "Missing 'projectName' for compose-ps".to_string())?;
            compose::compose_ps(project.to_string())
                .await
                .map_err(|e| e.to_string())
        }
        "compose-diagnose" => {
            let file_path = str_field(payload, &["filePath", "file_path", "path", "file"])
                .ok_or_else(|| "Missing 'filePath' for compose-diagnose".to_string())?;
            let diag = compose_diagnose::compose_diagnose(file_path.to_string())
                .await
                .map_err(|e| e.to_string())?;
            serde_json::to_string_pretty(&diag).map_err(|e| e.to_string())
        }
        "compose-autofix-history" => {
            let history = compose_autofix_apply::compose_autofix_history()
                .await
                .map_err(|e| e.to_string())?;
            serde_json::to_string_pretty(&history).map_err(|e| e.to_string())
        }
        "system-df" => containers::system_df().await.map_err(|e| e.to_string()),
        "system-check" => {
            let info = system::check_system().await.map_err(|e| e.to_string())?;
            serde_json::to_string_pretty(&info).map_err(|e| e.to_string())
        }
        "system-check-tool" => {
            let name = str_field(payload, &["name", "tool"])
                .ok_or_else(|| "Missing 'name' for system-check-tool".to_string())?;
            let status = system::check_tool(name.to_string())
                .await
                .map_err(|e| e.to_string())?;
            serde_json::to_string_pretty(&status).map_err(|e| e.to_string())
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
        "k8s-list-contexts" => k8s_cluster::k8s_contexts().await.map_err(|e| e.to_string()),
        "k8s-current-context" => k8s_cluster::k8s_current_context()
            .await
            .map_err(|e| e.to_string()),
        "k8s-list-namespaces" => {
            let list = kubernetes::k8s_namespaces()
                .await
                .map_err(|e| e.to_string())?;
            serde_json::to_string_pretty(&list).map_err(|e| e.to_string())
        }
        "k8s-list-resources" => {
            let resource = str_field(payload, &["resource", "resourceType", "resource_type", "kind"])
                .ok_or_else(|| "Missing 'resource' for k8s-list-resources".to_string())?;
            let ns = str_field(payload, &["namespace", "ns"]).unwrap_or("all");
            kubernetes::k8s_resources(resource.to_string(), ns.to_string())
                .await
                .map_err(|e| e.to_string())
        }
        "k8s-describe" => {
            let ns = str_field(payload, &["namespace", "ns"]).unwrap_or("default");
            let resource_type =
                str_field(payload, &["resourceType", "resource_type", "resource", "kind"])
                    .ok_or_else(|| "Missing 'resourceType' for k8s-describe".to_string())?;
            let name = str_field(payload, &["name", "pod"])
                .ok_or_else(|| "Missing 'name' for k8s-describe".to_string())?;
            kubernetes::k8s_describe(ns.to_string(), resource_type.to_string(), name.to_string())
                .await
                .map_err(|e| e.to_string())
        }
        "k8s-yaml" => {
            let ns = str_field(payload, &["namespace", "ns"]).unwrap_or("default");
            let resource_type =
                str_field(payload, &["resourceType", "resource_type", "resource", "kind"])
                    .ok_or_else(|| "Missing 'resourceType' for k8s-yaml".to_string())?;
            let name = str_field(payload, &["name"])
                .ok_or_else(|| "Missing 'name' for k8s-yaml".to_string())?;
            k8s_resources::k8s_yaml(resource_type.to_string(), ns.to_string(), name.to_string())
                .await
                .map_err(|e| e.to_string())
        }
        "k8s-logs" => {
            let ns = str_field(payload, &["namespace", "ns"]).unwrap_or("default");
            let pod = str_field(payload, &["pod", "name"])
                .ok_or_else(|| "Missing 'pod' for k8s-logs".to_string())?;
            let lines = u32_field(payload, &["lines", "tail"], 100);
            kubernetes::k8s_pod_logs(ns.to_string(), pod.to_string(), lines)
                .await
                .map_err(|e| e.to_string())
        }
        "k8s-container-logs" => {
            let ns = str_field(payload, &["namespace", "ns"]).unwrap_or("default");
            let pod = str_field(payload, &["pod", "name"])
                .ok_or_else(|| "Missing 'pod' for k8s-container-logs".to_string())?;
            let container = str_field(payload, &["container"]).unwrap_or("");
            let lines = u32_field(payload, &["lines", "tail"], 100);
            let previous = payload
                .get("previous")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            k8s_resources::k8s_container_logs(
                ns.to_string(),
                pod.to_string(),
                container.to_string(),
                lines,
                previous,
            )
            .await
            .map_err(|e| e.to_string())
        }
        "k8s-pod-containers" => {
            let ns = str_field(payload, &["namespace", "ns"]).unwrap_or("default");
            let pod = str_field(payload, &["pod", "name"])
                .ok_or_else(|| "Missing 'pod' for k8s-pod-containers".to_string())?;
            k8s_resources::k8s_pod_containers(ns.to_string(), pod.to_string())
                .await
                .map_err(|e| e.to_string())
        }
        "k8s-events" => {
            let ns = str_field(payload, &["namespace", "ns"]).unwrap_or("all");
            kubernetes::k8s_events(ns.to_string())
                .await
                .map_err(|e| e.to_string())
        }
        "k8s-crds" => k8s_resources::k8s_crds().await.map_err(|e| e.to_string()),
        "k8s-crd-resources" => {
            let resource = str_field(payload, &["resource", "crd", "name"])
                .ok_or_else(|| "Missing 'resource' for k8s-crd-resources".to_string())?;
            let ns = str_field(payload, &["namespace", "ns"]).unwrap_or("all");
            k8s_resources::k8s_crd_resources(resource.to_string(), ns.to_string())
                .await
                .map_err(|e| e.to_string())
        }
        "k8s-port-forward-list" => k8s_cluster::k8s_pf_list().await.map_err(|e| e.to_string()),
        "kind-list" => kind::kind_list().await.map_err(|e| e.to_string()),
        "lima-list" => {
            let vms = lima::lima_list().await.map_err(|e| e.to_string())?;
            serde_json::to_string_pretty(&vms).map_err(|e| e.to_string())
        }
        "lima-info" => {
            let name = str_field(payload, &["name", "profile"])
                .ok_or_else(|| "Missing 'name' for lima-info".to_string())?;
            lima::lima_info(name.to_string())
                .await
                .map_err(|e| e.to_string())
        }
        "lima-templates" => lima::lima_templates().await.map_err(|e| e.to_string()),
        "model-list" => {
            let profile = str_field(payload, &["profile"]).unwrap_or("default");
            let runner = str_field(payload, &["runner"]).map(str::to_string);
            let list = models::list_models(profile.to_string(), runner)
                .await
                .map_err(|e| e.to_string())?;
            serde_json::to_string_pretty(&list).map_err(|e| e.to_string())
        }
        "ai-config-status" => {
            let settings = load_ai_settings().await;
            let redacted = redact_ai_settings_map(&settings);
            serde_json::to_string_pretty(&redacted).map_err(|e| e.to_string())
        }
        "list-presets" => {
            let _ = crate::commands::ai_chat::ensure_knowledge_bank_initialized();
            let presets = knowledge_bank::get_all_presets().await?;
            serde_json::to_string_pretty(&presets).map_err(|e| e.to_string())
        }
        "colima-config-get" => {
            let profile = str_field(payload, &["profile", "name"]).unwrap_or("default");
            let snap = colima_config::get_colima_config(profile.to_string())
                .await
                .map_err(|e| e.to_string())?;
            serde_json::to_string_pretty(&snap).map_err(|e| e.to_string())
        }
        "topology-get" => {
            let graph = topology::get_topology().await.map_err(|e| e.to_string())?;
            serde_json::to_string_pretty(&graph).map_err(|e| e.to_string())
        }
        "security-history" => {
            if let Some(digest) = str_field(payload, &["imageDigest", "image_digest", "digest"]) {
                let runs = security_history::security_score_history(digest.to_string())
                    .await
                    .map_err(|e| e.to_string())?;
                serde_json::to_string_pretty(&runs).map_err(|e| e.to_string())
            } else {
                let limit = i64_opt_field(payload, &["limit"]).unwrap_or(50);
                let runs = security_history::recent(limit)?;
                serde_json::to_string_pretty(&runs).map_err(|e| e.to_string())
            }
        }
        "security-rules" => {
            let pack = security_rules::security_rule_pack()
                .await
                .map_err(|e| e.to_string())?;
            serde_json::to_string_pretty(&pack).map_err(|e| e.to_string())
        }
        "security-alternatives" => {
            let image_ref = str_field(payload, &["imageRef", "image_ref", "image", "name"])
                .ok_or_else(|| "Missing 'imageRef' for security-alternatives".to_string())?;
            let suggestions = security_catalog::security_alternatives(image_ref.to_string())
                .await
                .map_err(|e| e.to_string())?;
            serde_json::to_string_pretty(&suggestions).map_err(|e| e.to_string())
        }
        "falco-state" => {
            let st = falco_bridge::falco_state()
                .await
                .map_err(|e| e.to_string())?;
            serde_json::to_string_pretty(&st).map_err(|e| e.to_string())
        }
        "falco-events" => {
            let limit = i64_opt_field(payload, &["limit"]);
            let events = falco_bridge::falco_events(limit)
                .await
                .map_err(|e| e.to_string())?;
            serde_json::to_string_pretty(&events).map_err(|e| e.to_string())
        }
        "detonation-list" => {
            let list = detonation::detonation_list()
                .await
                .map_err(|e| e.to_string())?;
            serde_json::to_string_pretty(&list).map_err(|e| e.to_string())
        }
        "alerts-rules" => {
            let rules = alerts::alerts_list_rules()
                .await
                .map_err(|e| e.to_string())?;
            serde_json::to_string_pretty(&rules).map_err(|e| e.to_string())
        }
        "alerts-events" => {
            let limit = i64_opt_field(payload, &["limit"]);
            let events = alerts::alerts_recent_events(limit)
                .await
                .map_err(|e| e.to_string())?;
            serde_json::to_string_pretty(&events).map_err(|e| e.to_string())
        }
        "self-heal-rules" => {
            let rules = self_heal::self_heal_list_rules()
                .await
                .map_err(|e| e.to_string())?;
            serde_json::to_string_pretty(&rules).map_err(|e| e.to_string())
        }
        "self-heal-log" => {
            let limit = i64_opt_field(payload, &["limit"]);
            let log = self_heal::self_heal_recent_log(limit)
                .await
                .map_err(|e| e.to_string())?;
            serde_json::to_string_pretty(&log).map_err(|e| e.to_string())
        }
        "activity-feed" => {
            let kind = str_field(payload, &["kind"]).map(str::to_string);
            let target = str_field(payload, &["target"]).map(str::to_string);
            let from_ms = i64_opt_field(payload, &["fromMs", "from_ms"]);
            let to_ms = i64_opt_field(payload, &["toMs", "to_ms"]);
            let limit = i64_opt_field(payload, &["limit"]);
            let feed = activity_feed::activity_feed(kind, target, from_ms, to_ms, limit)
                .await
                .map_err(|e| e.to_string())?;
            serde_json::to_string_pretty(&feed).map_err(|e| e.to_string())
        }
        "navigate" => {
            let page = str_field(payload, &["page", "target", "view"])
                .ok_or_else(|| "Missing 'page' for navigate".to_string())?;
            let section = str_field(payload, &["section"]);
            let toast = str_field(payload, &["toast", "message"]);
            let emitted = crate::routes::ui::emit_ui_navigate(page, section, toast)?;
            serde_json::to_string_pretty(&emitted).map_err(|e| e.to_string())
        }
        _ => Err(format!("Unknown safe query: {event}")),
    }
}

async fn execute_mutating_event(event: &str, payload: &Value) -> Result<String, String> {
    match event {
        "start-instance" | "create-instance" => start_colima_preserving_yaml(payload).await,
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
        "delete-instance" => {
            let profile = str_field(payload, &["profile", "name"])
                .ok_or_else(|| "Missing 'profile' for delete-instance".to_string())?;
            let force = payload
                .get("force")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            colima::delete_instance_cli(profile.to_string(), force)
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
        "colima-config-apply" => {
            let profile = str_field(payload, &["profile", "name"]).unwrap_or("default");
            let changes_val = payload.get("changes").unwrap_or(payload);
            let changes: colima_config::ConfigChanges =
                serde_json::from_value(changes_val.clone()).map_err(|e| e.to_string())?;
            let expected_mtime = match i64_opt_field(payload, &["expectedMtime", "expected_mtime", "mtime"]) {
                Some(m) => m,
                None => {
                    colima_config::get_colima_config(profile.to_string())
                        .await
                        .map_err(|e| e.to_string())?
                        .mtime
                }
            };
            let res =
                colima_config::apply_colima_config(profile.to_string(), changes, expected_mtime)
                    .await
                    .map_err(|e| e.to_string())?;
            serde_json::to_string_pretty(&res).map_err(|e| e.to_string())
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
        "run-container" => {
            let image = str_field(payload, &["image"])
                .ok_or_else(|| "Missing 'image' for run-container".to_string())?;
            let name = str_field(payload, &["name"]).unwrap_or("");
            let ports = string_vec_field(payload, &["ports"]);
            let env_vars = string_vec_field(payload, &["envVars", "env_vars", "env"]);
            let vols = string_vec_field(payload, &["volumes"]);
            let detach = payload
                .get("detach")
                .and_then(Value::as_bool)
                .unwrap_or(true);
            let remove_on_exit = payload
                .get("removeOnExit")
                .or_else(|| payload.get("remove_on_exit"))
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let extra_args = string_vec_field(payload, &["extraArgs", "extra_args"]);
            containers::run_container(
                image.to_string(),
                name.to_string(),
                ports,
                env_vars,
                vols,
                detach,
                remove_on_exit,
                extra_args,
            )
            .await
            .map_err(|e| e.to_string())
        }
        "rename-container" => {
            let id = str_field(payload, &["id", "container_id", "containerId"])
                .ok_or_else(|| "Missing 'id' for rename-container".to_string())?;
            let new_name = str_field(payload, &["newName", "new_name", "name"])
                .ok_or_else(|| "Missing 'newName' for rename-container".to_string())?;
            containers::rename_container(id.to_string(), new_name.to_string())
                .await
                .map_err(|e| e.to_string())
        }
        "container-exec" => {
            let id = str_field(payload, &["id", "container_id", "containerId"])
                .ok_or_else(|| "Missing 'id' for container-exec".to_string())?;
            let cmd = str_field(payload, &["command", "cmd"])
                .ok_or_else(|| "Missing 'command' for container-exec".to_string())?;
            containers::container_exec(id.to_string(), cmd.to_string())
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
        "remove-image" => {
            let id = str_field(payload, &["id", "image", "image_id", "imageId"])
                .ok_or_else(|| "Missing 'id' for remove-image".to_string())?;
            let force = payload
                .get("force")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            containers::remove_image(id.to_string(), force)
                .await
                .map_err(|e| e.to_string())
        }
        "prune-images" => containers::prune_images().await.map_err(|e| e.to_string()),
        "tag-image" => {
            let source = str_field(payload, &["source", "image"])
                .ok_or_else(|| "Missing 'source' for tag-image".to_string())?;
            let target = str_field(payload, &["target", "tag"])
                .ok_or_else(|| "Missing 'target' for tag-image".to_string())?;
            containers::tag_image(source.to_string(), target.to_string())
                .await
                .map_err(|e| e.to_string())
        }
        "volume-create" => {
            let name = str_field(payload, &["name"])
                .ok_or_else(|| "Missing 'name' for volume-create".to_string())?;
            let driver = str_field(payload, &["driver"]).unwrap_or("local");
            volumes::create_volume(name.to_string(), driver.to_string())
                .await
                .map_err(|e| e.to_string())
        }
        "volume-remove" => {
            let name = str_field(payload, &["name", "id"])
                .ok_or_else(|| "Missing 'name' for volume-remove".to_string())?;
            let force = payload
                .get("force")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            volumes::remove_volume(name.to_string(), force)
                .await
                .map_err(|e| e.to_string())
        }
        "volume-prune" => volumes::prune_volumes().await.map_err(|e| e.to_string()),
        "network-create" => {
            let name = str_field(payload, &["name"])
                .ok_or_else(|| "Missing 'name' for network-create".to_string())?;
            let driver = str_field(payload, &["driver"]).unwrap_or("bridge");
            let subnet = str_field(payload, &["subnet"]).unwrap_or("");
            networks::create_network(name.to_string(), driver.to_string(), subnet.to_string())
                .await
                .map_err(|e| e.to_string())
        }
        "network-remove" => {
            let name = str_field(payload, &["name", "id"])
                .ok_or_else(|| "Missing 'name' for network-remove".to_string())?;
            networks::remove_network(name.to_string())
                .await
                .map_err(|e| e.to_string())
        }
        "network-prune" => networks::prune_networks().await.map_err(|e| e.to_string()),
        "system-prune" => {
            let all = payload
                .get("all")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            containers::system_prune(all).await.map_err(|e| e.to_string())
        }
        "compose-up" => {
            let dir = str_field(payload, &["project_dir", "projectDir", "dir", "file"]).unwrap_or("");
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
        "compose-stop" => {
            let project = str_field(payload, &["project_name", "projectName", "name", "dir"])
                .ok_or_else(|| "Missing 'projectName' for compose-stop".to_string())?;
            compose::compose_stop(project.to_string())
                .await
                .map_err(|e| e.to_string())
        }
        "compose-start" => {
            let project = str_field(payload, &["project_name", "projectName", "name", "dir"])
                .ok_or_else(|| "Missing 'projectName' for compose-start".to_string())?;
            compose::compose_start(project.to_string())
                .await
                .map_err(|e| e.to_string())
        }
        "compose-autofix-propose" => {
            let file_path = str_field(payload, &["filePath", "file_path", "path", "file"])
                .ok_or_else(|| "Missing 'filePath' for compose-autofix-propose".to_string())?;
            let proposal = compose_autofix::compose_autofix_propose(file_path.to_string())
                .await
                .map_err(|e| e.to_string())?;
            serde_json::to_string_pretty(&proposal).map_err(|e| e.to_string())
        }
        "compose-autofix-apply" => {
            let file_path = str_field(payload, &["filePath", "file_path", "path", "file"])
                .ok_or_else(|| "Missing 'filePath' for compose-autofix-apply".to_string())?;
            let new_content = str_field(payload, &["newContent", "new_content", "content"])
                .ok_or_else(|| "Missing 'newContent' for compose-autofix-apply".to_string())?;
            let explanation = str_field(payload, &["explanation"]).unwrap_or("Applied by agent");
            let declared_removals =
                string_vec_field(payload, &["declaredRemovals", "declared_removals"]);
            let rec = compose_autofix_apply::compose_autofix_apply(
                file_path.to_string(),
                new_content.to_string(),
                explanation.to_string(),
                declared_removals,
            )
            .await
            .map_err(|e| e.to_string())?;
            serde_json::to_string_pretty(&rec).map_err(|e| e.to_string())
        }
        "compose-autofix-undo" => {
            let fix_id = str_field(payload, &["fixId", "fix_id", "id"])
                .ok_or_else(|| "Missing 'fixId' for compose-autofix-undo".to_string())?;
            let rec = compose_autofix_apply::compose_autofix_undo(fix_id.to_string())
                .await
                .map_err(|e| e.to_string())?;
            serde_json::to_string_pretty(&rec).map_err(|e| e.to_string())
        }
        "k8s-set-context" => {
            let ctx = str_field(payload, &["context", "name"])
                .ok_or_else(|| "Missing 'context' for k8s-set-context".to_string())?;
            k8s_cluster::k8s_set_context(ctx.to_string())
                .await
                .map_err(|e| e.to_string())
        }
        "k8s-apply" => {
            let yaml = str_field(payload, &["yaml", "manifest", "content"])
                .ok_or_else(|| "Missing 'yaml' for k8s-apply".to_string())?;
            let ns = str_field(payload, &["namespace", "ns"]).unwrap_or("");
            k8s_resources::k8s_apply(yaml.to_string(), ns.to_string())
                .await
                .map_err(|e| e.to_string())
        }
        "k8s-delete" => {
            let resource_type =
                str_field(payload, &["resourceType", "resource_type", "resource", "kind"])
                    .ok_or_else(|| "Missing 'resourceType' for k8s-delete".to_string())?;
            let ns = str_field(payload, &["namespace", "ns"]).unwrap_or("default");
            let name = str_field(payload, &["name"])
                .ok_or_else(|| "Missing 'name' for k8s-delete".to_string())?;
            k8s_resources::k8s_delete_resource(
                resource_type.to_string(),
                ns.to_string(),
                name.to_string(),
            )
            .await
            .map_err(|e| e.to_string())
        }
        "k8s-restart-resource" => {
            let resource_type =
                str_field(payload, &["resourceType", "resource_type", "resource", "kind"])
                    .unwrap_or("deployment");
            let ns = str_field(payload, &["namespace", "ns"]).unwrap_or("default");
            let name = str_field(payload, &["name"])
                .ok_or_else(|| "Missing 'name' for k8s-restart-resource".to_string())?;
            k8s_resources::k8s_restart(resource_type.to_string(), ns.to_string(), name.to_string())
                .await
                .map_err(|e| e.to_string())
        }
        "k8s-scale" => {
            let ns = str_field(payload, &["namespace", "ns"]).unwrap_or("default");
            let deployment = str_field(payload, &["deployment", "name"])
                .ok_or_else(|| "Missing 'deployment' for k8s-scale".to_string())?;
            let replicas = u32_field(payload, &["replicas"], 1);
            kubernetes::k8s_scale(ns.to_string(), deployment.to_string(), replicas)
                .await
                .map_err(|e| e.to_string())
        }
        "k8s-generic-scale" => {
            let resource_type =
                str_field(payload, &["resourceType", "resource_type", "resource", "kind"])
                    .ok_or_else(|| "Missing 'resourceType' for k8s-generic-scale".to_string())?;
            let ns = str_field(payload, &["namespace", "ns"]).unwrap_or("default");
            let name = str_field(payload, &["name"])
                .ok_or_else(|| "Missing 'name' for k8s-generic-scale".to_string())?;
            let replicas = u32_field(payload, &["replicas"], 1);
            k8s_resources::k8s_generic_scale(
                resource_type.to_string(),
                ns.to_string(),
                name.to_string(),
                replicas,
            )
            .await
            .map_err(|e| e.to_string())
        }
        "k8s-node-action" => {
            let name = str_field(payload, &["name", "node"])
                .ok_or_else(|| "Missing 'name' for k8s-node-action".to_string())?;
            let action = str_field(payload, &["action"])
                .ok_or_else(|| "Missing 'action' for k8s-node-action".to_string())?;
            k8s_cluster::k8s_node_action(name.to_string(), action.to_string())
                .await
                .map_err(|e| e.to_string())
        }
        "k8s-port-forward-start" => {
            let ns = str_field(payload, &["namespace", "ns"]).unwrap_or("default");
            let name = str_field(payload, &["name", "pod", "service"])
                .ok_or_else(|| "Missing 'name' for k8s-port-forward-start".to_string())?;
            let local_port = u32_field(payload, &["localPort", "local_port"], 8080);
            let remote_port = u32_field(payload, &["remotePort", "remote_port"], local_port);
            let resource_type =
                str_field(payload, &["resourceType", "resource_type"]).unwrap_or("pod");
            k8s_cluster::k8s_pf_start(
                ns.to_string(),
                name.to_string(),
                local_port,
                remote_port,
                resource_type.to_string(),
            )
            .await
            .map_err(|e| e.to_string())
        }
        "k8s-port-forward-stop" => {
            let local_port = u32_field(payload, &["localPort", "local_port", "port"], 0);
            if local_port == 0 {
                return Err("Missing 'localPort' for k8s-port-forward-stop".to_string());
            }
            k8s_cluster::k8s_pf_stop(local_port)
                .await
                .map_err(|e| e.to_string())
        }
        "k8s-exec" => {
            let ns = str_field(payload, &["namespace", "ns"]).unwrap_or("default");
            let pod = str_field(payload, &["pod", "name"])
                .ok_or_else(|| "Missing 'pod' for k8s-exec".to_string())?;
            let container = str_field(payload, &["container"]).unwrap_or("");
            if let Some(cmd) = str_field(payload, &["command", "cmd"]) {
                if !crate::validation::is_valid_k8s_name(ns) {
                    return Err(format!("Invalid namespace: {ns}"));
                }
                if !crate::validation::is_valid_k8s_name(pod) {
                    return Err(format!("Invalid pod name: {pod}"));
                }
                if !container.is_empty() && !crate::validation::is_valid_k8s_name(container) {
                    return Err(format!("Invalid container name: {container}"));
                }
                if crate::validation::contains_shell_injection(cmd) {
                    return Err("Command contains forbidden characters".to_string());
                }
                let ns_owned = ns.to_string();
                let pod_owned = pod.to_string();
                let container_owned = container.to_string();
                let cmd_owned = cmd.to_string();
                crate::helpers::run_blocking(move || {
                    let mut args = vec!["exec", "-n", &ns_owned, &pod_owned];
                    if !container_owned.is_empty() {
                        args.push("-c");
                        args.push(&container_owned);
                    }
                    args.extend(["--", "sh", "-c", &cmd_owned]);
                    crate::helpers::run_cmd("kubectl", &args)
                })
                .await
            } else {
                kubernetes::k8s_exec(ns.to_string(), pod.to_string(), container.to_string())
                    .await
                    .map_err(|e| e.to_string())
            }
        }
        "kind-create" => {
            let name = str_field(payload, &["name"]).unwrap_or("kind");
            let image = str_field(payload, &["image"]).unwrap_or("");
            kind::kind_create(name.to_string(), image.to_string())
                .await
                .map_err(|e| e.to_string())
        }
        "kind-delete" => {
            let name = str_field(payload, &["name"])
                .ok_or_else(|| "Missing 'name' for kind-delete".to_string())?;
            kind::kind_delete(name.to_string())
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
        "lima-create" => {
            let name = str_field(payload, &["name"])
                .ok_or_else(|| "Missing 'name' for lima-create".to_string())?;
            let template = str_field(payload, &["template"]).unwrap_or("default");
            let cpus = u32_field(payload, &["cpus", "cpu"], 2);
            let memory = u32_field(payload, &["memory"], 4);
            let disk = u32_field(payload, &["disk"], 60);
            lima::lima_create(
                name.to_string(),
                template.to_string(),
                cpus,
                memory,
                disk,
            )
            .await
        }
        "lima-delete" => {
            let name = str_field(payload, &["name", "profile"])
                .ok_or_else(|| "Missing 'name' for lima-delete".to_string())?;
            let force = payload
                .get("force")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            lima::lima_delete(name.to_string(), force)
                .await
                .map_err(|e| e.to_string())
        }
        "lima-shell" => {
            let name = str_field(payload, &["name", "profile"])
                .ok_or_else(|| "Missing 'name' for lima-shell".to_string())?;
            let cmd = str_field(payload, &["command", "cmd"])
                .ok_or_else(|| "Missing 'command' for lima-shell".to_string())?;
            lima::lima_shell(name.to_string(), cmd.to_string())
                .await
                .map_err(|e| e.to_string())
        }
        "model-pull" => {
            let profile = str_field(payload, &["profile"]).unwrap_or("default");
            let model_name = str_field(payload, &["model", "modelName", "model_name", "name"])
                .ok_or_else(|| "Missing 'model' for model-pull".to_string())?;
            let runner = str_field(payload, &["runner"]).map(str::to_string);
            models::pull_model(profile.to_string(), model_name.to_string(), runner)
                .await
                .map_err(|e| e.to_string())
        }
        "model-serve" => {
            let profile = str_field(payload, &["profile"]).unwrap_or("default");
            let model_name = str_field(payload, &["model", "modelName", "model_name", "name"])
                .ok_or_else(|| "Missing 'model' for model-serve".to_string())?;
            let port_u32 = u32_field(payload, &["port"], 11434);
            let port = u16::try_from(port_u32).unwrap_or(11434);
            let runner = str_field(payload, &["runner"]).map(str::to_string);
            models::serve_model(profile.to_string(), model_name.to_string(), port, runner)
                .await
                .map_err(|e| e.to_string())
        }
        "model-delete" => {
            let profile = str_field(payload, &["profile"]).unwrap_or("default");
            let model_name = str_field(payload, &["model", "modelName", "model_name", "name"])
                .ok_or_else(|| "Missing 'model' for model-delete".to_string())?;
            let runner = str_field(payload, &["runner"]).map(str::to_string);
            models::delete_model(profile.to_string(), model_name.to_string(), runner)
                .await
                .map_err(|e| e.to_string())
        }
        "security-scan" => {
            let scan_id = default_scan_id(payload);
            let image_ref = str_field(payload, &["imageRef", "image_ref", "image"])
                .ok_or_else(|| "Missing 'imageRef' for security-scan".to_string())?;
            let refresh = payload.get("refresh").and_then(Value::as_bool);
            let res = security_scan::security_scan_image(scan_id, image_ref.to_string(), refresh)
                .await
                .map_err(|e| e.to_string())?;
            serde_json::to_string_pretty(&res).map_err(|e| e.to_string())
        }
        "security-sbom" => {
            let image_ref = str_field(payload, &["imageRef", "image_ref", "image"])
                .ok_or_else(|| "Missing 'imageRef' for security-sbom".to_string())?;
            let dest_dir = str_field(payload, &["destDir", "dest_dir", "dir"])
                .ok_or_else(|| "Missing 'destDir' for security-sbom".to_string())?;
            let file_name = str_field(payload, &["fileName", "file_name", "file"])
                .unwrap_or("sbom.json");
            let format = parse_sbom_format(payload);
            let overwrite = payload.get("overwrite").and_then(Value::as_bool);
            security_scan::security_sbom_export(
                image_ref.to_string(),
                dest_dir.to_string(),
                file_name.to_string(),
                format,
                overwrite,
            )
            .await
            .map_err(|e| e.to_string())
        }
        "security-audit" => {
            let scan_id = default_scan_id(payload);
            let image_ref = str_field(payload, &["imageRef", "image_ref", "image"])
                .ok_or_else(|| "Missing 'imageRef' for security-audit".to_string())?;
            let level = parse_security_level(payload);
            let refresh = payload.get("refresh").and_then(Value::as_bool);
            let audit =
                security_scan::security_audit_image(scan_id, image_ref.to_string(), level, refresh)
                    .await
                    .map_err(|e| e.to_string())?;
            serde_json::to_string_pretty(&audit).map_err(|e| e.to_string())
        }
        "security-triage" => {
            let level = parse_security_level(payload);
            let audit: security_scan::SecurityAudit = if let Some(audit_val) = payload.get("audit")
            {
                serde_json::from_value(audit_val.clone()).map_err(|e| e.to_string())?
            } else {
                let scan_id = default_scan_id(payload);
                let image_ref = str_field(payload, &["imageRef", "image_ref", "image"])
                    .ok_or_else(|| {
                        "Missing 'audit' or 'imageRef' for security-triage".to_string()
                    })?;
                let refresh = payload.get("refresh").and_then(Value::as_bool);
                security_scan::security_audit_image(scan_id, image_ref.to_string(), level, refresh)
                    .await
                    .map_err(|e| e.to_string())?
            };
            let res = security_triage::security_triage(audit, level)
                .await
                .map_err(|e| e.to_string())?;
            serde_json::to_string_pretty(&res).map_err(|e| e.to_string())
        }
        "detonation-start" => {
            let config: detonation::DetonationConfig =
                serde_json::from_value(payload.clone()).map_err(|e| e.to_string())?;
            detonation::detonation_start(config)
                .await
                .map_err(|e| e.to_string())
        }
        "detonation-cancel" => {
            let session_id = str_field(payload, &["sessionId", "session_id", "id"])
                .ok_or_else(|| "Missing 'sessionId' for detonation-cancel".to_string())?;
            let cancelled = detonation::detonation_cancel(session_id.to_string())
                .await
                .map_err(|e| e.to_string())?;
            Ok(json!({ "cancelled": cancelled, "sessionId": session_id }).to_string())
        }
        "ai-update-config" => {
            let _ = crate::commands::ai_chat::ensure_knowledge_bank_initialized();
            let Some(obj) = payload.as_object() else {
                return Err("Expected JSON object payload for ai-update-config".to_string());
            };
            if let Some(provider_val) = obj
                .get("ai_provider")
                .or_else(|| obj.get("provider"))
                .and_then(Value::as_str)
                .filter(|p| !is_canonical_ai_provider(p))
            {
                return Err(format!(
                    "Invalid ai_provider '{}'. Expected one of: {}",
                    provider_val,
                    CANONICAL_AI_PROVIDERS.join(", ")
                ));
            }

            let mut updated_keys = Vec::new();
            for (raw_k, raw_v) in obj {
                let Some(val_str) = raw_v.as_str() else {
                    continue;
                };
                let key = match raw_k.as_str() {
                    "provider" => "ai_provider",
                    "model" => "ai_model",
                    "endpoint" => "ai_endpoint",
                    other if other.starts_with("ai_") => other,
                    _ => continue,
                };
                knowledge_bank::set_setting(key.to_string(), val_str.to_string())
                    .await
                    .map_err(|e| e.to_string())?;
                updated_keys.push(key.to_string());
            }
            Ok(json!({ "updated": updated_keys }).to_string())
        }
        "save-preset" => {
            let _ = crate::commands::ai_chat::ensure_knowledge_bank_initialized();
            let id = str_field(payload, &["id", "name"])
                .ok_or_else(|| "Missing 'id' for save-preset".to_string())?;
            let config_json = if let Some(s) = payload
                .get("configJson")
                .or_else(|| payload.get("config_json"))
                .and_then(Value::as_str)
            {
                s.to_string()
            } else if let Some(cfg_obj) = payload.get("config") {
                serde_json::to_string(cfg_obj).map_err(|e| e.to_string())?
            } else {
                serde_json::to_string(payload).map_err(|e| e.to_string())?
            };
            knowledge_bank::save_preset(id.to_string(), config_json)
                .await
                .map_err(|e| e.to_string())
        }
        "delete-preset" => {
            let _ = crate::commands::ai_chat::ensure_knowledge_bank_initialized();
            let id = str_field(payload, &["id", "name"])
                .ok_or_else(|| "Missing 'id' for delete-preset".to_string())?;
            knowledge_bank::delete_preset(id.to_string())
                .await
                .map_err(|e| e.to_string())
        }
        "cli-exec" => {
            let base_cmd = str_field(payload, &["command", "cmd"])
                .ok_or_else(|| "Missing 'command' for cli-exec".to_string())?;
            let args = string_vec_field(payload, &["args"]);
            let full_cmd = if args.is_empty() {
                base_cmd.to_string()
            } else {
                format!("{base_cmd} {}", args.join(" "))
            };
            let res = shell_sandbox::sandbox_execute_approved(full_cmd)
                .await
                .map_err(|e| e.to_string())?;
            Ok(format_exec_result(&res))
        }
        _ => Err(format!("Unknown mutating event: {event}")),
    }
}

fn next_message_id(prefix: &str) -> String {
    static MSG_SEQ: AtomicU64 = AtomicU64::new(1);
    let seq = MSG_SEQ.fetch_add(1, Ordering::Relaxed);
    format!("{prefix}-{}-{seq}", chrono::Utc::now().timestamp_millis())
}

pub async fn run_headless_agent_detailed(
    req: HeadlessChatRequest,
) -> Result<HeadlessChatOutcome, String> {
    if req.prompt.trim().is_empty() {
        return Err("Prompt must not be empty".to_string());
    }

    let _ = crate::commands::ai_chat::ensure_knowledge_bank_initialized();

    let session_id = req
        .session_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(A2A_DEFAULT_CONVERSATION_ID)
        .to_string();

    crate::sse::emit_all(
        "agent-external-chat-turn",
        &json!({
            "phase": "start",
            "session_id": &session_id,
            "mirror_ui": req.mirror_ui,
            "prompt": &req.prompt,
        }),
    );

    let settings = load_ai_settings().await;
    let resolved = resolve_ai_config(&req, &settings);

    // 1. Embedded/disk application context + headless tool syntax addendum
    let home_dir = std::env::var("HOME").ok().map(std::path::PathBuf::from);
    let app_context = embedded_resources::load_app_context(home_dir);
    let mut system_prompt = build_headless_system_prompt(&app_context, req.allow_actions);

    // 2. Best-effort Knowledge Bank solutions & anti-patterns
    if let Some(ctx_text) = knowledge_bank::kb_query(req.prompt.clone())
        .await
        .ok()
        .map(|r| r.context_text.trim().to_string())
        .filter(|s| !s.is_empty())
    {
        system_prompt.push_str("\n\n");
        system_prompt.push_str(&ctx_text);
    }

    // 3. Best-effort agent memory recall
    if let Some(memories) = knowledge_bank::search_memory(req.prompt.clone(), 5)
        .await
        .ok()
        .filter(|m| !m.is_empty())
    {
        system_prompt.push_str("\n\n## Recalled User Memories & Preferences\n");
        for mem in memories {
            system_prompt.push_str("- ");
            system_prompt.push_str(mem.trim());
            system_prompt.push('\n');
        }
    }

    // Load up to last 10 clean turns (20 alternating messages) from session history
    let prior_history = crate::commands::ai_chat::ai_chat_load_history(Some(session_id.clone()))
        .await
        .unwrap_or_default();
    let clean_history = sanitize_history_turns(&prior_history, MAX_HISTORY_MESSAGES);

    let mut messages = Vec::with_capacity(2 + clean_history.len());
    messages.push(ChatMessage {
        role: "system".to_string(),
        content: system_prompt,
    });
    messages.extend(clean_history);
    messages.push(ChatMessage {
        role: "user".to_string(),
        content: req.prompt.clone(),
    });

    let mut tools_executed: Vec<ToolExecutionRecord> = Vec::new();
    let mut navigated_to: Option<String> = None;
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
        .await
        .map_err(|e| e.to_string())?;

        let pipe_tags = extract_tool_tags(&llm_reply);
        let directive_tags = extract_directive_tags(&llm_reply);
        let cleaned = strip_tool_tags(&llm_reply);
        if !cleaned.is_empty() {
            last_clean_reply = cleaned.clone();
        }

        if pipe_tags.is_empty() && directive_tags.is_empty() {
            last_clean_reply = cleaned;
            break;
        }

        let mut sections: Vec<String> = Vec::new();

        for tag in &pipe_tags {
            match dispatch_tool_tag(tag, req.allow_actions).await {
                Ok(output) => {
                    let is_blocked = output.starts_with("ACTION_BLOCKED:");
                    if !is_blocked && tag.event == "navigate" {
                        let parsed_page = serde_json::from_str::<Value>(&output)
                            .ok()
                            .and_then(|v| v.get("page").and_then(Value::as_str).map(str::to_string));
                        if let Some(p) = parsed_page {
                            navigated_to = Some(p);
                        }
                    }
                    tools_executed.push(ToolExecutionRecord {
                        kind: tag.kind.as_str().to_string(),
                        name: tag.event.clone(),
                        status: if is_blocked { "blocked" } else { "ok" }.to_string(),
                    });
                    sections.push(format!("### {} (ok)\n{}", tag.event, output));
                }
                Err(err) => {
                    tools_executed.push(ToolExecutionRecord {
                        kind: tag.kind.as_str().to_string(),
                        name: tag.event.clone(),
                        status: "error".to_string(),
                    });
                    sections.push(format!("### {} (error)\n{}", tag.event, err));
                }
            }
        }

        for dtag in &directive_tags {
            let label = if dtag.arg.is_empty() {
                dtag.kind.as_str().to_string()
            } else {
                format!("{}:{}", dtag.kind.as_str(), dtag.arg)
            };
            match dispatch_directive_tag(dtag, req.allow_actions, &mut navigated_to).await {
                Ok(Some(obs)) => {
                    let is_blocked = obs.starts_with("ACTION_BLOCKED:");
                    tools_executed.push(ToolExecutionRecord {
                        kind: dtag.kind.as_str().to_string(),
                        name: label.clone(),
                        status: if is_blocked { "blocked" } else { "ok" }.to_string(),
                    });
                    sections.push(format!("### {label} (ok)\n{obs}"));
                }
                Ok(None) => {
                    tools_executed.push(ToolExecutionRecord {
                        kind: dtag.kind.as_str().to_string(),
                        name: label,
                        status: "ok".to_string(),
                    });
                }
                Err(err) => {
                    tools_executed.push(ToolExecutionRecord {
                        kind: dtag.kind.as_str().to_string(),
                        name: label.clone(),
                        status: "error".to_string(),
                    });
                    sections.push(format!("### {label} (error)\n{err}"));
                }
            }
        }

        // If only passive side-effects (`[NAVIGATE]`, `[LEARN*]`, `[REMEMBER_PREFERENCE]`)
        // ran and the LLM already produced clean prose, no follow-up LLM turn is needed.
        if sections.is_empty() {
            if let Some(page) = navigated_to.as_deref().filter(|_| last_clean_reply.is_empty()) {
                last_clean_reply = format!("Navigated to the '{page}' page.");
            }
            break;
        }

        messages.push(ChatMessage {
            role: "assistant".to_string(),
            content: llm_reply,
        });

        last_tool_summary = sections.join("\n\n");
        messages.push(ChatMessage {
            role: "user".to_string(),
            content: format!(
                "[TOOL_RESULTS]\n{last_tool_summary}\n[/TOOL_RESULTS]\n\
                 Provide the final concise response for the user in their language based on the results above."
            ),
        });
    }

    let final_reply = if !last_clean_reply.is_empty() {
        last_clean_reply
    } else {
        last_tool_summary
    };

    // Persist user prompt and clean assistant reply to session history (best-effort).
    let _ = crate::commands::ai_chat::ai_chat_save_message(
        AiChatHistoryMessage {
            id: next_message_id("ext-u"),
            role: "user".to_string(),
            content: req.prompt.clone(),
        },
        Some(session_id.clone()),
    )
    .await;
    let _ = crate::commands::ai_chat::ai_chat_save_message(
        AiChatHistoryMessage {
            id: next_message_id("ext-a"),
            role: "assistant".to_string(),
            content: final_reply.clone(),
        },
        Some(session_id.clone()),
    )
    .await;

    crate::sse::emit_all(
        "agent-external-chat-turn",
        &json!({
            "phase": "complete",
            "session_id": &session_id,
            "mirror_ui": req.mirror_ui,
            "prompt": &req.prompt,
            "reply": &final_reply,
            "navigated_to": &navigated_to,
        }),
    );

    Ok(HeadlessChatOutcome {
        reply: final_reply,
        session_id,
        provider: resolved.provider,
        model: resolved.model,
        tools_executed,
        navigated_to,
    })
}

#[tauri::command]
pub async fn run_headless_agent(req: HeadlessChatRequest) -> Result<String, String> {
    let outcome = run_headless_agent_detailed(req).await?;
    Ok(outcome.reply)
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
    fn extracts_and_strips_non_pipe_directive_tags() {
        let turn = r#"Checking reference and navigating.
[READ_REFERENCE: skills/colima/SKILL.md]
[QUERY_STATE]
[DIAGNOSE]
[RUN: docker ps]
[RUN_APPROVE: colima start]
[LEARN: stale lock | remove ha.sock]
[LEARN_REASONING: Check XDG_CONFIG_HOME first]
[REMEMBER_PREFERENCE: Prefers VZ mount virtiofs]
[NAVIGATE: containers]"#;

        let directives = extract_directive_tags(turn);
        assert_eq!(directives.len(), 9);
        assert_eq!(
            directives[0],
            DirectiveTag {
                kind: DirectiveKind::ReadReference,
                arg: "skills/colima/SKILL.md".to_string()
            }
        );
        assert_eq!(
            directives[1],
            DirectiveTag {
                kind: DirectiveKind::QueryState,
                arg: String::new()
            }
        );
        assert_eq!(
            directives[2],
            DirectiveTag {
                kind: DirectiveKind::Diagnose,
                arg: String::new()
            }
        );
        assert_eq!(
            directives[3],
            DirectiveTag {
                kind: DirectiveKind::Run,
                arg: "docker ps".to_string()
            }
        );
        assert_eq!(
            directives[4],
            DirectiveTag {
                kind: DirectiveKind::RunApprove,
                arg: "colima start".to_string()
            }
        );
        assert_eq!(
            directives[5],
            DirectiveTag {
                kind: DirectiveKind::Learn,
                arg: "stale lock | remove ha.sock".to_string()
            }
        );
        assert_eq!(
            directives[6],
            DirectiveTag {
                kind: DirectiveKind::LearnReasoning,
                arg: "Check XDG_CONFIG_HOME first".to_string()
            }
        );
        assert_eq!(
            directives[7],
            DirectiveTag {
                kind: DirectiveKind::RememberPreference,
                arg: "Prefers VZ mount virtiofs".to_string()
            }
        );
        assert_eq!(
            directives[8],
            DirectiveTag {
                kind: DirectiveKind::Navigate,
                arg: "containers".to_string()
            }
        );

        assert_eq!(strip_tool_tags(turn), "Checking reference and navigating.");
    }

    #[test]
    fn extracts_tag_without_json_payload() {
        let tags = extract_tool_tags("[QUERY: list-instances]");
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].event, "list-instances");
        assert_eq!(tags[0].payload, json!({}));
    }

    #[test]
    fn classifies_safe_vs_mutating_events_across_full_catalog() {
        for safe in [
            "list-instances",
            "colima-status",
            "list-containers",
            "container-top",
            "volume-inspect",
            "network-inspect",
            "compose-ps",
            "compose-diagnose",
            "compose-autofix-history",
            "k8s-list-contexts",
            "k8s-current-context",
            "k8s-list-namespaces",
            "k8s-list-resources",
            "k8s-describe",
            "k8s-yaml",
            "k8s-logs",
            "k8s-container-logs",
            "k8s-pod-containers",
            "k8s-events",
            "k8s-crds",
            "k8s-crd-resources",
            "k8s-port-forward-list",
            "kind-list",
            "lima-info",
            "lima-templates",
            "system-check-tool",
            "ai-config-status",
            "list-presets",
            "colima-config-get",
            "topology-get",
            "security-history",
            "security-rules",
            "security-alternatives",
            "falco-state",
            "falco-events",
            "detonation-list",
            "alerts-rules",
            "alerts-events",
            "self-heal-rules",
            "self-heal-log",
            "activity-feed",
            "navigate",
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
            "create-instance",
            "delete-instance",
            "colima-config-apply",
            "run-container",
            "rename-container",
            "container-exec",
            "remove-image",
            "prune-images",
            "tag-image",
            "volume-create",
            "volume-remove",
            "volume-prune",
            "network-create",
            "network-remove",
            "network-prune",
            "system-prune",
            "compose-stop",
            "compose-start",
            "compose-autofix-propose",
            "compose-autofix-apply",
            "compose-autofix-undo",
            "k8s-set-context",
            "k8s-apply",
            "k8s-delete",
            "k8s-restart-resource",
            "k8s-scale",
            "k8s-generic-scale",
            "k8s-node-action",
            "k8s-port-forward-start",
            "k8s-port-forward-stop",
            "k8s-exec",
            "kind-create",
            "kind-delete",
            "lima-create",
            "lima-delete",
            "lima-shell",
            "model-pull",
            "model-serve",
            "model-delete",
            "security-scan",
            "security-sbom",
            "security-audit",
            "security-triage",
            "detonation-start",
            "detonation-cancel",
            "ai-update-config",
            "save-preset",
            "delete-preset",
            "cli-exec",
        ] {
            assert_eq!(
                classify_event(mutating),
                EventSafety::Mutating,
                "expected {mutating} to be Mutating"
            );
        }

        assert_eq!(classify_event("nonexistent-event"), EventSafety::Unknown);
    }

    #[test]
    fn redacts_all_secret_keys_in_ai_config_status() {
        let mut settings = HashMap::new();
        settings.insert("ai_provider".to_string(), "anthropic".to_string());
        settings.insert(
            "ai_model.anthropic".to_string(),
            "claude-sonnet-4-6".to_string(),
        );
        settings.insert(
            "ai_api_key.anthropic".to_string(),
            "sk-ant-super-secret".to_string(),
        );
        settings.insert("AUTH_TOKEN".to_string(), "tok-123".to_string());
        settings.insert("webhook_secret".to_string(), "sec-456".to_string());
        settings.insert("registry_Password".to_string(), "pw-789".to_string());

        let redacted = redact_ai_settings_map(&settings);
        assert_eq!(redacted["ai_provider"], "anthropic");
        assert_eq!(redacted["ai_model.anthropic"], "claude-sonnet-4-6");
        assert_eq!(redacted["ai_api_key.anthropic"], "[REDACTED]");
        assert_eq!(redacted["AUTH_TOKEN"], "[REDACTED]");
        assert_eq!(redacted["webhook_secret"], "[REDACTED]");
        assert_eq!(redacted["registry_Password"], "[REDACTED]");
    }

    #[tokio::test]
    async fn blocks_mutating_events_and_run_approve_when_allow_actions_is_false() {
        let tag = ToolTag {
            kind: TagKind::Query,
            event: "stop-container".to_string(),
            payload: json!({"id": "redis-prod"}),
        };

        let out = dispatch_tool_tag(&tag, false)
            .await
            .expect("blocked mutation returns an instructional note to the LLM");
        assert!(out.contains("ACTION_BLOCKED"));
        assert!(out.contains("--yes"));
        assert!(out.contains("stop-container"));

        let mut nav = None;
        let run_blocked = dispatch_directive_tag(
            &DirectiveTag {
                kind: DirectiveKind::RunApprove,
                arg: "colima stop".to_string(),
            },
            false,
            &mut nav,
        )
        .await
        .expect("run_approve blocked note")
        .expect("observation present");
        assert!(run_blocked.contains("ACTION_BLOCKED"));

        let read_ref = dispatch_directive_tag(
            &DirectiveTag {
                kind: DirectiveKind::ReadReference,
                arg: "skills/colima/SKILL.md".to_string(),
            },
            false,
            &mut nav,
        )
        .await
        .expect("read_reference succeeds")
        .expect("reference text returned");
        assert!(!read_ref.trim().is_empty());

        dispatch_directive_tag(
            &DirectiveTag {
                kind: DirectiveKind::Navigate,
                arg: "topology".to_string(),
            },
            false,
            &mut nav,
        )
        .await
        .expect("navigate directive succeeds");
        assert_eq!(nav.as_deref(), Some("topology"));
    }

    #[tokio::test]
    async fn executes_safe_environment_resolutions_without_allow_actions() {
        let tag = ToolTag {
            kind: TagKind::Query,
            event: "environment-resolutions".to_string(),
            payload: json!({}),
        };

        let out = dispatch_tool_tag(&tag, false)
            .await
            .expect("safe query should execute automatically");
        let parsed: Value = serde_json::from_str(&out).expect("valid JSON report");
        assert!(parsed["rows"].is_array());
    }

    #[test]
    fn sanitizes_history_to_strict_alternating_user_assistant_pairs() {
        let raw = vec![
            AiChatHistoryMessage {
                id: "1".into(),
                role: "assistant".into(),
                content: "orphan greeting".into(),
            },
            AiChatHistoryMessage {
                id: "2".into(),
                role: "user".into(),
                content: "first question".into(),
            },
            AiChatHistoryMessage {
                id: "3".into(),
                role: "user".into(),
                content: "[TOOL_RESULTS]\nnoise\n[/TOOL_RESULTS]".into(),
            },
            AiChatHistoryMessage {
                id: "4".into(),
                role: "assistant".into(),
                content: "first answer [NAVIGATE: containers]".into(),
            },
            AiChatHistoryMessage {
                id: "5".into(),
                role: "user".into(),
                content: "trailing unfinished user turn".into(),
            },
        ];

        let clean = sanitize_history_turns(&raw, 20);
        assert_eq!(clean.len(), 2);
        assert_eq!(clean[0].role, "user");
        assert_eq!(clean[0].content, "first question");
        assert_eq!(clean[1].role, "assistant");
        assert_eq!(clean[1].content, "first answer");
    }

    #[test]
    fn start_config_defaults_to_zero_overrides_so_colima_yaml_wins() {
        let cfg = build_start_config_from_payload(&json!({"profile": "work"}));
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
            ..Default::default()
        };
        let resolved = resolve_ai_config(&req, &settings);
        assert_eq!(resolved.provider, "anthropic");
        assert_eq!(resolved.model, "claude-opus-4-6");
        assert_eq!(resolved.api_key, "sk-ant-test");

        let override_req = HeadlessChatRequest {
            prompt: "status".to_string(),
            provider: Some("gemini".to_string()),
            model: None,
            allow_actions: false,
            ..Default::default()
        };
        let resolved_gemini = resolve_ai_config(&override_req, &HashMap::new());
        assert_eq!(resolved_gemini.provider, "gemini");
        assert_eq!(resolved_gemini.model, "gemini-2.5-flash");
    }
}
