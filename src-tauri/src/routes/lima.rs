use axum::{
    http::StatusCode,
    response::Json,
};
use crate::api_server::*;
use crate::commands::lima;
use crate::routes::payloads::*;

pub async fn api_lima_list() -> (StatusCode, Json<ApiResponse<Vec<lima::LimaInstance>>>) {
    match lima::lima_list().await {
        Ok(list) => ok(list),
        Err(e) => err(e.to_string()),
    }
}


pub async fn api_lima_start(Json(body): Json<LimaNameBody>) -> (StatusCode, Json<ApiResponse<String>>) {
    match lima::lima_start(body.name).await {
        Ok(out) => ok(out),
        Err(e) => err(e.to_string()),
    }
}


pub async fn api_lima_stop(Json(body): Json<LimaNameBody>) -> (StatusCode, Json<ApiResponse<String>>) {
    match lima::lima_stop(body.name).await {
        Ok(out) => ok(out),
        Err(e) => err(e.to_string()),
    }
}


pub async fn api_lima_delete(
    Json(body): Json<LimaDeleteBody>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    match lima::lima_delete(body.name, body.force).await {
        Ok(out) => ok(out),
        Err(e) => err(e.to_string()),
    }
}


pub async fn api_lima_info() -> (StatusCode, Json<ApiResponse<String>>) {
    match lima::lima_info("".to_string()).await {
        Ok(out) => ok(out),
        Err(e) => err(e.to_string()),
    }
}


pub async fn api_lima_shell(
    Json(body): Json<LimaShellBody>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    // Security validation now happens inside command function
    match lima::lima_shell(body.name, body.command).await {
        Ok(out) => ok(out),
        Err(e) => err(e.to_string()),
    }
}


pub async fn api_lima_templates() -> (StatusCode, Json<ApiResponse<String>>) {
    match lima::lima_templates().await {
        Ok(out) => ok(out),
        Err(e) => err(e.to_string()),
    }
}


pub async fn api_lima_create(
    Json(body): Json<LimaCreateBody>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    match lima::lima_create(body.name, body.template, body.cpus, body.memory, body.disk).await {
        Ok(out) => ok(out),
        Err(e) => err(e.to_string()),
    }
}
