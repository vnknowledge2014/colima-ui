//! Browser-mode entry points for image scanning.
//!
//! Thin, like every other route module: the rules — what is a valid image
//! reference, where an SBOM may be written, what a cancelled scan means — live
//! in `commands::security_scan` so both transports get them.
//!
//! Progress reaches the caller over the existing SSE stream; there is no second
//! channel here. A scan carries the caller's `scanId`, which is also what
//! cancels it.

use axum::{extract::Query, http::StatusCode, response::Json};
use serde::Deserialize;

use crate::api_server::*;
use crate::commands::security_autofix;
use crate::commands::security_catalog;
use crate::commands::security_rules::{self, Level, RulePack};
use crate::commands::security_scan::{self, SbomFormat, ScanResult, SecurityAudit};
use crate::commands::security_history;
use crate::commands::security_policy;
use crate::commands::security_triage;
use crate::commands::security_watch;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanBody {
    pub scan_id: String,
    pub image_ref: String,
    /// Ignore a cached result for this image and run the scanner again.
    #[serde(default)]
    pub refresh: bool,
}

pub async fn api_security_scan(
    Json(body): Json<ScanBody>,
) -> (StatusCode, Json<ApiResponse<ScanResult>>) {
    let result = crate::helpers::run_blocking(move || {
        security_scan::scan_image_blocking(&body.scan_id, &body.image_ref, body.refresh)
    })
    .await;

    match result {
        Ok(scan) => ok(scan),
        // A scanner that cannot read one image is an error about that image, not
        // a broken endpoint — the caller shows the reason next to the image and
        // keeps the rest of its list.
        Err(e) => err(crate::error::ColimaError::command_failed(e)),
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanCancelBody {
    pub scan_id: String,
}

pub async fn api_security_scan_cancel(
    Json(body): Json<ScanCancelBody>,
) -> (StatusCode, Json<ApiResponse<bool>>) {
    match security_scan::cancel(&body.scan_id) {
        Ok(cancelled) => ok(cancelled),
        Err(e) => err(crate::error::ColimaError::validation(e)),
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditBody {
    pub scan_id: String,
    pub image_ref: String,
    /// Strictness. Absent means L1, the default the UI shows.
    #[serde(default)]
    pub level: Level,
    #[serde(default)]
    pub refresh: bool,
}

/// Scan, evaluate the rules, and score — in one request.
///
/// Not three endpoints: a score is only meaningful beside the findings and rule
/// results it was computed from, and assembling it client-side is how a score
/// ends up shown next to a different image's scan.
pub async fn api_security_audit(
    Json(body): Json<AuditBody>,
) -> (StatusCode, Json<ApiResponse<SecurityAudit>>) {
    // The clock is read here, at the edge, so everything below it stays a pure
    // function of its arguments.
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);

    let result = crate::helpers::run_blocking(move || {
        security_scan::audit_image_blocking(
            &body.scan_id,
            &body.image_ref,
            body.level,
            body.refresh,
            now,
        )
    })
    .await;

    match result {
        Ok(audit) => ok(audit),
        Err(e) => err(crate::error::ColimaError::command_failed(e)),
    }
}

#[derive(Deserialize)]
pub struct TriageBody {
    pub audit: SecurityAudit,
    #[serde(default)]
    pub level: Option<Level>,
}

/// Order the failing rules by what fixing each is worth.
///
/// Takes the audit rather than re-running it: triage is a pure re-scoring of a
/// result the caller already has, and scanning again would be both slow and a
/// different measurement.
pub async fn api_security_triage(
    Json(body): Json<TriageBody>,
) -> (StatusCode, Json<ApiResponse<security_triage::TriageResult>>) {
    ok(security_triage::triage(
        &body.audit,
        body.level.unwrap_or_default(),
    ))
}

#[derive(Deserialize)]
pub struct SecurityAutofixBody {
    #[serde(alias = "dockerfilePath")]
    pub dockerfile_path: String,
}

pub async fn api_security_autofix_propose(
    Json(body): Json<SecurityAutofixBody>,
) -> (StatusCode, Json<ApiResponse<Vec<security_autofix::SecurityPatch>>>) {
    match security_triage::security_autofix_propose(body.dockerfile_path).await {
        Ok(patches) => ok(patches),
        Err(e) => err(e),
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryQuery {
    pub image_digest: String,
}

pub async fn api_security_history(
    Query(q): Query<HistoryQuery>,
) -> (StatusCode, Json<ApiResponse<Vec<security_history::ScanRun>>>) {
    if let Err(e) = security_policy::require_entitled() {
        return err(e);
    }
    match security_history::history(&q.image_digest) {
        Ok(runs) => ok(runs),
        Err(e) => err(crate::error::ColimaError::internal(e)),
    }
}

pub async fn api_security_policy_list(
) -> (StatusCode, Json<ApiResponse<Vec<security_policy::PolicyRule>>>) {
    if let Err(e) = security_policy::require_entitled() {
        return err(e);
    }
    match security_policy::list_rules() {
        Ok(rules) => ok(rules),
        Err(e) => err(crate::error::ColimaError::internal(e)),
    }
}

pub async fn api_security_policy_save(
    Json(rule): Json<security_policy::PolicyRule>,
) -> (StatusCode, Json<ApiResponse<i64>>) {
    if let Err(e) = security_policy::require_entitled() {
        return err(e);
    }
    match security_policy::upsert_rule(&rule) {
        Ok(id) => ok(id),
        Err(e) => err(crate::error::ColimaError::validation(e)),
    }
}

#[derive(Debug, Deserialize)]
pub struct PolicyDeleteBody {
    pub id: i64,
}

pub async fn api_security_policy_delete(
    Json(body): Json<PolicyDeleteBody>,
) -> (StatusCode, Json<ApiResponse<bool>>) {
    if let Err(e) = security_policy::require_entitled() {
        return err(e);
    }
    match security_policy::delete_rule(body.id) {
        Ok(()) => ok(true),
        Err(e) => err(crate::error::ColimaError::validation(e)),
    }
}

/// Never gated: the page must be able to show a background job that is running
/// even when the subscription that started it has lapsed.
pub async fn api_security_watch_state(
) -> (StatusCode, Json<ApiResponse<security_watch::WatchState>>) {
    ok(security_watch::state())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WatchSetBody {
    #[serde(default)]
    pub enabled: Option<bool>,
    #[serde(default)]
    pub interval_hours: Option<i64>,
}

/// Turning the watcher **off** is deliberately not gated here either — the
/// entitlement check lives in `set_enabled`, and it only applies to switching
/// on.
pub async fn api_security_watch_set(
    Json(body): Json<WatchSetBody>,
) -> (StatusCode, Json<ApiResponse<security_watch::WatchState>>) {
    if let Some(hours) = body.interval_hours {
        if let Err(e) = security_watch::set_interval_hours(hours) {
            return err(crate::error::ColimaError::validation(e));
        }
    }
    match body.enabled {
        Some(enabled) => match security_watch::set_enabled(enabled) {
            Ok(state) => ok(state),
            Err(e) => err(crate::error::ColimaError::validation(e)),
        },
        None => ok(security_watch::state()),
    }
}

/// The rule pack this build carries, so the UI can explain a failed rule without
/// the backend repeating its text in every result.
pub async fn api_security_rule_pack() -> (StatusCode, Json<ApiResponse<&'static RulePack>>) {
    ok(security_rules::rule_pack())
}

#[derive(Debug, Deserialize)]
pub struct AlternativesQuery {
    pub image: String,
}

/// Base images to consider instead of this one.
///
/// A query parameter rather than a body because it reads nothing and changes
/// nothing — and the image name goes no further than this process: the catalog
/// is a table the app already carries.
pub async fn api_security_alternatives(
    Query(q): Query<AlternativesQuery>,
) -> (StatusCode, Json<ApiResponse<security_catalog::CatalogSuggestions>>) {
    ok(security_catalog::suggestions_for(&q.image))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SbomBody {
    pub image_ref: String,
    pub dest_dir: String,
    pub file_name: String,
    pub format: SbomFormat,
    /// Absent means no: replacing a file the caller did not mention is the kind
    /// of thing that has to be asked for.
    #[serde(default)]
    pub overwrite: bool,
}

pub async fn api_security_sbom(
    Json(body): Json<SbomBody>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    let result = crate::helpers::run_blocking(move || {
        security_scan::export_sbom_blocking(
            &body.image_ref,
            &body.dest_dir,
            &body.file_name,
            body.format,
            body.overwrite,
        )
    })
    .await;

    match result {
        Ok(path) => ok(path),
        Err(e) => err(crate::error::ColimaError::command_failed(e)),
    }
}

// ===== Runtime events (Falco) =====
//
// Thin like the rest of this module. Whether Falco is usable, and what counts
// as usable, is decided in `commands::falco_bridge` — the Tauri path has to
// enforce exactly the same answer, and two implementations would drift.

pub async fn api_falco_state(
) -> (StatusCode, Json<ApiResponse<crate::commands::falco_bridge::FalcoState>>) {
    // Detection SSHes into the VM, so it is blocking work; it cannot fail in a
    // way worth reporting, because "no Falco" is one of its answers rather than
    // an error.
    match crate::helpers::run_blocking(|| Ok(crate::commands::falco_bridge::detect())).await {
        Ok(state) => ok(state),
        Err(e) => err(crate::error::ColimaError::from(e)),
    }
}

pub async fn api_falco_events(
    Query(q): Query<LimitQuery>,
) -> (
    StatusCode,
    Json<ApiResponse<Vec<crate::commands::security_history::StoredFalcoEvent>>>,
) {
    match crate::commands::security_history::falco_events(q.limit.unwrap_or(200)) {
        Ok(events) => ok(events),
        Err(e) => err(crate::error::ColimaError::from(e)),
    }
}

#[derive(Debug, Deserialize)]
pub struct LimitQuery {
    pub limit: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WatchBody {
    pub enabled: bool,
}

/// Turning the reader off is never gated — see `falco_bridge::set_watching`.
pub async fn api_falco_watch(
    Json(body): Json<WatchBody>,
) -> (StatusCode, Json<ApiResponse<bool>>) {
    match crate::commands::falco_bridge::set_watching(body.enabled) {
        Ok(v) => ok(v),
        Err(e) => err(crate::error::ColimaError::from(e)),
    }
}

pub async fn api_falco_watch_state() -> (StatusCode, Json<ApiResponse<bool>>) {
    ok(crate::commands::falco_bridge::is_watching())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExplainBody {
    pub event_ids: Vec<i64>,
}

/// Builds the prompt; sending it is the caller's job, with the caller's key.
pub async fn api_falco_explain(
    Json(body): Json<ExplainBody>,
) -> (
    StatusCode,
    Json<ApiResponse<crate::commands::falco_triage::FalcoExplainPayload>>,
) {
    let stored = match crate::commands::security_history::falco_events(500) {
        Ok(s) => s,
        Err(e) => return err(crate::error::ColimaError::from(e)),
    };
    let selected: Vec<_> = stored
        .into_iter()
        .filter(|e| body.event_ids.contains(&e.id))
        .collect();
    ok(crate::commands::falco_triage::build_explain_payload(&selected))
}
