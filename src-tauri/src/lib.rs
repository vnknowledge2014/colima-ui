mod api_server;
mod commands;
mod instance_reader;
mod path_util;
mod poller;
mod terminal_session;

use commands::ai_chat;
use commands::colima;
use commands::compose;
use commands::docker;
use commands::kubernetes;
use commands::lima;
use commands::models;
use commands::networks;
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
        .plugin(tauri_plugin_shell::init())
        .manage(PollerState::default())
        .setup(|app| {
            // Start HTTP API server for browser-mode access
            api_server::start_api_server();
            // Start background instance poller
            poller::start_instance_poller(app.handle());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Colima commands
            colima::list_instances,
            colima::start_instance,
            colima::stop_instance,
            colima::delete_instance,
            colima::instance_status,
            colima::get_ssh_command,
            colima::kubernetes_action,
            // Docker commands
            docker::list_containers,
            docker::start_container,
            docker::stop_container,
            docker::restart_container,
            docker::remove_container,
            docker::container_logs,
            docker::list_images,
            docker::inspect_container,
            docker::remove_image,
            docker::pull_image,
            docker::prune_images,
            docker::inspect_image,
            docker::tag_image,
            docker::system_prune,
            docker::system_df,
            docker::container_stats,
            docker::all_container_stats,
            docker::container_top,
            docker::container_exec,
            docker::run_container,
            docker::rename_container,
            docker::pause_container,
            docker::unpause_container,
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

