//! HTTP mirror of the colima.yaml commands.
//!
//! Browser mode has to reach every one of these: the config page and the Help
//! page are the same Svelte components in both modes, so a route missing here
//! shows up as a feature that silently works in the app and not in the browser.

use axum::{extract::Query, http::StatusCode, response::Json};
use serde::Deserialize;
use std::collections::HashMap;

use crate::commands::colima_config::{self, ApplyResult, ConfigChanges, ConfigSnapshot};
use crate::commands::kb_articles::{self, Article, ArticleSummary};
use crate::helpers::{err, ok, ApiResponse};

fn profile_of(q: &HashMap<String, String>) -> String {
    q.get("profile")
        .cloned()
        .unwrap_or_else(|| "default".to_string())
}

fn locale_of(q: &HashMap<String, String>) -> String {
    q.get("locale").cloned().unwrap_or_else(|| "en".to_string())
}

pub async fn api_get_colima_config(
    Query(q): Query<HashMap<String, String>>,
) -> (StatusCode, Json<ApiResponse<ConfigSnapshot>>) {
    match colima_config::get_colima_config(profile_of(&q)).await {
        Ok(v) => ok(v),
        Err(e) => err(e),
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewConfigRequest {
    pub profile: String,
    pub changes: ConfigChanges,
}

pub async fn api_preview_colima_config(
    Json(body): Json<PreviewConfigRequest>,
) -> (StatusCode, Json<ApiResponse<ApplyResult>>) {
    match colima_config::preview_colima_config(body.profile, body.changes).await {
        Ok(v) => ok(v),
        Err(e) => err(e),
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyConfigRequest {
    pub profile: String,
    pub changes: ConfigChanges,
    /// Echoed from the snapshot the form was loaded with — see
    /// `apply_colima_config` for why a stale value must not be written.
    pub expected_mtime: i64,
}

pub async fn api_apply_colima_config(
    Json(body): Json<ApplyConfigRequest>,
) -> (StatusCode, Json<ApiResponse<ApplyResult>>) {
    match colima_config::apply_colima_config(body.profile, body.changes, body.expected_mtime).await
    {
        Ok(v) => ok(v),
        Err(e) => err(e),
    }
}

pub async fn api_kb_list_articles(
    Query(q): Query<HashMap<String, String>>,
) -> (StatusCode, Json<ApiResponse<Vec<ArticleSummary>>>) {
    match kb_articles::kb_list_articles(locale_of(&q)).await {
        Ok(v) => ok(v),
        Err(e) => err(e),
    }
}

pub async fn api_kb_get_article(
    Query(q): Query<HashMap<String, String>>,
) -> (StatusCode, Json<ApiResponse<Article>>) {
    let slug = q.get("slug").cloned().unwrap_or_default();
    match kb_articles::kb_get_article(slug, locale_of(&q)).await {
        Ok(v) => ok(v),
        Err(e) => err(e),
    }
}

pub async fn api_kb_search_articles(
    Query(q): Query<HashMap<String, String>>,
) -> (StatusCode, Json<ApiResponse<Vec<ArticleSummary>>>) {
    let query = q.get("q").cloned().unwrap_or_default();
    match kb_articles::kb_search_articles(query, locale_of(&q)).await {
        Ok(v) => ok(v),
        Err(e) => err(e),
    }
}
