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
use crate::commands::security_catalog;
use crate::commands::security_rules::{self, Level, RulePack};
use crate::commands::security_scan::{self, SbomFormat, ScanResult, SecurityAudit};

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
