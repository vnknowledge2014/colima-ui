use axum::{
    extract::Query,
    http::StatusCode,
    response::Json,
};
use crate::api_server::*;
use crate::commands::networks;
use crate::routes::payloads::*;

pub async fn api_list_networks() -> (StatusCode, Json<ApiResponse<Vec<networks::DockerNetwork>>>) {
    match networks::list_networks().await {
        Ok(list) => ok(list),
        Err(e) => err(e.to_string()),
    }
}


pub async fn api_create_network(
    Json(body): Json<CreateNetworkBody>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    match networks::create_network(body.name, body.driver, body.subnet).await {
        Ok(out) => ok(out),
        Err(e) => err(e.to_string()),
    }
}


pub async fn api_remove_network(
    Query(q): Query<NetworkNameQuery>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    match networks::remove_network(q.name).await {
        Ok(out) => ok(out),
        Err(e) => err(e.to_string()),
    }
}


pub async fn api_inspect_network(
    Query(q): Query<NetworkNameQuery>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    match networks::inspect_network(q.name).await {
        Ok(out) => ok(out),
        Err(e) => err(e.to_string()),
    }
}


pub async fn api_prune_networks() -> (StatusCode, Json<ApiResponse<String>>) {
    match networks::prune_networks().await {
        Ok(out) => ok(out),
        Err(e) => err(e.to_string()),
    }
}
