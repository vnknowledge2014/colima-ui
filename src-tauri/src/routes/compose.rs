use axum::{
    extract::Query,
    http::StatusCode,
    response::Json,
};
use crate::api_server::*;
use crate::commands::compose;
use crate::routes::payloads::*;

pub async fn api_list_compose() -> (StatusCode, Json<ApiResponse<Vec<compose::ComposeProject>>>) {
    match compose::list_compose_projects().await {
        Ok(out) => ok(out),
        Err(e) => err(e.to_string()),
    }
}


pub async fn api_compose_up(
    Json(body): Json<ComposeUpBody>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    match compose::compose_up(body.project_dir, body.detach).await {
        Ok(out) => ok(out),
        Err(e) => err(e.to_string()),
    }
}


pub async fn api_compose_down(
    Json(body): Json<ComposeProjectBody>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    match compose::compose_down(body.project_name).await {
        Ok(out) => ok(out),
        Err(e) => err(e.to_string()),
    }
}


pub async fn api_compose_restart(
    Json(body): Json<ComposeProjectBody>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    match compose::compose_restart(body.project_name).await {
        Ok(out) => ok(out),
        Err(e) => err(e.to_string()),
    }
}


pub async fn api_compose_logs(
    Query(q): Query<ComposeLogsQuery>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    match compose::compose_logs(q.project_name, q.lines).await {
        Ok(out) => ok(out),
        Err(e) => err(e.to_string()),
    }
}


pub async fn api_compose_ps(
    Query(q): Query<ComposePsQuery>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    match compose::compose_ps(q.project_name).await {
        Ok(out) => ok(out),
        Err(e) => err(e.to_string()),
    }
}
