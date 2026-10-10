use axum::{
    extract::Query,
    http::StatusCode,
    response::Json,
};
use crate::api_server::*;
use crate::commands::models;
use crate::routes::payloads::*;

pub async fn api_list_models(
    Query(q): Query<ModelQuery>,
) -> (StatusCode, Json<ApiResponse<Vec<models::AiModel>>>) {
    let runner = if q.runner.is_empty() { None } else { Some(q.runner) };
    match models::list_models(q.profile, runner).await {
        Ok(list) => ok(list),
        Err(e) => err(e.to_string()),
    }
}


pub async fn api_pull_model(Query(q): Query<ModelQuery>) -> (StatusCode, Json<ApiResponse<String>>) {
    let runner = if q.runner.is_empty() { None } else { Some(q.runner) };
    match models::pull_model(q.profile, q.model_name, runner).await {
        Ok(msg) => ok(msg),
        Err(e) => err(e.to_string()),
    }
}


pub async fn api_serve_model(Query(q): Query<ModelQuery>) -> (StatusCode, Json<ApiResponse<String>>) {
    let runner = if q.runner.is_empty() { None } else { Some(q.runner) };
    match models::serve_model(q.profile, q.model_name, q.port, runner).await {
        Ok(msg) => ok(msg),
        Err(e) => err(e.to_string()),
    }
}


pub async fn api_delete_model(Query(q): Query<ModelQuery>) -> (StatusCode, Json<ApiResponse<String>>) {
    let runner = if q.runner.is_empty() { None } else { Some(q.runner) };
    match models::delete_model(q.profile, q.model_name, runner).await {
        Ok(msg) => ok(msg),
        Err(e) => err(e.to_string()),
    }
}
