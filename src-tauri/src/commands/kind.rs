//! kind (Kubernetes IN Docker) cluster management.
//!
//! These existed only as HTTP routes (`/api/kind/*`). The frontend calls them
//! through `call()`, which invokes a Tauri command in the desktop app and falls
//! back to HTTP in browser mode — so with no command registered, every kind
//! action worked in the browser and failed in the app with
//! "Command kind_list not found".
//!
//! `run_cmd` is used rather than a bare `Command` because it sets `DOCKER_HOST`
//! for `kind`, which needs to reach the same daemon the rest of the app talks
//! to. A kind cluster created against a different daemon would be invisible
//! everywhere else in the UI.

use crate::error::ColimaError;
use crate::helpers::{run_blocking, run_cmd};
use crate::validation::is_valid_k8s_name;

/// Names reach `kind --name`, so they are argv elements rather than shell
/// input. The guard is here to reject the shapes kind itself would refuse,
/// and to keep the error in the UI instead of in kind's stderr.
fn ensure_valid_cluster_name(name: &str) -> Result<(), String> {
    if !is_valid_k8s_name(name) {
        return Err(format!("Invalid cluster name: {name}"));
    }
    Ok(())
}

#[tauri::command]
pub async fn kind_list() -> Result<String, ColimaError> {
    run_blocking(|| run_cmd("kind", &["get", "clusters"]))
        .await
        .map_err(ColimaError::from)
}

#[tauri::command]
pub async fn kind_create(name: String, image: String) -> Result<String, ColimaError> {
    async move {
        ensure_valid_cluster_name(&name)?;
        // The node image is a container reference, not a k8s name, so it is
        // checked for emptiness only — the same as the HTTP route.
        run_blocking(move || {
            let mut args = vec!["create", "cluster", "--name", &name];
            if !image.is_empty() {
                args.push("--image");
                args.push(&image);
            }
            run_cmd("kind", &args)
        })
        .await
    }
    .await
    .map_err(ColimaError::from)
}

#[tauri::command]
pub async fn kind_delete(name: String) -> Result<String, ColimaError> {
    async move {
        ensure_valid_cluster_name(&name)?;
        run_blocking(move || run_cmd("kind", &["delete", "cluster", "--name", &name])).await
    }
    .await
    .map_err(ColimaError::from)
}
