pub mod error;
pub mod routes;
mod api_server;
pub mod commands;
pub mod crash;
mod docker_state;
pub mod docker_events;
pub mod instance_reader;
pub mod path_util;
mod poller;
mod terminal_session;
pub mod adapters;
pub mod services;

// Extracted from api_server.rs for single-responsibility
pub mod auth;
pub mod sse;
pub mod validation;
pub mod redact;
pub mod tray;
pub mod platform;
pub mod helpers;
/// Long-running CLI commands whose output must not be buffered into RAM.
pub mod streaming_cmd;
pub mod transfer_registry;

use commands::ai_chat;
use commands::announcements;
use commands::colima;
use commands::compose;
use commands::compose_diagnose;
use commands::containers;
use commands::diagnostics;
use commands::file_transfer;
use commands::runtime;
use commands::k8s_cluster;
use commands::k8s_resources;
use commands::kind;
use commands::knowledge_bank;
use commands::kubernetes;
use commands::lima;
use commands::metrics_collector;
use commands::models;
use commands::networks;
use commands::searxng;
use commands::security_scan;
use commands::shell_sandbox;
use commands::system;
use commands::terminal;
use commands::topology;
use commands::volumes;
use poller::PollerState;

/// Kill every pty on the way out.
///
/// Terminal sessions hold an ssh process inside the VM. Once the window is
/// gone there is no UI left to close them from, so they would survive the app.
fn reap_terminal_sessions(app: &tauri::AppHandle) {
    use tauri::Manager;
    // Clone the Arc out of managed state: the `State` guard borrows the handle.
    let mgr = (*app.state::<terminal_session::SharedSessionManager>()).clone();
    // Bound to a statement rather than used directly in `if let`: a scrutinee
    // temporary lives to the end of the enclosing block, which would outlive
    // `mgr` itself. Declared after `mgr`, so it drops first.
    let locked = mgr.lock();
    if let Ok(mut m) = locked {
        m.close_all();
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Redact secrets from panic output before anything else can panic.
    crash::install();

    // Fix PATH so we can find colima, docker, limactl etc.
    // when launched from Finder/Dock (which doesn't inherit shell PATH)
    path_util::fix_path_env();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_http::init())
        // Native file pickers for import/export. The picker records the user's
        // intent in the UI; it is not an authorization boundary — written paths
        // are confined in commands::file_transfer regardless of how they arrived.
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        // Receives colimaui://auth-callback?... after the OAuth consent screen.
        // The URL is untrusted transport: PKCE + a `state` check on the webview
        // side are what make the callback safe (see src/lib/account-oauth.ts).
        .plugin(tauri_plugin_deep_link::init())
        .manage(PollerState::default())
        // One manager for the whole app: terminal tabs are Tauri commands, and
        // they all have to reach the same set of live ptys.
        .manage(terminal_session::create_session_manager())
        .setup(|app| {
            // Initialize Knowledge Bank (SQLite)
            knowledge_bank::init_knowledge_bank();

            // Start HTTP API server for browser-mode access
            api_server::start_api_server();
            // Start background instance poller
            poller::start_instance_poller(app.handle());

            // The app's single metrics sampling loop. It stays idle until a
            // client subscribes to the `metrics.sample` topic, so starting it
            // here costs nothing when nobody has opened the Activity page.
            metrics_collector::spawn_collector();

            // Setup DockerState
            use std::sync::Arc;
            use tauri::Manager;
            use tokio::sync::RwLock;
            let docker_state = Arc::new(RwLock::new(docker_state::DockerState::new()));
            app.manage(docker_state.clone());

            // Start Docker state watcher
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                docker_state::start_docker_watcher(app_handle, docker_state).await;
            });

            // Self-healing listens and counts unconditionally; whether anything
            // is done about what it sees is decided per action, against the kill
            // switch at the moment of acting.
            commands::self_heal::spawn_watcher();
            commands::self_heal::spawn_sweeper();

            // Resource Saver Mode
            let resource_saver_state = Arc::new(RwLock::new(commands::system::ResourceSaverState::default()));
            app.manage(resource_saver_state.clone());
            commands::system::start_resource_saver_daemon(resource_saver_state);

            // Tray last: it reads the poller's instance list, so the poller has
            // to exist first. Failure here is logged and ignored — several
            // Linux desktops have no system tray and the app works without one.
            tray::init(app.handle());

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Runtime info
            runtime::get_runtime_info,
            // Colima commands
            colima::list_instances,
            colima::start_instance,
            colima::stop_instance,
            colima::delete_instance,
            colima::instance_status,
            colima::get_ssh_command,
            colima::kubernetes_action,
            colima::collect_diagnostic_logs,
            colima::create_worker_node,
            // Colima config (colima.yaml is the single source of truth)
            commands::colima_config::get_colima_config,
            commands::colima_config::preview_colima_config,
            commands::colima_config::apply_colima_config,
            // Docker commands
            containers::list_containers,
            containers::start_container,
            containers::stop_container,
            containers::restart_container,
            containers::remove_container,
            containers::container_logs,
            containers::list_images,
            containers::inspect_container,
            containers::remove_image,
            containers::pull_image,
            containers::prune_images,
            containers::inspect_image,
            containers::tag_image,
            containers::system_prune,
            containers::system_df,
            containers::container_stats,
            containers::all_container_stats,
            containers::container_top,
            containers::container_exec,
            containers::run_container,
            containers::rename_container,
            containers::pause_container,
            containers::unpause_container,
            containers::docker_diagnose,
            // Volume commands
            volumes::list_volumes,
            volumes::create_volume,
            volumes::remove_volume,
            volumes::prune_volumes,
            volumes::inspect_volume,
            // Network commands
            networks::list_networks,
            networks::create_network,
            networks::remove_network,
            networks::inspect_network,
            networks::prune_networks,
            // Docker topology graph
            topology::get_topology,
            // Live metrics sampling period
            metrics_collector::set_metrics_interval,
            // Diagnostic bundle for bug reports
            diagnostics::diagnostic_bundle,
            diagnostics::save_diagnostic_bundle,
            // File + image transfers (background, cancellable)
            file_transfer::image_save,
            file_transfer::image_load,
            file_transfer::copy_to_container,
            file_transfer::copy_from_container,
            file_transfer::cancel_transfer,
            file_transfer::transfer_list,
            announcements::announcements_fetch,
            // Image vulnerability scanning
            security_scan::security_scan_image,
            security_scan::security_scan_cancel,
            security_scan::security_sbom_export,
            security_scan::security_audit_image,
            commands::security_rules::security_rule_pack,
            // Self-healing rules (Pro; the kill switch is not gated)
            commands::self_heal::self_heal_list_rules,
            commands::self_heal::self_heal_save_rule,
            commands::self_heal::self_heal_recent_log,
            commands::self_heal::self_heal_is_enabled,
            commands::self_heal::self_heal_set_enabled,
            // Local action history
            commands::activity::activity_query,
            commands::activity_feed::activity_feed,
            commands::activity_feed::activity_export,
            commands::security_catalog::security_alternatives,
            // Model commands
            models::list_models,
            models::pull_model,
            models::serve_model,
            models::delete_model,
            // System commands
            system::check_system,
            system::get_colima_version,
            system::check_tool,
            system::host_specs,
            commands::engine_resources::engine_resources,
            commands::system_capabilities::get_system_capabilities,
            system::get_app_context,
            system::read_reference,
            // Compose commands
            compose::list_compose_projects,
            compose::compose_up,
            compose::compose_down,
            compose::compose_restart,
            compose::compose_logs,
            compose::compose_ps,
            compose_diagnose::compose_validate,
            compose_diagnose::compose_diagnose,
            // Kubernetes commands
            kubernetes::k8s_check,
            kubernetes::k8s_namespaces,
            kubernetes::k8s_pods,
            kubernetes::k8s_services,
            kubernetes::k8s_deployments,
            kubernetes::k8s_pod_logs,
            kubernetes::k8s_delete_pod,
            kubernetes::k8s_describe,
            kubernetes::k8s_exec,
            // Resource-scoped k8s operations. These shipped as HTTP routes
            // only, so they worked in browser mode and failed in the app.
            k8s_resources::k8s_apply,
            k8s_resources::k8s_yaml,
            k8s_resources::k8s_delete_resource,
            k8s_resources::k8s_restart,
            k8s_resources::k8s_generic_scale,
            k8s_resources::k8s_crds,
            k8s_resources::k8s_crd_resources,
            k8s_resources::k8s_pod_containers,
            k8s_resources::k8s_container_logs,
            // Cluster-scoped k8s operations.
            k8s_cluster::k8s_contexts,
            k8s_cluster::k8s_current_context,
            k8s_cluster::k8s_set_context,
            k8s_cluster::k8s_nodes_json,
            k8s_cluster::k8s_events_json,
            k8s_cluster::k8s_node_action,
            k8s_cluster::k8s_pf_list,
            k8s_cluster::k8s_pf_start,
            k8s_cluster::k8s_pf_stop,
            k8s_cluster::k8s_cluster_health,
            k8s_cluster::k8s_benchmark,
            // kind clusters.
            kind::kind_list,
            kind::kind_create,
            kind::kind_delete,
            kubernetes::k8s_scale,
            kubernetes::k8s_nodes,
            kubernetes::k8s_events,
            kubernetes::k8s_resources,
            // Lima commands
            lima::lima_create,
            lima::lima_list,
            lima::lima_start,
            lima::lima_stop,
            lima::lima_delete,
            lima::lima_info,
            lima::lima_shell,
            lima::lima_templates,
            // AI Chat
            ai_chat::ai_chat,
            ai_chat::ai_list_models,
            ai_chat::ai_chat_load_history,
            ai_chat::ai_chat_save_message,
            ai_chat::ai_chat_clear_history,
            ai_chat::ai_chat_list_conversations,
            ai_chat::ai_chat_create_conversation,
            ai_chat::ai_chat_rename_conversation,
            ai_chat::ai_chat_delete_conversation,
            // Terminal (pty over IPC)
            terminal::terminal_create,
            terminal::terminal_write,
            terminal::terminal_resize,
            terminal::terminal_poll_exit,
            terminal::terminal_close,
            // AI Diagnostics — SearXNG + HTML→MD
            searxng::searxng_search,
            searxng::fetch_page_as_markdown,
            // Knowledge Bank
            commands::kb_articles::kb_list_articles,
            commands::kb_articles::kb_get_article,
            commands::kb_articles::kb_search_articles,
            knowledge_bank::kb_query,
            knowledge_bank::kb_feedback,
            knowledge_bank::kb_save_solution,
            knowledge_bank::kb_learn,
            knowledge_bank::kb_save_anti_pattern,
            knowledge_bank::add_memory,
            knowledge_bank::search_memory,
            knowledge_bank::get_all_memories,
            knowledge_bank::update_memory,
            knowledge_bank::delete_memory,
            knowledge_bank::save_preset_snapshot,
            knowledge_bank::load_preset_snapshot,
            knowledge_bank::list_all_preset_snapshots,
            knowledge_bank::get_setting,
            knowledge_bank::set_setting,
            knowledge_bank::get_all_settings,
            knowledge_bank::get_preset,
            knowledge_bank::get_all_presets,
            knowledge_bank::save_preset,
            knowledge_bank::delete_preset,
            // Shell Sandbox
            shell_sandbox::sandbox_classify,
            shell_sandbox::sandbox_execute,
            shell_sandbox::sandbox_execute_approved,
            // System and API Server
            api_server::get_platform,
            // The desktop webview's only way to get the HTTP API token, now
            // that the public endpoint that used to serve it is gone. Needed
            // for SSE, which cannot send an Authorization header.
            api_server::api_token,
            system::set_resource_saver,
        ])
        .build(tauri::generate_context!())
        .expect("error while running tauri application")
        .run(|app, event| {
            // Reap every pty on the way out. Terminal sessions hold an ssh
            // process inside the VM; without this they survive the window that
            // owned them and there is no longer any UI to close them from.
            if let tauri::RunEvent::ExitRequested { .. } = event {
                reap_terminal_sessions(app);
                // Same reasoning for streamed commands: a `docker save` started
                // from a window that is going away has nothing left to report to,
                // and would otherwise keep writing a file nobody is waiting for.
                streaming_cmd::kill_all_streams();
            }
        });
}
