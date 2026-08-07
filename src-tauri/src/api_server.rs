//! HTTP API server for browser-mode access.
//!
//! This module provides:
//! - `build_router()` — constructs the Axum router with all API routes
//! - `start_api_server()` — starts the HTTP server on port 11420
//!
//! All supporting functionality is extracted into dedicated modules:
//! - `auth` — token generation and middleware
//! - `sse` — SSE broadcast, Docker watcher, instance publisher
//! - `validation` — input sanitization and security checks
//! - `platform` — OS/arch/package manager detection
//! - `helpers` — API response wrappers, CLI runner, caching

use axum::{
    http::HeaderValue,
    middleware,
    routing::{get, post},
    Router,
};
use tower_http::cors::CorsLayer;

// Re-export everything from sub-modules for backward compatibility.
// Existing `use crate::api_server::*;` in routes/ will still work.
pub use crate::auth::*;
pub use crate::sse::*;
pub use crate::validation::*;
pub use crate::platform::*;
pub use crate::helpers::*;

use crate::terminal_session;

// ===== Route Handler Imports =====

use crate::routes::system::*;
use crate::routes::instances::*;
use crate::routes::containers::*;
use crate::routes::images::*;
use crate::routes::volumes::*;
use crate::routes::networks::*;
use crate::routes::compose::*;
use crate::routes::payloads::*;
use crate::routes::capabilities::*;
use crate::routes::models::*;
use crate::routes::ws::*;
use crate::routes::k8s::*;
use crate::routes::lima::*;
use crate::routes::ai::*;
use crate::routes::kb::*;
use crate::routes::misc::*;

/// Build the axum router with all API routes
pub fn build_router() -> Router {
    // Restrict CORS to localhost origins only (prevents CSRF from arbitrary websites)
    let cors = CorsLayer::new()
        .allow_origin([
            "http://127.0.0.1:1420".parse::<HeaderValue>().unwrap(),
            "http://localhost:1420".parse::<HeaderValue>().unwrap(),
            "http://127.0.0.1:11420".parse::<HeaderValue>().unwrap(),
            "http://localhost:11420".parse::<HeaderValue>().unwrap(),
            "tauri://localhost".parse::<HeaderValue>().unwrap(),
        ])
        .allow_methods(tower_http::cors::Any)
        .allow_headers(tower_http::cors::Any);

    let protected_routes = Router::new()
        // Headless CLI
        .route("/api/cli/chat", post(api_cli_chat))
        // SSE stream for browser mode
        .route("/api/events", get(api_events))
        // System
        .route("/api/system/check", get(api_check_system))
        .route("/api/system/version", get(api_get_version))
        .route("/api/system/homebrew", get(api_check_homebrew))
        .route("/api/system/check-tool", get(api_check_tool))
        .route("/api/system/platform", get(api_get_platform))
        .route("/api/system/host-specs", get(api_host_specs))
        .route("/api/system/install", post(api_install_dep))
        // Colima instances
        .route("/api/instances", get(api_list_instances))
        .route("/api/instances/start", post(api_start_instance))
        .route("/api/instances/stop", post(api_stop_instance))
        .route("/api/instances/delete", post(api_delete_instance))
        .route("/api/instances/status", get(api_instance_status))
        .route("/api/instances/ssh", get(api_ssh_command))
        .route("/api/instances/k8s", post(api_k8s_action))
        // Docker containers
        .route("/api/containers", get(api_list_containers))
        .route("/api/containers/start", post(api_start_container))
        .route("/api/containers/stop", post(api_stop_container))
        .route("/api/containers/restart", post(api_restart_container))
        .route("/api/containers/remove", post(api_remove_container))
        .route("/api/containers/logs", get(api_container_logs))
        .route("/api/containers/inspect", get(api_inspect_container))
        .route("/api/containers/stats", get(api_container_stats))
        .route("/api/containers/stats/all", get(api_all_container_stats))
        .route("/api/containers/top", get(api_container_top))
        .route("/api/containers/exec", post(api_container_exec))
        .route("/api/containers/run", post(api_run_container))
        .route("/api/containers/rename", post(api_rename_container))
        .route("/api/containers/pause", post(api_pause_container))
        .route("/api/containers/unpause", post(api_unpause_container))
        .route("/api/images", get(api_list_images))
        .route("/api/images/remove", post(api_remove_image))
        .route("/api/images/pull", post(api_pull_image))
        .route("/api/images/prune", post(api_prune_images))
        .route("/api/images/inspect", get(api_inspect_image))
        .route("/api/images/tag", post(api_tag_image))
        // Docker volumes
        .route("/api/volumes", get(api_list_volumes))
        .route("/api/volumes/create", post(api_create_volume))
        .route("/api/volumes/remove", post(api_remove_volume))
        .route("/api/volumes/prune", post(api_prune_volumes))
        .route("/api/volumes/inspect", get(api_inspect_volume))
        // Docker networks
        .route("/api/networks", get(api_list_networks))
        .route("/api/networks/create", post(api_create_network))
        .route("/api/networks/remove", post(api_remove_network))
        .route("/api/networks/inspect", get(api_inspect_network))
        .route("/api/networks/prune", post(api_prune_networks))
        // System
        .route("/api/system/prune", post(api_system_prune))
        .route("/api/system/df", get(api_system_df))
        // Models
        .route("/api/models", get(api_list_models))
        .route("/api/models/pull", post(api_pull_model))
        .route("/api/models/serve", post(api_serve_model))
        .route("/api/models/delete", post(api_delete_model))
        // Compose
        .route("/api/compose", get(api_list_compose))
        .route("/api/compose/up", post(api_compose_up))
        .route("/api/compose/down", post(api_compose_down))
        .route("/api/compose/restart", post(api_compose_restart))
        .route("/api/compose/logs", get(api_compose_logs))
        .route("/api/compose/ps", get(api_compose_ps))
        // Kubernetes
        .route("/api/k8s/check", get(api_k8s_check))
        .route("/api/k8s/namespaces", get(api_k8s_namespaces))
        .route("/api/k8s/pods", get(api_k8s_pods))
        .route("/api/k8s/services", get(api_k8s_services))
        .route("/api/k8s/deployments", get(api_k8s_deployments))
        .route("/api/k8s/pods/logs", get(api_k8s_pod_logs))
        .route("/api/k8s/pods/delete", post(api_k8s_delete_pod))
        .route("/api/k8s/describe", get(api_k8s_describe))
        .route("/api/k8s/scale", post(api_k8s_scale))
        .route("/api/k8s/nodes", get(api_k8s_nodes))
        .route("/api/k8s/events", get(api_k8s_events))
        .route("/api/k8s/resources", get(api_k8s_resources))
        .route("/api/k8s/resources/delete", post(api_k8s_delete_resource))
        .route("/api/k8s/resources/restart", post(api_k8s_restart))
        .route("/api/k8s/resources/yaml", get(api_k8s_yaml))
        .route("/api/k8s/nodes/json", get(api_k8s_nodes_json))
        .route("/api/k8s/events/json", get(api_k8s_events_json))
        .route("/api/k8s/contexts", get(api_k8s_contexts))
        .route("/api/k8s/contexts/current", get(api_k8s_current_context))
        .route("/api/k8s/contexts/set", post(api_k8s_set_context))
        // K8s Phase 2
        .route("/api/k8s/apply", post(api_k8s_apply))
        .route(
            "/api/k8s/port-forward/start",
            post(api_k8s_port_forward_start),
        )
        .route(
            "/api/k8s/port-forward/stop",
            post(api_k8s_port_forward_stop),
        )
        .route("/api/k8s/port-forward/list", get(api_k8s_port_forward_list))
        .route("/api/k8s/exec", post(api_k8s_exec))
        .route("/api/k8s/pods/containers", get(api_k8s_pod_containers))
        .route("/api/k8s/pods/container-logs", get(api_k8s_container_logs))
        .route("/api/k8s/nodes/action", post(api_k8s_node_action))
        // Kind
        .route("/api/kind", get(api_kind_list))
        .route("/api/kind/create", post(api_kind_create))
        .route("/api/kind/delete", post(api_kind_delete))
        // K8s Phase 3
        .route("/api/k8s/scale-generic", post(api_k8s_generic_scale))
        .route("/api/k8s/cluster-health", get(api_k8s_cluster_health))
        // CRDs
        .route("/api/k8s/crds", get(api_k8s_crds))
        .route("/api/k8s/crds/resources", get(api_k8s_crd_resources))
        // Log streaming
        .route("/api/k8s/pods/logs/stream", get(api_k8s_log_stream))
        // Benchmark
        .route("/api/k8s/benchmark", post(api_k8s_benchmark))
        // Lima
        .route("/api/lima", get(api_lima_list))
        .route("/api/lima/start", post(api_lima_start))
        .route("/api/lima/stop", post(api_lima_stop))
        .route("/api/lima/delete", post(api_lima_delete))
        .route("/api/lima/info", get(api_lima_info))
        .route("/api/lima/shell", post(api_lima_shell))
        .route("/api/lima/templates", get(api_lima_templates))
        .route("/api/lima/create", post(api_lima_create))
        // Docker System
        .route("/api/docker/df", get(api_docker_df))
        .route("/api/docker/prune", post(api_docker_prune))
        // AI Chat
        .route("/api/ai/chat", post(api_ai_chat))
        .route("/api/ai/models", post(api_ai_list_models))
        .route("/api/ai/search", post(api_ai_search))
        .route("/api/ai/fetch-page", post(api_ai_fetch_page))
        .route("/api/ai/context", get(api_ai_context))
        // Settings
        .route("/api/settings", get(api_get_settings))
        .route("/api/settings", post(api_set_setting))
        // Knowledge Bank
        .route("/api/kb/query", post(api_kb_query))
        .route("/api/kb/search", post(api_kb_search))
        .route("/api/kb/memories", get(api_kb_get_memories))
        .route("/api/kb/memories/update", post(api_kb_update_memory))
        .route("/api/kb/memories/delete", post(api_kb_delete_memory))
        // Shell Sandbox
        .route("/api/sandbox/execute", post(api_sandbox_execute))
        .route("/api/sandbox/execute-approved", post(api_sandbox_execute_approved))
        .route("/api/cli/execute_stream", post(api_sandbox_execute_stream))
        // Capabilities
        .route("/api/capabilities", get(api_capabilities))
        // Diagnostics
        .route("/api/diagnostics/logs", get(api_diagnostics_logs))
        // Terminal sessions (browser mode)
        .route("/api/terminal/create", post(api_terminal_create))
        .route("/api/terminal/write", post(api_terminal_write))
        .route("/api/terminal/read", get(api_terminal_read))
        .route("/api/terminal/close", post(api_terminal_close))
        .route("/api/terminal/resize", post(api_terminal_resize))
        .with_state(terminal_session::create_session_manager())
        .layer(middleware::from_fn(auth_middleware));

    // Unauthenticated routes — only token discovery (CORS still restricts to localhost)
    let public_routes = Router::new()
        .route("/api/auth/token", get(api_auth_token))
        .route("/api/health", get(api_health));

    // Merge: public routes have no auth, protected routes require Bearer token
    public_routes.merge(protected_routes).layer(cors)
}

/// Start the HTTP API server on port 11420 using Tauri's tokio runtime.
/// Never panics — if binding fails, the server simply won't start.
pub fn start_api_server() {
    // Initialize SSE broadcast channel eagerly
    let _ = get_sse_tx();

    tauri::async_runtime::spawn(async {
        let app = build_router();

        // Spawn Docker bollard watcher for SSE events
        tokio::spawn(sse_docker_watcher());
        // Spawn instance change publisher for SSE events
        tokio::spawn(sse_instance_publisher());

        // Fix #12: Try ports 11420-11429 as fallback when primary port is taken
        let mut listener_opt = None;
        let mut bound_port = 11420u16;
        'port_scan: for port in 11420..=11429 {
            for attempt in 0..3 {
                match tokio::net::TcpListener::bind(format!("127.0.0.1:{}", port)).await {
                    Ok(l) => {
                        listener_opt = Some(l);
                        bound_port = port;
                        break 'port_scan;
                    }
                    Err(e) => {
                        if attempt < 2 {
                            eprintln!(
                                "[API Server] Port {} attempt {}/3 failed: {} — retrying",
                                port,
                                attempt + 1,
                                e
                            );
                            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                        }
                    }
                }
            }
        }

        match listener_opt {
            Some(listener) => {
                println!("HTTP API server running on http://127.0.0.1:{}", bound_port);

                // Fix #14: Clean up any orphaned port-forward processes on shutdown
                let cleanup = async {
                    if let Err(e) = axum::serve(listener, app).await {
                        eprintln!("[API Server] Server error: {}", e);
                    }
                    // Server stopped — kill all tracked port-forwards
                    if let Ok(fwds) = PORT_FORWARDS.lock() {
                        for (_, pid) in fwds.iter() {
                            #[cfg(unix)]
                            unsafe {
                                libc::kill(*pid as i32, libc::SIGTERM);
                            }
                        }
                    }
                };
                cleanup.await;
            }
            None => {
                eprintln!(
                    "[API Server] Could not bind to any port 11420-11429 — API server disabled"
                );
            }
        }
    });
}
