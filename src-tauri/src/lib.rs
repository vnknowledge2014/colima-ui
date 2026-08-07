pub mod error;
pub mod routes;
mod api_server;
pub mod commands;
mod docker_state;
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
pub mod platform;
pub mod helpers;

use commands::ai_chat;
use commands::colima;
use commands::compose;
use commands::containers;
use commands::runtime;
use commands::knowledge_bank;
use commands::kubernetes;
use commands::lima;
use commands::models;
use commands::networks;
use commands::searxng;
use commands::shell_sandbox;
use commands::system;
use commands::volumes;
use poller::PollerState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Fix PATH so we can find colima, docker, limactl etc.
    // when launched from Finder/Dock (which doesn't inherit shell PATH)
    path_util::fix_path_env();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_http::init())
        .manage(PollerState::default())
        .setup(|app| {
            // Initialize Knowledge Bank (SQLite)
            knowledge_bank::init_knowledge_bank();

            // Start HTTP API server for browser-mode access
            api_server::start_api_server();
            // Start background instance poller
            poller::start_instance_poller(app.handle());

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
            
            // Resource Saver Mode
            let resource_saver_state = Arc::new(RwLock::new(commands::system::ResourceSaverState::default()));
            app.manage(resource_saver_state.clone());
            commands::system::start_resource_saver_daemon(resource_saver_state);
            
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
            system::get_app_context,
            system::read_reference,
            // Compose commands
            compose::list_compose_projects,
            compose::compose_up,
            compose::compose_down,
            compose::compose_restart,
            compose::compose_logs,
            compose::compose_ps,
            // Kubernetes commands
            kubernetes::k8s_check,
            kubernetes::k8s_namespaces,
            kubernetes::k8s_pods,
            kubernetes::k8s_services,
            kubernetes::k8s_deployments,
            kubernetes::k8s_pod_logs,
            kubernetes::k8s_delete_pod,
            kubernetes::k8s_describe,
            kubernetes::k8s_scale,
            kubernetes::k8s_nodes,
            kubernetes::k8s_events,
            // Lima commands
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
            // AI Diagnostics — SearXNG + HTML→MD
            searxng::searxng_search,
            searxng::fetch_page_as_markdown,
            // Knowledge Bank
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
            system::set_resource_saver,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
