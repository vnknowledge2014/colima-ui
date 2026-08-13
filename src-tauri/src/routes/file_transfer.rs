//! The browser-mode entry points for background transfers.
//!
//! These pass `None` for the `AppHandle`: there is no desktop window to emit
//! Tauri events into, and progress reaches the caller over the SSE stream. The
//! commands publish to SSE unconditionally for exactly this reason.
//!
//! Note that these routes are reachable with a valid token regardless of any
//! dialog the desktop UI may have shown. The file picker records the user's intent
//! in the UI; it is not an authorization mechanism. Confinement of written paths is
//! enforced inside `commands::file_transfer`, on every path, not at this layer.

use axum::{http::StatusCode, response::Json};
use crate::api_server::*;
use crate::commands::file_transfer::{self, TransferStarted};
use crate::routes::payloads::*;

pub async fn api_image_save(
    Json(body): Json<ImageSaveBody>,
) -> (StatusCode, Json<ApiResponse<TransferStarted>>) {
    match file_transfer::start_image_save(
        None,
        body.images,
        body.dest_dir,
        body.file_name,
        body.overwrite.unwrap_or(false),
    ) {
        Ok(started) => ok(started),
        Err(e) => err(e),
    }
}

pub async fn api_image_load(
    Json(body): Json<ImageLoadBody>,
) -> (StatusCode, Json<ApiResponse<TransferStarted>>) {
    match file_transfer::start_image_load(None, body.tar_path) {
        Ok(started) => ok(started),
        Err(e) => err(e),
    }
}

pub async fn api_copy_to_container(
    Json(body): Json<CopyToContainerBody>,
) -> (StatusCode, Json<ApiResponse<TransferStarted>>) {
    match file_transfer::start_copy_to_container(
        None,
        body.container_id,
        body.host_path,
        body.container_path,
    ) {
        Ok(started) => ok(started),
        Err(e) => err(e),
    }
}

pub async fn api_copy_from_container(
    Json(body): Json<CopyFromContainerBody>,
) -> (StatusCode, Json<ApiResponse<TransferStarted>>) {
    match file_transfer::start_copy_from_container(
        None,
        body.container_id,
        body.container_path,
        body.dest_dir,
        body.file_name,
        body.overwrite.unwrap_or(false),
    ) {
        Ok(started) => ok(started),
        Err(e) => err(e),
    }
}

pub async fn api_cancel_transfer(
    Json(body): Json<CancelTransferBody>,
) -> (
    StatusCode,
    Json<ApiResponse<crate::transfer_registry::CancelOutcome>>,
) {
    ok(file_transfer::cancel_transfer_job(&body.job_id))
}

/// Every transfer this process knows about.
///
/// The reconciliation path: events are the fast route and are lossy — the SSE
/// channel drops frames under lag by design — so a client that reconnects asks here
/// instead of assuming the job it last saw is still running.
pub async fn api_transfer_list() -> (
    StatusCode,
    Json<ApiResponse<Vec<crate::transfer_registry::TransferSnapshot>>>,
) {
    ok(crate::transfer_registry::list())
}
