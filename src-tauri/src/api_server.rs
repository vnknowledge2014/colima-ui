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

// ===== Route Handler Imports =====

use crate::routes::system::*;
use crate::routes::instances::*;
use crate::routes::containers::*;
use crate::routes::images::*;
use crate::routes::volumes::*;
use crate::routes::networks::*;
use crate::routes::topology::*;
use crate::routes::file_transfer::*;
use crate::routes::announcements::*;
use crate::routes::security::*;
use crate::routes::diagnostics::*;
use crate::routes::compose::*;
use crate::routes::capabilities::*;
use crate::routes::models::*;
use crate::routes::k8s::*;
use crate::routes::lima::*;
use crate::routes::ai::*;
use crate::routes::kb::*;
use crate::routes::misc::*;
use crate::routes::resolution::*;

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
        .route(
            "/api/system/capabilities",
            get(crate::routes::system_capabilities::api_system_capabilities),
        )
        .route("/api/system/platform", get(api_get_platform))
        .route("/api/system/host-specs", get(api_host_specs))
        .route("/api/system/engine-resources", get(api_engine_resources))
        .route("/api/system/install", post(api_install_dep))
        .route(
            "/api/system/autostart",
            get(api_get_autostart_status).post(api_configure_autostart),
        )
        .route("/api/system/resource-saver", post(api_set_resource_saver))
        .route("/api/system/runtime", get(api_get_runtime_info))
        // Colima instances
        .route("/api/instances", get(api_list_instances))
        .route("/api/instances/start", post(api_start_instance))
        .route("/api/instances/stop", post(api_stop_instance))
        .route("/api/instances/delete", post(api_delete_instance))
        .route("/api/instances/status", get(api_instance_status))
        .route("/api/instances/ssh", get(api_ssh_command))
        .route("/api/instances/k8s", post(api_k8s_action))
        .route("/api/instances/create-worker", post(api_create_worker_node))
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
        // Docker topology graph
        .route("/api/topology", get(api_topology))
        // Background transfers. Progress arrives on the SSE stream, not in the
        // response: these return a job id immediately and run in the background.
        .route("/api/images/save", post(api_image_save))
        .route("/api/images/load", post(api_image_load))
        .route("/api/containers/cp/to", post(api_copy_to_container))
        .route("/api/containers/cp/from", post(api_copy_from_container))
        .route("/api/transfers/cancel", post(api_cancel_transfer))
        .route("/api/transfers", get(api_transfer_list))
        .route("/api/announcements", get(api_announcements))
        // Image scanning. Long-running like a transfer, but the result is the
        // point, so the response carries it; progress arrives over SSE meanwhile.
        .route("/api/security/scan", post(api_security_scan))
        .route("/api/security/scan/cancel", post(api_security_scan_cancel))
        .route("/api/security/sbom", post(api_security_sbom))
        .route("/api/security/audit", post(api_security_audit))
        .route("/api/security/triage", post(api_security_triage))
        .route("/api/security/autofix/propose", post(api_security_autofix_propose))
        .route("/api/security/history", get(api_security_history))
        .route("/api/security/policy", get(api_security_policy_list).post(api_security_policy_save))
        .route("/api/security/policy/delete", post(api_security_policy_delete))
        .route("/api/security/watch", get(api_security_watch_state).post(api_security_watch_set))
        .route("/api/security/rules", get(api_security_rule_pack))
        .route("/api/security/alternatives", get(api_security_alternatives))
        // Detonation. Start returns a session id immediately; the timeline
        // arrives on SSE, because the interesting part is what happens while
        // the sample runs rather than the value at the end.
        .route("/api/detonation/start", post(crate::routes::detonation::api_detonation_start))
        .route("/api/detonation/cancel", post(crate::routes::detonation::api_detonation_cancel))
        .route("/api/detonation/session", post(crate::routes::detonation::api_detonation_get))
        .route("/api/detonation/list", get(crate::routes::detonation::api_detonation_list))
        .route("/api/detonation/export", post(crate::routes::detonation::api_detonation_export))
        // Runtime events. Falco does the detecting; these only read what it
        // wrote and say whether it is in a state where it can detect at all.
        .route("/api/security/falco/state", get(api_falco_state))
        .route("/api/security/falco/events", get(api_falco_events))
        .route("/api/security/falco/watch", get(api_falco_watch_state).post(api_falco_watch))
        .route("/api/security/falco/explain", post(api_falco_explain))
        // Live metrics: samples arrive on the SSE stream under the
        // `metrics.sample` topic; this only tunes the sampling period.
        .route("/api/metrics/interval", post(api_set_metrics_interval))
        .route("/api/metrics/history", get(crate::routes::metrics::api_metrics_history))
        .route("/api/metrics/health", get(crate::routes::metrics::api_metrics_health))
        // Alert rules are the user's own configuration, so these write.
        .route("/api/alerts/rules", get(crate::routes::metrics::api_alerts_list_rules).post(crate::routes::metrics::api_alerts_save_rule))
        .route("/api/alerts/rules/delete", post(crate::routes::metrics::api_alerts_delete_rule))
        .route("/api/alerts/events", get(crate::routes::metrics::api_alerts_events))
        .route("/api/alerts/backtest", post(crate::routes::metrics::api_alerts_backtest))
        .route("/api/self-heal/rules", get(crate::routes::self_heal::api_self_heal_list_rules).post(crate::routes::self_heal::api_self_heal_save_rule))
        .route("/api/self-heal/log", get(crate::routes::self_heal::api_self_heal_log))
        .route("/api/self-heal/enabled", get(crate::routes::self_heal::api_self_heal_enabled).post(crate::routes::self_heal::api_self_heal_set_enabled))
        .route("/api/activity", get(crate::routes::activity::api_activity_query))
        .route("/api/activity/feed", get(crate::routes::activity::api_activity_feed))
        .route("/api/activity/export", post(crate::routes::activity::api_activity_export))
        // Diagnostics. Building a bundle sends nothing anywhere; the response
        // goes back to the caller, who decides what to do with it.
        .route("/api/diagnostics/bundle", post(api_diagnostic_bundle))
        .route("/api/diagnostics/save", post(api_save_diagnostic_bundle))
        // System
        .route("/api/system/prune", post(api_system_prune))
        .route("/api/system/df", get(api_system_df))
        .route("/api/system/reference", get(api_system_reference))
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
        .route("/api/compose/stop", post(api_compose_stop))
        .route("/api/compose/start", post(api_compose_start))
        .route("/api/compose/logs", get(api_compose_logs))
        .route("/api/compose/ps", get(api_compose_ps))
        .route("/api/compose/diagnose", post(api_compose_diagnose))
        .route("/api/compose/autofix/propose", post(api_compose_autofix_propose))
        .route("/api/compose/autofix/apply", post(api_compose_autofix_apply))
        .route("/api/compose/autofix/undo", post(api_compose_autofix_undo))
        .route("/api/compose/autofix/history", get(api_compose_autofix_history))
        .route("/api/pro/status", get(crate::pro::api_pro_status))
        .route("/api/subscription", get(crate::subscription::api_subscription_state))
        .route("/api/subscription/store", post(crate::subscription::api_subscription_store))
        .route(
            "/api/telemetry/consent",
            get(crate::telemetry::api_telemetry_consent).post(crate::telemetry::api_telemetry_set_consent),
        )
        .route("/api/telemetry/should-prompt", get(crate::telemetry::api_telemetry_should_prompt))
        .route("/api/telemetry/preview", get(crate::telemetry::api_telemetry_preview))
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
        .route("/api/ai/reference", get(api_ai_reference))
        .route("/api/ai/history", get(api_ai_load_history).post(api_ai_save_message))
        .route("/api/ai/history/clear", post(api_ai_clear_history))
        .route("/api/ai/conversations", get(api_ai_list_conversations).post(api_ai_create_conversation))
        .route("/api/ai/conversations/rename", post(api_ai_rename_conversation))
        .route("/api/ai/conversations/delete", post(api_ai_delete_conversation))
        // Environment resolution record (see commands/resolution.rs)
        .route("/api/environment/resolutions", get(api_get_environment_resolutions))
        // Settings & Presets
        .route("/api/settings", get(api_get_settings))
        .route("/api/settings", post(api_set_setting))
        .route(
            "/api/presets/snapshot",
            get(api_load_preset_snapshot).post(api_save_preset_snapshot),
        )
        .route("/api/presets/snapshots", get(api_list_preset_snapshots))
        .route("/api/presets", get(api_get_presets).post(api_save_preset))
        .route("/api/presets/delete", post(api_delete_preset))
        // Colima config — colima.yaml is the single source of truth
        .route("/api/instances/config", get(crate::routes::colima_config::api_get_colima_config))
        .route("/api/instances/config/preview", post(crate::routes::colima_config::api_preview_colima_config))
        .route("/api/instances/config/apply", post(crate::routes::colima_config::api_apply_colima_config))
        // Help articles
        .route("/api/kb/articles", get(crate::routes::colima_config::api_kb_list_articles))
        .route("/api/kb/articles/get", get(crate::routes::colima_config::api_kb_get_article))
        .route("/api/kb/articles/search", get(crate::routes::colima_config::api_kb_search_articles))
        // Knowledge Bank
        .route("/api/kb/query", post(api_kb_query))
        .route("/api/kb/feedback", post(api_kb_feedback))
        .route("/api/kb/learn", post(api_kb_learn))
        .route("/api/kb/search", post(api_kb_search))
        .route("/api/kb/memories", get(api_kb_get_memories))
        .route("/api/kb/memories/add", post(api_kb_add_memory))
        .route("/api/kb/memories/update", post(api_kb_update_memory))
        .route("/api/kb/memories/delete", post(api_kb_delete_memory))
        // Shell Sandbox
        .route("/api/sandbox/execute", post(api_sandbox_execute))
        .route("/api/sandbox/execute-approved", post(api_sandbox_execute_approved))
        // Capabilities
        .route("/api/capabilities", get(api_capabilities))
        // Diagnostics
        .route("/api/diagnostics/logs", get(api_diagnostics_logs))
        // UI Remote Control Bridge
        .route("/api/ui/navigate", post(crate::routes::ui::api_ui_navigate))
        .route("/api/ui/state", get(crate::routes::ui::api_ui_state))
        // Native A2A v0.2.1 Protocol
        .route("/.well-known/agent.json", get(crate::routes::a2a::api_a2a_agent_card))
        .route("/a2a", post(crate::routes::a2a::api_a2a_rpc))
        // Terminal sessions used to live here, over HTTP. They are Tauri
        // commands now: a shell endpoint reachable on a local port is arbitrary
        // code execution, and the safest version of that endpoint is one that
        // does not exist. See plans/260811-0919-terminal-integration.
        .layer(middleware::from_fn(auth_middleware));

    // Unauthenticated routes. `/api/health` reports liveness and nothing else,
    // so it is safe to leave open — the client port-scans 11420-11429 with it
    // before it has any credential.
    //
    // `/api/auth/token` used to live here and no longer does. CORS restricts
    // browsers, not local processes, so an open token endpoint handed the whole
    // API to anything running on the machine. Clients now get the token through
    // `auth::api_token` (IPC), `~/.colima-ui/api_token` (local CLI/agents, mode
    // 0600), or a `#token=…` URL fragment in dev builds; see `auth`.
    let public_routes = Router::new().route("/api/health", get(api_health));

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
        // TODO(port-shadowing): a second instance lands on 11421+ here. `api_port`
        // now records `bound_port` in `~/.colima-ui/api_port` so `cui` and external
        // host agents probe the active instance first before scanning 11420-11429,
        // though a newer instance still stays on `bound_port` if the 11420 holder
        // later exits. Consider notifying the user when `bound_port != 11420`.
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
                if let Err(e) = crate::auth::persist_session_credentials(bound_port) {
                    eprintln!(
                        "[API Server] Warning: could not persist session credentials: {}",
                        e
                    );
                }

                // Browser mode needs the token out-of-band now that the public
                // endpoint is gone. In a dev build, print the ready-made URL:
                // the developer running `pnpm tauri dev` is the only person who
                // can see this terminal, and the vite server they need is the
                // one on 1420.
                //
                // Debug-only on purpose. A release build serves no frontend of
                // its own (there is no ServeDir anywhere in this router), so it
                // has no browser mode to bootstrap, and printing a live
                // credential to stdout in production would just be a new leak.
                #[cfg(debug_assertions)]
                println!(
                    "Browser mode: http://localhost:1420/#token={}",
                    get_api_token()
                );

                // Fix #14: Clean up any orphaned port-forward processes on shutdown
                let cleanup = async {
                    if let Err(e) = axum::serve(listener, app).await {
                        eprintln!("[API Server] Server error: {}", e);
                    }
                    // Server stopped — kill all tracked port-forwards
                    if let Ok(fwds) = PORT_FORWARDS.lock() {
                        for pid in fwds.values() {
                            #[cfg(unix)]
                            unsafe {
                                libc::kill(*pid as i32, libc::SIGTERM);
                            }
                        }
                    }
                    crate::auth::cleanup_session_credentials();
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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    async fn status_of(uri: &str, auth: Option<&str>) -> StatusCode {
        let mut req = Request::builder().uri(uri);
        if let Some(value) = auth {
            req = req.header("authorization", value);
        }
        build_router()
            .oneshot(req.body(Body::empty()).expect("request is well-formed"))
            .await
            .expect("router is infallible")
            .status()
    }

    /// The whole point of the change this guards: no route hands out the token.
    ///
    /// `GET /api/auth/token` sat in the public router and returned the bearer
    /// token to any caller, protected only by CORS — which constrains browsers
    /// and not the local processes that were the actual threat. Re-adding it,
    /// or anything like it, must fail here rather than in a security review a
    /// year from now.
    ///
    /// The status is 401 rather than 404: `Router::layer` wraps unmatched paths
    /// too, so anything outside the public router meets the auth middleware
    /// first. That is a small bonus — an unauthenticated caller cannot map the
    /// route table by probing — but the assertion deliberately checks "never
    /// 200" rather than a specific rejection code, so a future axum that
    /// answers 404 here does not fail a test about credentials.
    #[tokio::test]
    async fn no_route_hands_out_the_api_token() {
        let status = status_of("/api/auth/token", None).await;
        assert!(
            status.is_client_error(),
            "unauthenticated /api/auth/token must be rejected, got {status}"
        );
        assert_ne!(status, StatusCode::OK, "the token endpoint is back");
    }

    /// Health has to stay open: the client port-scans 11420-11429 with it to
    /// find the server, and it necessarily does that before it has a token.
    #[tokio::test]
    async fn health_stays_public() {
        assert_eq!(status_of("/api/health", None).await, StatusCode::OK);
    }

    /// `/api/events` authenticates by `?token=` because `EventSource` cannot
    /// send headers. Adding a second query parameter must not disturb that —
    /// the token is found by parsing the query, not by matching the whole
    /// string, and this pins that down now that clients pass `?topics=` too.
    #[tokio::test]
    async fn the_event_stream_authenticates_alongside_other_query_parameters() {
        let token = get_api_token();

        let with_topics = format!("/api/events?token={token}&topics=metrics.sample");
        assert_eq!(status_of(&with_topics, None).await, StatusCode::OK);

        // Order must not matter either.
        let token_last = format!("/api/events?topics=metrics.sample&token={token}");
        assert_eq!(status_of(&token_last, None).await, StatusCode::OK);

        // And the topics parameter must not become a way in without a token.
        assert_eq!(
            status_of("/api/events?topics=metrics.sample", None).await,
            StatusCode::UNAUTHORIZED
        );
    }

    /// The property the subscriber registry rests on: a subscription is counted
    /// for exactly as long as the client holds the stream, and stops being
    /// counted when the client goes away without saying so.
    ///
    /// Asserted through the real router rather than by calling `subscribe_topics`
    /// directly, because the risk is not in that function — it is in whether the
    /// guard actually reaches the HTTP response body and dies with it. A unit
    /// test of the registry would pass even if the handler dropped the guard the
    /// moment it built the stream.
    #[tokio::test]
    async fn a_stream_is_counted_for_as_long_as_the_client_holds_it() {
        let topic = "test.router.counted";
        let token = get_api_token();
        assert_eq!(subscriber_count(topic), 0, "a previous test leaked a count");

        let res = build_router()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/events?token={token}&topics={topic}"))
                    .body(Body::empty())
                    .expect("request is well-formed"),
            )
            .await
            .expect("router is infallible");

        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(subscriber_count(topic), 1, "the open stream is not counted");

        // Exactly what a closed tab looks like from this side: the body is
        // dropped, and nobody told us.
        drop(res);
        assert_eq!(
            subscriber_count(topic), 0,
            "the count outlived the client — this is the leak the guard prevents"
        );
    }

    /// The other half of the contract — everything else is behind the token.
    /// Without this, deleting the auth layer would still pass the test above.
    #[tokio::test]
    async fn everything_else_requires_a_token() {
        assert_eq!(status_of("/api/containers", None).await, StatusCode::UNAUTHORIZED);
        assert_eq!(
            status_of("/api/containers", Some("Bearer not-the-real-token")).await,
            StatusCode::UNAUTHORIZED
        );
    }

    #[tokio::test]
    async fn event_stream_also_accepts_bearer_header_auth() {
        let bearer = format!("Bearer {}", get_api_token());
        assert_eq!(status_of("/api/events", Some(&bearer)).await, StatusCode::OK);
    }

    #[tokio::test]
    async fn parity_routes_are_mounted_and_accept_expected_shapes() {
        let bearer = format!("Bearer {}", get_api_token());

        // GET /api/system/runtime
        assert_eq!(
            status_of("/api/system/runtime", Some(&bearer)).await,
            StatusCode::OK
        );

        // GET /api/system/check-tool?name=qemu-img (allowlisted — must return 200, not 500 Unknown tool)
        assert_eq!(
            status_of("/api/system/check-tool?name=qemu-img", Some(&bearer)).await,
            StatusCode::OK
        );

        // POST /api/system/resource-saver
        let res = build_router()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/system/resource-saver")
                    .header("authorization", &bearer)
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"enabled":false,"idle_minutes":20}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);

        // POST /api/containers/unpause accepts both query and JSON body without 404/415/422
        let unpause_q = build_router()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/containers/unpause")
                    .header("authorization", &bearer)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_ne!(unpause_q.status(), StatusCode::NOT_FOUND);
        assert_ne!(unpause_q.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);

        let unpause_json = build_router()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/containers/unpause")
                    .header("authorization", &bearer)
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"id":""}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_ne!(unpause_json.status(), StatusCode::NOT_FOUND);
        assert_ne!(unpause_json.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    async fn a2a_ui_and_reference_routes_enforce_bearer_auth_and_contracts() {
        use axum::body::to_bytes;

        // 1. `/api/health` remains public while all new A2A, UI, and AI reference routes require auth.
        assert_eq!(status_of("/api/health", None).await, StatusCode::OK);
        assert_eq!(
            status_of("/.well-known/agent.json", None).await,
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            status_of("/api/ui/state", None).await,
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            status_of("/api/ai/reference?path=router.md", None).await,
            StatusCode::UNAUTHORIZED
        );

        let unauth_a2a = build_router()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/a2a")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"jsonrpc":"2.0","id":1,"method":"tasks/get","params":{"id":"x"}}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(unauth_a2a.status(), StatusCode::UNAUTHORIZED);

        let unauth_ui_nav = build_router()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/ui/navigate")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"page":"containers"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(unauth_ui_nav.status(), StatusCode::UNAUTHORIZED);

        let bearer = format!("Bearer {}", get_api_token());

        // 2. Agent Card (`GET /.well-known/agent.json`) with valid Bearer token
        let card_res = build_router()
            .oneshot(
                Request::builder()
                    .uri("/.well-known/agent.json")
                    .header("authorization", &bearer)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(card_res.status(), StatusCode::OK);
        let card_bytes = to_bytes(card_res.into_body(), 1024 * 1024).await.unwrap();
        let card_json: serde_json::Value = serde_json::from_slice(&card_bytes).unwrap();
        assert_eq!(card_json["protocolVersion"], "0.2.1");
        assert_eq!(card_json["authentication"]["schemes"][0], "Bearer");
        assert_eq!(card_json["skills"].as_array().map(Vec::len), Some(6));

        // 3. A2A JSON-RPC (`POST /a2a`): seed task -> `tasks/get` -> `tasks/cancel` -> unknown method
        let seeded_id = "router-test-a2a-task-1";
        crate::a2a::upsert_task(
            crate::a2a::A2aTask {
                id: seeded_id.to_string(),
                session_id: crate::commands::ai_chat::A2A_DEFAULT_CONVERSATION_ID.to_string(),
                status: crate::a2a::A2aTaskStatus {
                    state: "working".to_string(),
                    message: None,
                    timestamp: "2026-11-07T00:00:00Z".to_string(),
                },
                history: Vec::new(),
                artifacts: Vec::new(),
                state_transition_history: vec![crate::a2a::A2aStateTransition {
                    state: "working".to_string(),
                    timestamp: "2026-11-07T00:00:00Z".to_string(),
                }],
                metadata: None,
            },
            None,
        );

        let get_res = build_router()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/a2a")
                    .header("authorization", &bearer)
                    .header("content-type", "application/json")
                    .body(Body::from(format!(
                        r#"{{"jsonrpc":"2.0","id":10,"method":"tasks/get","params":{{"id":"{seeded_id}"}}}}"#
                    )))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(get_res.status(), StatusCode::OK);
        let get_json: serde_json::Value =
            serde_json::from_slice(&to_bytes(get_res.into_body(), 1024 * 1024).await.unwrap())
                .unwrap();
        assert_eq!(get_json["result"]["id"], seeded_id);
        assert_eq!(get_json["result"]["status"]["state"], "working");

        let cancel_res = build_router()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/a2a")
                    .header("authorization", &bearer)
                    .header("content-type", "application/json")
                    .body(Body::from(format!(
                        r#"{{"jsonrpc":"2.0","id":11,"method":"tasks/cancel","params":{{"id":"{seeded_id}"}}}}"#
                    )))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(cancel_res.status(), StatusCode::OK);
        let cancel_json: serde_json::Value =
            serde_json::from_slice(&to_bytes(cancel_res.into_body(), 1024 * 1024).await.unwrap())
                .unwrap();
        assert_eq!(cancel_json["result"]["status"]["state"], "canceled");

        let unknown_res = build_router()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/a2a")
                    .header("authorization", &bearer)
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"jsonrpc":"2.0","id":12,"method":"tasks/doesNotExist","params":{}}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(unknown_res.status(), StatusCode::OK);
        let unknown_json: serde_json::Value =
            serde_json::from_slice(&to_bytes(unknown_res.into_body(), 1024 * 1024).await.unwrap())
                .unwrap();
        assert_eq!(unknown_json["error"]["code"], -32601);

        // 4. UI Remote Control (`GET /api/ui/state` & `POST /api/ui/navigate`)
        let ui_state_res = build_router()
            .oneshot(
                Request::builder()
                    .uri("/api/ui/state")
                    .header("authorization", &bearer)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(ui_state_res.status(), StatusCode::OK);
        let ui_state_json: serde_json::Value = serde_json::from_slice(
            &to_bytes(ui_state_res.into_body(), 1024 * 1024)
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(
            ui_state_json["data"]["a2a_default_session_id"],
            crate::commands::ai_chat::A2A_DEFAULT_CONVERSATION_ID
        );

        let nav_ok = build_router()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/ui/navigate")
                    .header("authorization", &bearer)
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"page":"containers","section":"running","toast":"Switched to Containers"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(nav_ok.status(), StatusCode::OK);

        let nav_bad = build_router()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/ui/navigate")
                    .header("authorization", &bearer)
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"page":"not-a-valid-page"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(nav_bad.status(), StatusCode::BAD_REQUEST);

        // 5. Embedded AI Context & Reference (`GET /api/ai/context` & `GET /api/ai/reference`)
        assert_eq!(
            status_of("/api/ai/context", Some(&bearer)).await,
            StatusCode::OK
        );
        assert_eq!(
            status_of("/api/ai/reference?path=skills/colima/SKILL.md", Some(&bearer)).await,
            StatusCode::OK
        );
        assert_eq!(
            status_of("/api/ai/reference?path=../Cargo.toml", Some(&bearer)).await,
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            status_of("/api/ai/conversations", Some(&bearer)).await,
            StatusCode::OK
        );
        assert_eq!(
            status_of("/api/ai/conversations?include_a2a=true", Some(&bearer)).await,
            StatusCode::OK
        );
    }
}
