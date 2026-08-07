use axum::{
    extract::Query,
    http::StatusCode,
    response::Json,
};
use crate::api_server::*;
use crate::commands::colima;
use crate::instance_reader;
use crate::routes::payloads::*;

pub async fn api_list_instances() -> (StatusCode, Json<ApiResponse<Vec<colima::ColimaInstance>>>) {
    // Direct filesystem read — instant (<1ms) vs CLI (30-60s)
    let instances = instance_reader::list_instances_fast();
    ok(instances)
}


pub async fn api_start_instance(
    Json(config): Json<colima::StartConfig>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    match colima::start_instance(config).await {
        Ok(msg) => ok(msg),
        Err(e) => err(e),
    }
}


pub async fn api_stop_instance(
    Query(q): Query<ProfileQuery>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    // Note: command stop_instance uses Tauri state for Bollard reconnect.
    // HTTP route does CLI-only stop (no Bollard state to reconnect).
    match colima::stop_instance_cli(q.profile, q.force).await {
        Ok(msg) => ok(msg),
        Err(e) => err(e),
    }
}


pub async fn api_delete_instance(
    Query(q): Query<ProfileQuery>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    // Same note as stop: command uses Tauri state; route does CLI-only.
    match colima::delete_instance_cli(q.profile, q.force).await {
        Ok(msg) => ok(msg),
        Err(e) => err(e),
    }
}


pub async fn api_instance_status(
    Query(q): Query<ProfileQuery>,
) -> (StatusCode, Json<ApiResponse<colima::InstanceStatus>>) {
    match colima::instance_status(q.profile).await {
        Ok(status) => ok(status),
        Err(e) => err(e),
    }
}


pub async fn api_ssh_command(
    Query(q): Query<ProfileQuery>,
) -> (StatusCode, Json<ApiResponse<Vec<String>>>) {
    match colima::get_ssh_command(q.profile).await {
        Ok(args) => ok(args),
        Err(e) => err(e),
    }
}

