use axum::{http::StatusCode, response::Json};
use crate::api_server::*;
use crate::commands::diagnostics::{self, DiagnosticBundle};
use crate::routes::payloads::*;

/// Build a diagnostic bundle.
///
/// A POST because it runs collectors against the daemon and the host, not
/// because it changes anything. Nothing here transmits the result: the response
/// goes back to the caller and the decision to share it stays with the user.
pub async fn api_diagnostic_bundle(
    Json(body): Json<DiagnosticBundleBody>,
) -> (StatusCode, Json<ApiResponse<DiagnosticBundle>>) {
    ok(diagnostics::build_bundle(body.error, body.container_id, body.log_lines).await)
}

/// Write the selected sections to disk as Markdown.
pub async fn api_save_diagnostic_bundle(
    Json(body): Json<SaveDiagnosticBundleBody>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    match diagnostics::save_bundle(
        &body.bundle,
        &body.include,
        &body.dest_dir,
        &body.file_name,
        body.overwrite.unwrap_or(false),
    ) {
        Ok(path) => ok(path),
        Err(e) => err(e),
    }
}
