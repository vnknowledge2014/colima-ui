use axum::{
    extract::Query,
    http::StatusCode,
    response::Json,
};
use crate::api_server::*;
use crate::commands::volumes;
use crate::routes::payloads::*;

pub async fn api_list_volumes() -> (StatusCode, Json<ApiResponse<Vec<volumes::DockerVolume>>>) {
    match volumes::list_volumes().await {
        Ok(list) => ok(list),
        Err(e) => err(e.to_string()),
    }
}


pub async fn api_create_volume(
    Json(body): Json<CreateVolumeBody>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    match volumes::create_volume(body.name, body.driver).await {
        Ok(out) => ok(out),
        Err(e) => err(e.to_string()),
    }
}


pub async fn api_remove_volume(
    Query(q): Query<VolumeNameQuery>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    let force = q.force.unwrap_or(false);
    match volumes::remove_volume(q.name, force).await {
        Ok(out) => ok(out),
        Err(e) => err(e.to_string()),
    }
}


pub async fn api_prune_volumes() -> (StatusCode, Json<ApiResponse<String>>) {
    match volumes::prune_volumes().await {
        Ok(out) => ok(out),
        Err(e) => err(e.to_string()),
    }
}


pub async fn api_inspect_volume(
    Query(q): Query<VolumeNameQuery>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    match volumes::inspect_volume(q.name).await {
        Ok(out) => ok(out),
        Err(e) => err(e.to_string()),
    }
}
