use axum::{
    extract::Query,
    http::StatusCode,
    response::Json,
};
use serde::Deserialize;
use crate::api_server::*;
use crate::commands::containers;
use crate::routes::payloads::*;

/// List containers — delegates to Tauri command (CLI fallback path, no Bollard)
pub async fn api_list_containers(
    Query(q): Query<ContainerQuery>,
) -> (StatusCode, Json<ApiResponse<Vec<serde_json::Value>>>) {
    let all = q.all;
    match containers::list_containers_cli(all).await {
        Ok(list) => ok(list),
        Err(e) => err(e),
    }
}


pub async fn api_start_container(
    Query(q): Query<ContainerIdQuery>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    match containers::start_container(q.container_id).await {
        Ok(msg) => ok(msg),
        Err(e) => err(e),
    }
}


pub async fn api_stop_container(
    Query(q): Query<ContainerIdQuery>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    match containers::stop_container(q.container_id).await {
        Ok(msg) => ok(msg),
        Err(e) => err(e),
    }
}


pub async fn api_restart_container(
    Query(q): Query<ContainerIdQuery>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    match containers::restart_container(q.container_id).await {
        Ok(msg) => ok(msg),
        Err(e) => err(e),
    }
}


pub async fn api_remove_container(
    Query(q): Query<ContainerIdQuery>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    match containers::remove_container(q.container_id, q.force).await {
        Ok(msg) => ok(msg),
        Err(e) => err(e),
    }
}


pub async fn api_container_logs(
    Query(q): Query<ContainerIdQuery>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    match containers::container_logs(q.container_id, q.lines).await {
        Ok(logs) => ok(logs),
        Err(e) => err(e),
    }
}


pub async fn api_inspect_container(
    Query(q): Query<ContainerIdQuery>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    match containers::inspect_container(q.container_id).await {
        Ok(info) => ok(info),
        Err(e) => err(e),
    }
}


pub async fn api_container_stats(
    Query(q): Query<ContainerIdQuery>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    match containers::container_stats(q.container_id).await {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}


pub async fn api_all_container_stats() -> (StatusCode, Json<ApiResponse<String>>) {
    match containers::all_container_stats().await {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}


pub async fn api_container_top(
    Query(q): Query<ContainerIdQuery>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    match containers::container_top(q.container_id).await {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}


pub async fn api_container_exec(
    Json(body): Json<ContainerExecBody>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    // Validation now happens inside command function
    match containers::container_exec(body.container_id, body.command).await {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}


pub async fn api_run_container(
    Json(body): Json<RunContainerBody>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    // Validation now happens inside command function
    match containers::run_container(
        body.image,
        body.name,
        body.ports,
        body.env_vars,
        body.volumes,
        body.detach,
        body.remove_on_exit,
        body.extra_args,
    )
    .await
    {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}


pub async fn api_rename_container(
    Json(body): Json<RenameContainerBody>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    match containers::rename_container(body.container_id, body.new_name).await {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}


pub async fn api_pause_container(
    Query(q): Query<ContainerIdQuery>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    match containers::pause_container(q.container_id).await {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}


pub async fn api_unpause_container(
    Query(q): Query<ContainerIdQuery>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    match containers::unpause_container(q.container_id).await {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}
