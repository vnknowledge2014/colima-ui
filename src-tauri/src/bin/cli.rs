//! ColimaUI CLI — standalone command-line interface.
//!
//! Provides a unified CLI for managing containers, VMs, images, and system
//! operations across multiple DevOps tools.
//!
//! Usage: colimaui <command> <subcommand> [args]

use clap::{Parser, Subcommand};

/// ColimaUI — Unified DevOps management CLI
#[derive(Debug, Parser)]
#[command(
    name = "colimaui",
    version,
    about = "Unified CLI for containers, VMs, and DevOps tools"
)]
struct Cli {
    /// Output format
    #[arg(long, default_value = "table")]
    format: String,

    /// Use JSON output (shortcut for --format json)
    #[arg(long)]
    json: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Container management
    #[command(alias = "c")]
    Container {
        #[command(subcommand)]
        action: ContainerAction,
    },
    /// VM instance management
    #[command(alias = "v")]
    Vm {
        #[command(subcommand)]
        action: VmAction,
    },
    /// Kubernetes management
    K8s {
        #[command(subcommand)]
        action: K8sAction,
    },
    /// Compose management
    Compose {
        #[command(subcommand)]
        action: ComposeAction,
    },
    /// Lima VM management
    Lima {
        #[command(subcommand)]
        action: LimaAction,
    },
    /// Image management
    #[command(alias = "i")]
    Image {
        #[command(subcommand)]
        action: ImageAction,
    },
    /// System operations
    #[command(alias = "sys")]
    System {
        #[command(subcommand)]
        action: SystemAction,
    },
    /// Show overall status dashboard
    Status,
    /// Start the Model Context Protocol (MCP) server over stdio
    Mcp,
}

#[derive(Debug, Subcommand)]
enum ContainerAction {
    /// List containers
    Ls {
        /// Show all containers (including stopped)
        #[arg(short, long)]
        all: bool,
    },
    /// Start a container
    Start { id: String },
    /// Stop a container
    Stop { id: String },
    /// Restart a container
    Restart { id: String },
    /// Remove a container
    Rm {
        id: String,
        #[arg(short, long)]
        force: bool,
    },
    /// View container logs
    Logs {
        id: String,
        /// Number of lines to show
        #[arg(short = 'n', long, default_value = "100")]
        lines: u32,
    },
    /// Execute a command in a container
    Exec {
        id: String,
        /// Command to execute
        #[arg(last = true)]
        command: Vec<String>,
    },
    /// Get container resource usage stats (`docker stats --no-stream`)
    Stats {
        /// Container ID or name (or all running containers if omitted)
        id: Option<String>,
    },
    /// Passthrough command to the underlying container runtime
    #[command(external_subcommand)]
    Passthrough(Vec<String>),
}

#[derive(Debug, Subcommand, PartialEq, Eq)]
enum VmAction {
    /// List all VM instances
    Ls,
    /// Start a VM instance (preserves colima.yaml unless override flags are passed)
    Start {
        /// Profile name
        #[arg(default_value = "default")]
        profile: String,
        /// Number of CPUs (only passed to colima when explicitly specified)
        #[arg(long = "cpu", visible_alias = "cpus")]
        cpu: Option<u32>,
        /// Memory in GB (only passed to colima when explicitly specified)
        #[arg(long)]
        memory: Option<u32>,
        /// Disk in GB (only passed to colima when explicitly specified)
        #[arg(long)]
        disk: Option<u32>,
        /// Container runtime (only passed to colima when explicitly specified)
        #[arg(long)]
        runtime: Option<String>,
    },
    /// Stop a VM instance
    Stop {
        #[arg(default_value = "default")]
        profile: String,
        #[arg(short, long)]
        force: bool,
    },
    /// Delete a VM instance
    Delete {
        #[arg(default_value = "default")]
        profile: String,
        #[arg(short, long)]
        force: bool,
    },
    /// Get VM instance status
    Status {
        #[arg(default_value = "default")]
        profile: String,
    },
    /// Get SSH command for a VM instance
    Ssh {
        #[arg(default_value = "default")]
        profile: String,
    },
    /// Passthrough command to the underlying VM manager
    #[command(external_subcommand)]
    Passthrough(Vec<String>),
}

#[derive(Debug, Subcommand)]
enum K8sAction {
    /// Passthrough command to kubectl
    #[command(external_subcommand)]
    Passthrough(Vec<String>),
}

#[derive(Debug, Subcommand)]
enum ComposeAction {
    /// Passthrough command to docker-compose
    #[command(external_subcommand)]
    Passthrough(Vec<String>),
}

#[derive(Debug, Subcommand)]
enum LimaAction {
    /// Passthrough command to limactl
    #[command(external_subcommand)]
    Passthrough(Vec<String>),
}

#[derive(Debug, Subcommand)]
enum ImageAction {
    /// List images
    Ls,
    /// Pull an image
    Pull { name: String },
    /// Remove an image
    Rm {
        id: String,
        #[arg(short, long)]
        force: bool,
    },
    /// Prune unused (dangling) images via `docker image prune -f`
    Prune,
}

#[derive(Debug, Subcommand)]
enum SystemAction {
    /// Show disk usage
    Df,
    /// Prune unused resources via `docker system prune -f`
    Prune {
        #[arg(short, long)]
        all: bool,
    },
    /// Show system info
    Info,
}

fn main() {
    // Adopt login-shell environment first, then ensure Homebrew/MacPorts/Nix paths are present.
    colima_ui_lib::shell_env::adopt();
    colima_ui_lib::path_util::fix_path_env();

    let cli = Cli::parse();
    let use_json = cli.json || cli.format == "json";

    let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");

    rt.block_on(async {
        match cli.command {
            Commands::Container { action } => handle_container(action, use_json).await,
            Commands::Vm { action } => handle_vm(action, use_json).await,
            Commands::K8s { action } => handle_k8s(action).await,
            Commands::Compose { action } => handle_compose(action).await,
            Commands::Lima { action } => handle_lima(action).await,
            Commands::Image { action } => handle_image(action, use_json).await,
            Commands::System { action } => handle_system(action, use_json).await,
            Commands::Status => handle_status(use_json).await,
            Commands::Mcp => {
                if let Err(e) = colima_ui_lib::mcp::run_stdio_server().await {
                    exit_error(e);
                }
            }
        }
    });
}

async fn handle_k8s(action: K8sAction) {
    let svc = colima_ui_lib::services::orchestration::OrchestrationService::auto_detect();
    match action {
        K8sAction::Passthrough(args) => {
            if let Err(e) = svc.passthrough(&args).await {
                exit_error(e);
            }
        }
    }
}

async fn handle_compose(action: ComposeAction) {
    let svc = colima_ui_lib::services::compose::ComposeService::auto_detect();
    match action {
        ComposeAction::Passthrough(args) => {
            if let Err(e) = svc.passthrough(&args).await {
                exit_error(e);
            }
        }
    }
}

async fn handle_lima(action: LimaAction) {
    use colima_ui_lib::adapters::traits::VMManager;
    let svc = colima_ui_lib::adapters::lima::LimaAdapter::new();
    match action {
        LimaAction::Passthrough(args) => {
            if let Err(e) = svc.passthrough(&args).await {
                exit_error(e);
            }
        }
    }
}

async fn handle_container(action: ContainerAction, json: bool) {
    let svc = colima_ui_lib::services::container::ContainerService::auto_detect();

    match action {
        ContainerAction::Ls { all } => match svc.list_containers(all).await {
            Ok(containers) => {
                if json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&containers).unwrap_or_default()
                    );
                } else {
                    println!(
                        "{:<15} {:<25} {:<30} {:<20} PORTS",
                        "CONTAINER ID", "NAME", "IMAGE", "STATUS"
                    );
                    for c in &containers {
                        let short_id: String = c.id.chars().take(12).collect();
                        println!(
                            "{:<15} {:<25} {:<30} {:<20} {}",
                            short_id,
                            truncate(&c.name, 24),
                            truncate(&c.image, 29),
                            truncate(&c.status, 19),
                            c.ports
                        );
                    }
                    println!("\n{} containers total", containers.len());
                }
            }
            Err(e) => exit_error(e),
        },
        ContainerAction::Start { id } => print_result(svc.start_container(&id).await),
        ContainerAction::Stop { id } => print_result(svc.stop_container(&id).await),
        ContainerAction::Restart { id } => print_result(svc.restart_container(&id).await),
        ContainerAction::Rm { id, force } => print_result(svc.remove_container(&id, force).await),
        ContainerAction::Logs { id, lines } => match svc.container_logs(&id, lines).await {
            Ok(logs) => print!("{logs}"),
            Err(e) => exit_error(e),
        },
        ContainerAction::Exec { id, command } => {
            let cmd = command.join(" ");
            match svc.exec(&id, &cmd).await {
                Ok(output) => print!("{output}"),
                Err(e) => exit_error(e),
            }
        }
        ContainerAction::Stats { id } => match svc.container_stats(id.as_deref()).await {
            Ok(raw) => print_container_stats(&raw, json),
            Err(e) => exit_error(e),
        },
        ContainerAction::Passthrough(args) => {
            if let Err(e) = svc.passthrough(&args).await {
                exit_error(e);
            }
        }
    }
}

fn print_container_stats(raw: &str, json: bool) {
    let entries: Vec<serde_json::Value> = raw
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .collect();

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&entries).unwrap_or_default()
        );
        return;
    }

    if entries.is_empty() && !raw.trim().is_empty() {
        print!("{raw}");
        return;
    }

    println!(
        "{:<14} {:<24} {:<10} {:<24} {:<10} {:<22} {:<22} PIDS",
        "CONTAINER ID", "NAME", "CPU %", "MEM USAGE / LIMIT", "MEM %", "NET I/O", "BLOCK I/O"
    );
    for entry in &entries {
        let id = entry
            .get("ID")
            .or_else(|| entry.get("Container"))
            .and_then(|v| v.as_str())
            .unwrap_or("-");
        let short_id: String = id.chars().take(12).collect();
        let name = entry.get("Name").and_then(|v| v.as_str()).unwrap_or("-");
        let cpu = entry.get("CPUPerc").and_then(|v| v.as_str()).unwrap_or("-");
        let mem_usage = entry
            .get("MemUsage")
            .and_then(|v| v.as_str())
            .unwrap_or("-");
        let mem_perc = entry.get("MemPerc").and_then(|v| v.as_str()).unwrap_or("-");
        let net_io = entry.get("NetIO").and_then(|v| v.as_str()).unwrap_or("-");
        let block_io = entry.get("BlockIO").and_then(|v| v.as_str()).unwrap_or("-");
        let pids = entry.get("PIDs").and_then(|v| v.as_str()).unwrap_or("-");

        println!(
            "{:<14} {:<24} {:<10} {:<24} {:<10} {:<22} {:<22} {}",
            short_id,
            truncate(name, 23),
            truncate(cpu, 9),
            truncate(mem_usage, 23),
            truncate(mem_perc, 9),
            truncate(net_io, 21),
            truncate(block_io, 21),
            pids
        );
    }
}

async fn handle_vm(action: VmAction, json: bool) {
    let svc = colima_ui_lib::services::vm::VMService::colima();

    match action {
        VmAction::Ls => match svc.list().await {
            Ok(instances) => {
                if json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&instances).unwrap_or_default()
                    );
                } else {
                    println!(
                        "{:<20} {:<12} {:<8} {:<6} {:<10} {:<10} {:<10} ADDRESS",
                        "PROFILE", "STATUS", "ARCH", "CPUS", "MEMORY", "DISK", "RUNTIME"
                    );
                    for i in &instances {
                        println!(
                            "{:<20} {:<12} {:<8} {:<6} {:<10} {:<10} {:<10} {}",
                            i.name,
                            i.status,
                            i.arch,
                            i.cpus,
                            format_bytes(i.memory),
                            format_bytes(i.disk),
                            i.runtime,
                            i.address
                        );
                    }
                }
            }
            Err(e) => exit_error(e),
        },
        VmAction::Start {
            profile,
            cpu,
            memory,
            disk,
            runtime,
        } => {
            use colima_ui_lib::adapters::traits::VMConfig;
            let config = VMConfig {
                profile,
                runtime,
                cpus: cpu,
                memory,
                disk,
                vm_type: String::new(),
                kubernetes: false,
                kubernetes_version: String::new(),
                arch: String::new(),
                mount_type: String::new(),
                mounts: vec![],
                dns: vec![],
                network_address: false,
            };
            print_result(svc.start(config).await);
        }
        VmAction::Stop { profile, force } => print_result(svc.stop(&profile, force).await),
        VmAction::Delete { profile, force } => print_result(svc.delete(&profile, force).await),
        VmAction::Status { profile } => match svc.status(&profile).await {
            Ok(status) => {
                if json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&status).unwrap_or_default()
                    );
                } else {
                    println!("Profile: {}", status.profile);
                    println!("Status:  {}", status.status);
                    println!("Arch:    {}", status.arch);
                    println!("Runtime: {}", status.runtime);
                    if !status.cpu_usage.is_empty() {
                        println!("CPU:     {}", status.cpu_usage);
                    }
                    if !status.memory_usage.is_empty() {
                        println!("Memory:  {}", status.memory_usage);
                    }
                    if !status.disk_usage.is_empty() {
                        println!("Disk:    {}", status.disk_usage);
                    }
                    if !status.address.is_empty() {
                        println!("Address: {}", status.address);
                    }
                }
            }
            Err(e) => exit_error(e),
        },
        VmAction::Ssh { profile } => match svc.ssh_command(&profile).await {
            Ok(args) => println!("colima {}", args.join(" ")),
            Err(e) => exit_error(e),
        },
        VmAction::Passthrough(args) => {
            if let Err(e) = svc.passthrough(&args).await {
                exit_error(e);
            }
        }
    }
}

async fn handle_image(action: ImageAction, json: bool) {
    let svc = colima_ui_lib::services::container::ContainerService::auto_detect();

    match action {
        ImageAction::Ls => match svc.list_images().await {
            Ok(images) => {
                if json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&images).unwrap_or_default()
                    );
                } else {
                    println!("{:<40} {:<15} {:<15}", "REPOSITORY", "TAG", "SIZE");
                    for img in &images {
                        println!(
                            "{:<40} {:<15} {:<15}",
                            truncate(&img.repository, 39),
                            truncate(&img.tag, 14),
                            img.size
                        );
                    }
                }
            }
            Err(e) => exit_error(e),
        },
        ImageAction::Pull { name } => print_result(svc.pull_image(&name).await),
        ImageAction::Rm { id, force } => print_result(svc.remove_image(&id, force).await),
        ImageAction::Prune => match svc.prune_images().await {
            Ok(output) => print!("{output}"),
            Err(e) => exit_error(e),
        },
    }
}

async fn handle_system(action: SystemAction, _json: bool) {
    let svc = colima_ui_lib::services::container::ContainerService::auto_detect();

    match action {
        SystemAction::Df => match svc.system_df().await {
            Ok(output) => print!("{output}"),
            Err(e) => exit_error(e),
        },
        SystemAction::Prune { all } => print_result(svc.system_prune(all).await),
        SystemAction::Info => {
            println!("ColimaUI v{}", env!("CARGO_PKG_VERSION"));
            println!("Runtime: {}", svc.runtime_name());
        }
    }
}

async fn handle_status(json: bool) {
    let vm_svc = colima_ui_lib::services::vm::VMService::colima();
    let container_svc = colima_ui_lib::services::container::ContainerService::auto_detect();

    let vms = match vm_svc.list().await {
        Ok(list) => list,
        Err(e) => exit_error(e),
    };
    let any_vm_running = vms
        .iter()
        .any(|vm| vm.status.eq_ignore_ascii_case("running"));

    let containers_res = container_svc.list_containers(true).await;
    if json {
        let mut status = serde_json::Map::new();
        status.insert("vms".into(), serde_json::to_value(&vms).unwrap_or_default());
        match containers_res {
            Ok(containers) => {
                status.insert(
                    "containers".into(),
                    serde_json::to_value(&containers).unwrap_or_default(),
                );
            }
            Err(e) if any_vm_running => exit_error(e),
            Err(_) => {
                status.insert("containers".into(), serde_json::Value::Array(Vec::new()));
            }
        }
        println!(
            "{}",
            serde_json::to_string_pretty(&status).unwrap_or_default()
        );
    } else {
        println!("=== ColimaUI Status ===\n");
        println!("--- VM Instances ---");
        if vms.is_empty() {
            println!("  No instances found");
        } else {
            for vm in &vms {
                println!(
                    "  {} [{}] — {} cpu, {} mem, {}",
                    vm.name,
                    vm.status,
                    vm.cpus,
                    format_bytes(vm.memory),
                    vm.runtime
                );
            }
        }

        println!("\n--- Containers ---");
        match containers_res {
            Ok(containers) if !containers.is_empty() => {
                let running = containers.iter().filter(|c| c.state == "running").count();
                let stopped = containers.len() - running;
                println!("  {running} running, {stopped} stopped");
            }
            Ok(_) => println!("  No containers"),
            Err(e) if any_vm_running => exit_error(e),
            Err(_) => println!("  Runtime not running"),
        }
    }
}

// --- Helpers ---

fn exit_error(err: impl std::fmt::Display) -> ! {
    eprintln!("Error: {err}");
    std::process::exit(1);
}

fn print_result<E: std::fmt::Display>(result: Result<String, E>) {
    match result {
        Ok(msg) => println!("{msg}"),
        Err(e) => exit_error(e),
    }
}

fn truncate(s: &str, max: usize) -> String {
    if max == 0 {
        return String::new();
    }
    if s.chars().count() > max {
        let head: String = s.chars().take(max.saturating_sub(1)).collect();
        format!("{head}…")
    } else {
        s.to_string()
    }
}

fn format_bytes(bytes: u64) -> String {
    if bytes == 0 {
        return "-".to_string();
    }
    let gb = bytes as f64 / (1024.0 * 1024.0 * 1024.0);
    if gb >= 1.0 {
        format!("{gb:.1}GiB")
    } else {
        let mb = bytes as f64 / (1024.0 * 1024.0);
        format!("{mb:.0}MiB")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_handles_vietnamese_cjk_and_emoji_without_utf8_panic() {
        // Vietnamese multi-byte diacritics
        let vi = "Hệ thống quản lý container đang hoạt động";
        let vi_trunc = truncate(vi, 12);
        assert_eq!(vi_trunc.chars().count(), 12);
        assert!(vi_trunc.ends_with('…'));
        assert_eq!(vi_trunc, "Hệ thống qu…");

        // CJK (Chinese + Japanese)
        let zh = "正在运行的容器列表测试字符串";
        let zh_trunc = truncate(zh, 6);
        assert_eq!(zh_trunc.chars().count(), 6);
        assert_eq!(zh_trunc, "正在运行的…");

        let ja = "実行中のコンテナ一覧を表示して";
        let ja_trunc = truncate(ja, 8);
        assert_eq!(ja_trunc.chars().count(), 8);
        assert_eq!(ja_trunc, "実行中のコンテ…");

        // 4-byte UTF-8 emojis
        let emoji = "🐳🚀🔥✨📦🦀⚙️";
        let emoji_trunc = truncate(emoji, 4);
        assert_eq!(emoji_trunc.chars().count(), 4);
        assert_eq!(emoji_trunc, "🐳🚀🔥…");

        // Edge cases
        assert_eq!(truncate("short", 10), "short");
        assert_eq!(truncate("exact", 5), "exact");
        assert_eq!(truncate("🐳🚀", 1), "…");
        assert_eq!(truncate("🐳🚀", 0), "");
    }

    #[test]
    fn vm_start_flags_default_to_none_to_preserve_colima_yaml() {
        let parsed = Cli::try_parse_from(["colimaui", "vm", "start"]).expect("parse vm start");
        match parsed.command {
            Commands::Vm { action } => {
                assert_eq!(
                    action,
                    VmAction::Start {
                        profile: "default".to_string(),
                        cpu: None,
                        memory: None,
                        disk: None,
                        runtime: None,
                    }
                );
            }
            other => panic!("unexpected command: {other:?}"),
        }

        let explicit = Cli::try_parse_from([
            "colimaui",
            "vm",
            "start",
            "dev",
            "--cpu",
            "4",
            "--memory",
            "8",
            "--disk",
            "100",
            "--runtime",
            "containerd",
        ])
        .expect("parse explicit vm start");
        match explicit.command {
            Commands::Vm { action } => {
                assert_eq!(
                    action,
                    VmAction::Start {
                        profile: "dev".to_string(),
                        cpu: Some(4),
                        memory: Some(8),
                        disk: Some(100),
                        runtime: Some("containerd".to_string()),
                    }
                );
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn mcp_and_container_stats_subcommands_parse_cleanly() {
        let mcp_cli = Cli::try_parse_from(["colimaui", "mcp"]).expect("parse colimaui mcp");
        assert!(matches!(mcp_cli.command, Commands::Mcp));

        let stats_all =
            Cli::try_parse_from(["colimaui", "container", "stats"]).expect("parse stats all");
        assert!(matches!(
            stats_all.command,
            Commands::Container {
                action: ContainerAction::Stats { id: None }
            }
        ));

        let stats_one = Cli::try_parse_from(["colimaui", "container", "stats", "web-1"])
            .expect("parse stats single");
        match stats_one.command {
            Commands::Container {
                action: ContainerAction::Stats { id },
            } => assert_eq!(id.as_deref(), Some("web-1")),
            other => panic!("unexpected command: {other:?}"),
        }
    }
}
