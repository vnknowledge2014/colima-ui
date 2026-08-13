//! HTTP surface for self-healing rules, so browser mode reaches the same state
//! the desktop app does.
//!
//! The kill switch is served without an entitlement check, matching the Tauri
//! command it mirrors: a person has to be able to read and clear it whatever
//! their subscription says. Acting on a rule is still decided in the executor,
//! at the moment it acts — no route here performs a repair.

use axum::{extract::Query, http::StatusCode, response::Json};
use serde::Deserialize;

use crate::api_server::*;
use crate::commands::self_heal::{self, HealLogEntry, HealMode, HealRule};

pub async fn api_self_heal_list_rules() -> (StatusCode, Json<ApiResponse<Vec<HealRule>>>) {
    match crate::helpers::run_blocking(self_heal::list_rules).await {
        Ok(rules) => ok(rules),
        Err(e) => err(crate::error::ColimaError::internal(e)),
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveRuleBody {
    pub id: i64,
    pub mode: HealMode,
    pub threshold: f64,
    pub max_per_hour: i64,
    pub enabled: bool,
}

pub async fn api_self_heal_save_rule(
    Json(body): Json<SaveRuleBody>,
) -> (StatusCode, Json<ApiResponse<()>>) {
    match crate::helpers::run_blocking(move || {
        self_heal::update_rule(
            body.id,
            body.mode,
            body.threshold,
            body.max_per_hour,
            body.enabled,
        )
    })
    .await
    {
        Ok(()) => ok(()),
        // Asking for Auto on a rule that can only advise is bad input rather
        // than a failure: the answer is the same however often it is retried.
        Err(e) => err(crate::error::ColimaError::validation(e)),
    }
}

#[derive(Debug, Deserialize)]
pub struct LogQuery {
    pub limit: Option<i64>,
}

pub async fn api_self_heal_log(
    Query(q): Query<LogQuery>,
) -> (StatusCode, Json<ApiResponse<Vec<HealLogEntry>>>) {
    let limit = q.limit.unwrap_or(50);
    match crate::helpers::run_blocking(move || self_heal::recent_log(limit)).await {
        Ok(entries) => ok(entries),
        Err(e) => err(crate::error::ColimaError::internal(e)),
    }
}

pub async fn api_self_heal_enabled() -> (StatusCode, Json<ApiResponse<bool>>) {
    ok(self_heal::is_enabled())
}

#[derive(Debug, Deserialize)]
pub struct EnabledBody {
    pub on: bool,
}

pub async fn api_self_heal_set_enabled(
    Json(body): Json<EnabledBody>,
) -> (StatusCode, Json<ApiResponse<()>>) {
    match crate::helpers::run_blocking(move || self_heal::set_enabled(body.on)).await {
        Ok(()) => ok(()),
        Err(e) => err(crate::error::ColimaError::internal(e)),
    }
}
