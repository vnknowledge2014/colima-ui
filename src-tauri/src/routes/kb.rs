use axum::{
    extract::Query,
    http::StatusCode,
    response::Json,
};

use crate::api_server::*;
use crate::commands::*;
use crate::routes::payloads::*;

pub async fn api_get_settings(
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> (StatusCode, Json<ApiResponse<serde_json::Value>>) {
    if let Some(key) = params.get("key") {
        match knowledge_bank::get_setting(key.clone()).await {
            Ok(Some(val)) => ok(serde_json::Value::String(val)),
            Ok(None) => ok(serde_json::Value::Null),
            Err(e) => err(e),
        }
    } else {
        match knowledge_bank::get_all_settings().await {
            Ok(settings) => {
                let obj: serde_json::Map<String, serde_json::Value> = settings
                    .into_iter()
                    .map(|(k, v)| (k, serde_json::Value::String(v)))
                    .collect();
                ok(serde_json::Value::Object(obj))
            }
            Err(e) => err(e),
        }
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


pub async fn api_kb_feedback(
    Json(body): Json<KbFeedbackRequest>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    match knowledge_bank::kb_feedback(body.solution_id, body.is_like).await {
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

pub async fn api_kb_learn(
    Json(body): Json<KbLearnRequest>,
) -> (StatusCode, Json<ApiResponse<i64>>) {
    match knowledge_bank::kb_learn(body.error_pattern, body.solution_text).await {
        Ok(id) => ok(id),
        Err(e) => err(e.to_string()),
    }
}

pub async fn api_kb_add_memory(
    Json(body): Json<AddMemoryRequest>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    match knowledge_bank::add_memory(body.memory_type, body.content).await {
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

pub async fn api_diagnostics_logs(
    Query(q): Query<std::collections::HashMap<String, String>>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    let profile = q.get("profile").cloned().unwrap_or_else(|| "default".to_string());
    match colima::collect_diagnostic_logs(profile).await {
        Ok(report) => ok(report),
        Err(e) => err(e.to_string()),
    }
}

pub async fn api_load_preset_snapshot(
    Query(q): Query<PresetSnapshotQuery>,
) -> (StatusCode, Json<ApiResponse<Option<knowledge_bank::PresetSnapshot>>>) {
    match knowledge_bank::load_preset_snapshot(q.preset_id, q.instance_profile).await {
        Ok(snapshot) => ok(snapshot),
        Err(e) => err(e),
    }
}

pub async fn api_save_preset_snapshot(
    Json(body): Json<SavePresetSnapshotBody>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    match knowledge_bank::save_preset_snapshot(
        body.preset_id,
        body.instance_profile,
        body.containers_json,
        body.is_manual_override,
    )
    .await
    {
        Ok(msg) => ok(msg),
        Err(e) => err(e),
    }
}

pub async fn api_list_preset_snapshots(
    Query(q): Query<PresetSnapshotsListQuery>,
) -> (StatusCode, Json<ApiResponse<Vec<knowledge_bank::PresetSnapshot>>>) {
    match knowledge_bank::list_all_preset_snapshots(q.instance_profile).await {
        Ok(snapshots) => ok(snapshots),
        Err(e) => err(e),
    }
}

pub async fn api_get_presets(
) -> (StatusCode, Json<ApiResponse<std::collections::HashMap<String, String>>>) {
    match knowledge_bank::get_all_presets().await {
        Ok(presets) => ok(presets),
        Err(e) => err(e),
    }
}

pub async fn api_save_preset(
    Json(body): Json<SavePresetBody>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    match knowledge_bank::save_preset(body.id, body.config_json).await {
        Ok(msg) => ok(msg),
        Err(e) => err(e),
    }
}

pub async fn api_delete_preset(
    Query(q): Query<OptionalPresetIdQuery>,
    body: axum::body::Bytes,
) -> (StatusCode, Json<ApiResponse<String>>) {
    let from_query = q.id.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    let preset_id = match from_query {
        Some(id) => id,
        None => {
            if body.is_empty() {
                return err("Missing preset id".to_string());
            }
            match serde_json::from_slice::<DeletePresetBody>(&body) {
                Ok(parsed) => match parsed
                    .id
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                {
                    Some(id) => id,
                    None => return err("Missing preset id".to_string()),
                },
                Err(e) => return err(format!("Invalid delete preset payload: {}", e)),
            }
        }
    };
    match knowledge_bank::delete_preset(preset_id).await {
        Ok(msg) => ok(msg),
        Err(e) => err(e),
    }
}
