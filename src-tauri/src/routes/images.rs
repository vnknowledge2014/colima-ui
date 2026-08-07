use axum::{
    extract::Query,
    http::StatusCode,
    response::Json,
};
use crate::api_server::*;
use crate::commands::containers;
use crate::routes::payloads::*;

pub async fn api_list_images() -> (StatusCode, Json<ApiResponse<Vec<crate::commands::containers::DockerImage>>>) {
    match run_blocking(|| {
        let output = crate::commands::runtime::get_runtime_cmd()
            .args(["images", "--format", "json"])
            .output()
            .map_err(|e| format!("Failed to list images: {}", e))?;

        if !output.status.success() {
            return Err(format!(
                "docker images failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.trim().is_empty() {
            return Ok(vec![]);
        }

        Ok(stdout
            .lines()
            .filter(|l| !l.trim().is_empty())
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect())
    })
    .await
    {
        Ok(list) => ok(list),
        Err(e) => err(e),
    }
}


pub async fn api_remove_image(
    Query(q): Query<ImageIdQuery>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    let force = q.force.unwrap_or(false);
    match containers::remove_image(q.image_id, force).await {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}


pub async fn api_pull_image(
    Query(q): Query<ImagePullQuery>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    match containers::pull_image(q.image_name).await {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}


pub async fn api_prune_images() -> (StatusCode, Json<ApiResponse<String>>) {
    match containers::prune_images().await {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}


pub async fn api_inspect_image(
    Query(q): Query<ImageIdQuery>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    match containers::inspect_image(q.image_id).await {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}


pub async fn api_tag_image(Json(body): Json<ImageTagBody>) -> (StatusCode, Json<ApiResponse<String>>) {
    match containers::tag_image(body.source, body.target).await {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}
