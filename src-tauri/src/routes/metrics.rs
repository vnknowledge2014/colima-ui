//! History and alert endpoints for the Activity page.
//!
//! History is read-only: writing is the collector's business, and an endpoint
//! that inserted samples would let anything holding the local token forge it.
//! Alert rules are writable, because they are the user's own configuration.

use axum::{extract::Query, http::StatusCode, response::Json};
use serde::Deserialize;

use crate::api_server::*;
use crate::commands::alerts::{self, AlertEvent, AlertRule, BacktestResult};
use crate::commands::metrics_store::{self, HistorySeries, WriterHealth};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryQuery {
    /// Unix milliseconds, inclusive.
    pub from: i64,
    pub to: i64,
    /// Comma-separated container ids. Absent means every container.
    pub containers: Option<String>,
}

pub async fn api_metrics_history(
    Query(q): Query<HistoryQuery>,
) -> (StatusCode, Json<ApiResponse<HistorySeries>>) {
    let ids: Vec<String> = q
        .containers
        .unwrap_or_default()
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    match crate::helpers::run_blocking(move || metrics_store::history(q.from, q.to, &ids)).await {
        Ok(series) => ok(series),
        Err(e) => err(crate::error::ColimaError::internal(e)),
    }
}

/// Whether history is being written, and what has been lost.
///
/// Dropped batches are a fact about the data, not a diagnostic: a gap in a graph
/// that the user cannot account for is worse than one that is labelled.
pub async fn api_metrics_health() -> (StatusCode, Json<ApiResponse<WriterHealth>>) {
    ok(metrics_store::health(crate::commands::metrics_store::is_writing()))
}

// ===== Alerts =====

pub async fn api_alerts_list_rules() -> (StatusCode, Json<ApiResponse<Vec<AlertRule>>>) {
    match crate::helpers::run_blocking(alerts::list_rules).await {
        Ok(rules) => ok(rules),
        Err(e) => err(crate::error::ColimaError::internal(e)),
    }
}

pub async fn api_alerts_save_rule(
    Json(rule): Json<AlertRule>,
) -> (StatusCode, Json<ApiResponse<i64>>) {
    match crate::helpers::run_blocking(move || alerts::upsert_rule(&rule)).await {
        Ok(id) => ok(id),
        // A rule with a zero duration or cooldown is bad input, not a failure:
        // it would defeat the two conditions that make an alert meaningful.
        Err(e) => err(crate::error::ColimaError::validation(e)),
    }
}

#[derive(Debug, Deserialize)]
pub struct RuleIdBody {
    pub id: i64,
}

pub async fn api_alerts_delete_rule(
    Json(body): Json<RuleIdBody>,
) -> (StatusCode, Json<ApiResponse<()>>) {
    match crate::helpers::run_blocking(move || alerts::delete_rule(body.id)).await {
        Ok(()) => ok(()),
        Err(e) => err(crate::error::ColimaError::internal(e)),
    }
}

#[derive(Debug, Deserialize)]
pub struct EventsQuery {
    pub limit: Option<i64>,
}

pub async fn api_alerts_events(
    Query(q): Query<EventsQuery>,
) -> (StatusCode, Json<ApiResponse<Vec<AlertEvent>>>) {
    let limit = q.limit.unwrap_or(50);
    match crate::helpers::run_blocking(move || alerts::recent_events(limit)).await {
        Ok(events) => ok(events),
        Err(e) => err(crate::error::ColimaError::internal(e)),
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BacktestBody {
    pub rule: AlertRule,
    pub from_ms: i64,
    pub to_ms: i64,
}

pub async fn api_alerts_backtest(
    Json(body): Json<BacktestBody>,
) -> (StatusCode, Json<ApiResponse<BacktestResult>>) {
    match crate::helpers::run_blocking(move || alerts::backtest(&body.rule, body.from_ms, body.to_ms))
        .await
    {
        Ok(result) => ok(result),
        Err(e) => err(crate::error::ColimaError::internal(e)),
    }
}
