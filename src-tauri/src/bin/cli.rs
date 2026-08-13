//! ColimaUI CLI — standalone command-line interface.
//!
//! Provides a unified CLI for managing containers, VMs, images, and system
//! operations across multiple DevOps tools.
//!
//! Usage: colimaui <command> <subcommand> [args]

use clap::{Parser, Subcommand};

/// ColimaUI — Unified DevOps management CLI
#[derive(Parser)]
#[command(name = "colimaui", version, about = "Unified CLI for containers, VMs, and DevOps tools")]
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

#[derive(Subcommand)]
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
}

#[derive(Subcommand)]
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
    /// Get container stats
    Stats {
        /// Container ID (or all if not specified)
        id: Option<String>,
    },
    /// Passthrough command to the underlying container runtime
    #[command(external_subcommand)]
    Passthrough(Vec<String>),
}

#[derive(Subcommand)]
enum VmAction {
    /// List all VM instances
    Ls,
    /// Start a VM instance
    Start {
        /// Profile name
        #[arg(default_value = "default")]
        profile: String,
        /// Number of CPUs
        #[arg(long, default_value = "2")]
        cpus: u32,
        /// Memory in GB
        #[arg(long, default_value = "4")]
        memory: u32,
        /// Disk in GB
        #[arg(long, default_value = "60")]
        disk: u32,
        /// Container runtime
        #[arg(long, default_value = "docker")]
        runtime: String,
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

#[derive(Subcommand)]
enum K8sAction {
    /// Passthrough command to kubectl
    #[command(external_subcommand)]
    Passthrough(Vec<String>),
}

#[derive(Subcommand)]
enum ComposeAction {
    /// Passthrough command to docker-compose
    #[command(external_subcommand)]
    Passthrough(Vec<String>),
}

#[derive(Subcommand)]
enum LimaAction {
    /// Passthrough command to limactl
    #[command(external_subcommand)]
    Passthrough(Vec<String>),
}

#[derive(Subcommand)]
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
    /// Prune unused images
    Prune,
}

#[derive(Subcommand)]
enum SystemAction {
    /// Show disk usage
    Df,
    /// Prune unused resources
    Prune {
        #[arg(short, long)]
        all: bool,
    },
    /// Show system info
    Info,
}

fn main() {
    // Fix PATH for finding binaries (colima, docker, etc.)
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
        }
    });
}

async fn handle_k8s(action: K8sAction) {
    let svc = colima_ui_lib::services::orchestration::OrchestrationService::auto_detect();
    match action {
        K8sAction::Passthrough(args) => {
            if let Err(e) = svc.passthrough(&args).await {
                eprintln!("Error: {}", e);
            }
        }
    }
}

async fn handle_compose(action: ComposeAction) {
    let svc = colima_ui_lib::services::compose::ComposeService::auto_detect();
    match action {
        ComposeAction::Passthrough(args) => {
            if let Err(e) = svc.passthrough(&args).await {
                eprintln!("Error: {}", e);
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
                eprintln!("Error: {}", e);
            }
        }
    }
}

async fn handle_container(action: ContainerAction, json: bool) {
    let svc = colima_ui_lib::services::container::ContainerService::auto_detect();

    match action {
        ContainerAction::Ls { all } => {
            match svc.list_containers(all).await {
                Ok(containers) => {
                    if json {
                        println!("{}", serde_json::to_string_pretty(&containers).unwrap_or_default());
                    } else {
                        println!("{:<15} {:<25} {:<30} {:<20} PORTS", "CONTAINER ID", "NAME", "IMAGE", "STATUS");
                        for c in &containers {
                            println!("{:<15} {:<25} {:<30} {:<20} {}",
                                &c.id[..std::cmp::min(12, c.id.len())],
                                truncate(&c.name, 24),
                                truncate(&c.image, 29),
                                truncate(&c.status, 19),
                                &c.ports
                            );
                        }
                        println!("\n{} containers total", containers.len());
                    }
                }
                Err(e) => eprintln!("Error: {}", e),
            }
        }
        ContainerAction::Start { id } => print_result(svc.start_container(&id).await),
        ContainerAction::Stop { id } => print_result(svc.stop_container(&id).await),
        ContainerAction::Restart { id } => print_result(svc.restart_container(&id).await),
        ContainerAction::Rm { id, force } => print_result(svc.remove_container(&id, force).await),
        ContainerAction::Logs { id, lines } => {
            match svc.container_logs(&id, lines).await {
                Ok(logs) => print!("{}", logs),
                Err(e) => eprintln!("Error: {}", e),
            }
        }
        ContainerAction::Exec { id, command } => {
            let cmd = command.join(" ");
            match svc.exec(&id, &cmd).await {
                Ok(output) => print!("{}", output),
                Err(e) => eprintln!("Error: {}", e),
            }
        }
        ContainerAction::Stats { id: _id } => {
            eprintln!("Stats not yet implemented in CLI");
        }
        ContainerAction::Passthrough(args) => {
            if let Err(e) = svc.passthrough(&args).await {
                eprintln!("Error: {}", e);
            }
        }
    }
}

async fn handle_vm(action: VmAction, json: bool) {
    let svc = colima_ui_lib::services::vm::VMService::colima();

    match action {
        VmAction::Ls => {
            match svc.list().await {
                Ok(instances) => {
                    if json {
                        println!("{}", serde_json::to_string_pretty(&instances).unwrap_or_default());
                    } else {
                        println!("{:<20} {:<12} {:<8} {:<6} {:<10} {:<10} {:<10} ADDRESS", "PROFILE", "STATUS", "ARCH", "CPUS", "MEMORY", "DISK", "RUNTIME");
                        for i in &instances {
                            println!("{:<20} {:<12} {:<8} {:<6} {:<10} {:<10} {:<10} {}",
                                i.name, i.status, i.arch, i.cpus,
                                format_bytes(i.memory),
                                format_bytes(i.disk),
                                i.runtime, i.address
                            );
                        }
                    }
                }
                Err(e) => eprintln!("Error: {}", e),
            }
        }
        VmAction::Start { profile, cpus, memory, disk, runtime } => {
            use colima_ui_lib::adapters::traits::VMConfig;
            let config = VMConfig {
                profile,
                runtime,
                cpus,
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
        VmAction::Status { profile } => {
            match svc.status(&profile).await {
                Ok(status) => {
                    if json {
                        println!("{}", serde_json::to_string_pretty(&status).unwrap_or_default());
                    } else {
                        println!("Profile: {}", status.profile);
                        println!("Status:  {}", status.status);
                        println!("Arch:    {}", status.arch);
                        println!("Runtime: {}", status.runtime);
                        if !status.cpu_usage.is_empty() { println!("CPU:     {}", status.cpu_usage); }
                        if !status.memory_usage.is_empty() { println!("Memory:  {}", status.memory_usage); }
                        if !status.disk_usage.is_empty() { println!("Disk:    {}", status.disk_usage); }
                        if !status.address.is_empty() { println!("Address: {}", status.address); }
                    }
                }
                Err(e) => eprintln!("Error: {}", e),
            }
        }
        VmAction::Ssh { profile } => {
            match svc.ssh_command(&profile).await {
                Ok(args) => println!("colima {}", args.join(" ")),
                Err(e) => eprintln!("Error: {}", e),
            }
        }
        VmAction::Passthrough(args) => {
            if let Err(e) = svc.passthrough(&args).await {
                eprintln!("Error: {}", e);
            }
        }
    }
}

async fn handle_image(action: ImageAction, json: bool) {
    let svc = colima_ui_lib::services::container::ContainerService::auto_detect();

    match action {
        ImageAction::Ls => {
            match svc.list_images().await {
                Ok(images) => {
                    if json {
                        println!("{}", serde_json::to_string_pretty(&images).unwrap_or_default());
                    } else {
                        println!("{:<40} {:<15} {:<15}", "REPOSITORY", "TAG", "SIZE");
                        for img in &images {
                            println!("{:<40} {:<15} {:<15}", truncate(&img.repository, 39), truncate(&img.tag, 14), &img.size);
                        }
                    }
                }
                Err(e) => eprintln!("Error: {}", e),
            }
        }
        ImageAction::Pull { name } => print_result(svc.pull_image(&name).await),
        ImageAction::Rm { id, force } => print_result(svc.remove_image(&id, force).await),
        ImageAction::Prune => {
            // Use the runtime directly for prune
            match svc.system_prune(false).await {
                Ok(output) => print!("{}", output),
                Err(e) => eprintln!("Error: {}", e),
            }
        }
    }
}

async fn handle_system(action: SystemAction, _json: bool) {
    let svc = colima_ui_lib::services::container::ContainerService::auto_detect();

    match action {
        SystemAction::Df => {
            match svc.system_df().await {
                Ok(output) => print!("{}", output),
                Err(e) => eprintln!("Error: {}", e),
            }
        }
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

    if json {
        let mut status = serde_json::Map::new();
        if let Ok(vms) = vm_svc.list().await {
            status.insert("vms".into(), serde_json::to_value(&vms).unwrap_or_default());
        }
        if let Ok(containers) = container_svc.list_containers(true).await {
            status.insert("containers".into(), serde_json::to_value(&containers).unwrap_or_default());
        }
        println!("{}", serde_json::to_string_pretty(&status).unwrap_or_default());
    } else {
        println!("=== ColimaUI Status ===\n");
        println!("--- VM Instances ---");
        match vm_svc.list().await {
            Ok(vms) if !vms.is_empty() => {
                for vm in &vms {
                    println!("  {} [{}] — {} cpu, {} mem, {}", vm.name, vm.status, vm.cpus, format_bytes(vm.memory), vm.runtime);
                }
            }
            Ok(_) => println!("  No instances found"),
            Err(e) => println!("  Error: {}", e),
        }

        println!("\n--- Containers ---");
        match container_svc.list_containers(true).await {
            Ok(containers) if !containers.is_empty() => {
                let running = containers.iter().filter(|c| c.state == "running").count();
                let stopped = containers.len() - running;
                println!("  {} running, {} stopped", running, stopped);
            }
            Ok(_) => println!("  No containers"),
            Err(e) => println!("  Error: {}", e),
        }
    }
}

// --- Helpers ---

fn print_result<E: std::fmt::Display>(result: Result<String, E>) {
    match result {
        Ok(msg) => println!("{}", msg),
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() > max {
        format!("{}…", &s[..max - 1])
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
        format!("{:.1}GiB", gb)
    } else {
        let mb = bytes as f64 / (1024.0 * 1024.0);
        format!("{:.0}MiB", mb)
    }
}
