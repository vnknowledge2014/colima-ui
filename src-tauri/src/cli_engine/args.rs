//! Clap v4 command tree for `cui` / `colimaui` (`v0.3.0`).
//!
//! Supports global `--json` in any argument position (`cui --json vm list` and
//! `cui vm list --json`), `--format json` backward compatibility, and all 22
//! first-class subcommand domains plus legacy `system` / `status` aliases.

use clap::{Args, Parser, Subcommand};

/// Default isolated session ID for automated CLI / MCP / A2A turns.
pub const DEFAULT_A2A_SESSION_ID: &str = "colima-a2a-default-session";

/// Top-level known subcommands and aliases used by the free-form `cui "<prompt>"` router.
pub const KNOWN_TOP_SUBCOMMANDS: &[&str] = &[
    "vm",
    "instance",
    "instances",
    "v",
    "config",
    "presets",
    "container",
    "containers",
    "c",
    "image",
    "images",
    "i",
    "volume",
    "volumes",
    "network",
    "networks",
    "compose",
    "k8s",
    "kind",
    "lima",
    "security",
    "models",
    "alerts",
    "self-heal",
    "memories",
    "kb",
    "sessions",
    "ui",
    "events",
    "doctor",
    "chat",
    "mcp",
    "system",
    "sys",
    "status",
    "help",
];

#[derive(Debug, Parser)]
#[command(
    name = "cui",
    version,
    about = "ColimaUI v0.3.0 — CLI-First DevOps, Security, Knowledge & Headless AI Engine",
    propagate_version = true
)]
pub struct Cli {
    /// Emit structured JSON (`{"ok": true, "data": ...}`) on stdout with zero ANSI/spinner output
    #[arg(long, global = true)]
    pub json: bool,

    /// Output format (`table` or `json`)
    #[arg(long, global = true, default_value = "table", hide_default_value = true)]
    pub format: String,

    #[command(subcommand)]
    pub command: Commands,
}

impl Cli {
    pub fn use_json(&self) -> bool {
        self.json || self.format.eq_ignore_ascii_case("json")
    }
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Manage Colima VM instances and K3s worker nodes
    #[command(visible_aliases = ["instance", "instances"], alias = "v")]
    Vm {
        #[command(subcommand)]
        action: VmCommand,
    },

    /// Inspect, preview, and apply colima.yaml configuration
    Config {
        #[command(subcommand)]
        action: ConfigCommand,
    },

    /// Manage Colima resource presets and container snapshots
    Presets {
        #[command(subcommand)]
        action: PresetsCommand,
    },

    /// Manage Docker / containerd containers
    #[command(visible_alias = "containers", alias = "c")]
    Container {
        #[command(subcommand)]
        action: ContainerCommand,
    },

    /// Manage container images and tar archives
    #[command(visible_alias = "images", alias = "i")]
    Image {
        #[command(subcommand)]
        action: ImageCommand,
    },

    /// Manage Docker / containerd volumes
    #[command(visible_alias = "volumes")]
    Volume {
        #[command(subcommand)]
        action: VolumeCommand,
    },

    /// Manage Docker / containerd networks
    #[command(visible_alias = "networks")]
    Network {
        #[command(subcommand)]
        action: NetworkCommand,
    },

    /// Manage Docker Compose stacks, diagnostics, and autofix patches
    Compose {
        #[command(subcommand)]
        action: ComposeCommand,
    },

    /// Manage Kubernetes clusters, workloads, manifests, and port-forwards
    K8s {
        #[command(subcommand)]
        action: K8sCommand,
    },

    /// Manage local kind (Kubernetes-in-Docker) clusters
    Kind {
        #[command(subcommand)]
        action: KindCommand,
    },

    /// Manage standalone Lima virtual machines and templates
    Lima {
        #[command(subcommand)]
        action: LimaCommand,
    },

    /// Image vulnerability scanning, SBOM export, posture audit, Falco, and detonation
    Security {
        #[command(subcommand)]
        action: SecurityCommand,
    },

    /// Manage AI models inside Colima VM runners
    Models {
        #[command(subcommand)]
        action: ModelsCommand,
    },

    /// View metric alert rules and recent alert events
    Alerts {
        #[command(subcommand)]
        action: AlertsCommand,
    },

    /// Inspect and toggle autonomous self-healing rules
    #[command(name = "self-heal")]
    SelfHeal {
        #[command(subcommand)]
        action: SelfHealCommand,
    },

    /// Query and manage Knowledge Bank solutions, FTS5 memories, and help articles
    #[command(visible_alias = "kb")]
    Memories {
        #[command(subcommand)]
        action: MemoriesCommand,
    },

    /// Inspect and manage AI chat conversation sessions
    Sessions {
        #[command(subcommand)]
        action: SessionsCommand,
    },

    /// Drive the running ColimaUI Desktop window (navigation & toasts)
    Ui {
        #[command(subcommand)]
        action: UiCommand,
    },

    /// Stream live SSE events from the running ColimaUI Desktop server
    Events(EventsArgs),

    /// Run comprehensive health, API, credential, AI, and binary diagnostics
    Doctor,

    /// Converse with ColimaUI's Headless DevOps AI Agent
    Chat(ChatArgs),

    /// Start the native Model Context Protocol (MCP) server over stdio
    Mcp,

    /// Host & container runtime system utilities (`check`, `df`, `prune`, `specs`)
    #[command(alias = "sys")]
    System {
        #[command(subcommand)]
        action: SystemCommand,
    },

    /// Show combined VM + container status summary
    Status,
}

// ============================================================================
// 1. VM
// ============================================================================

#[derive(Debug, Subcommand, PartialEq, Eq)]
pub enum VmCommand {
    /// List all Colima VM profiles
    #[command(visible_alias = "ls")]
    List,

    /// Show detailed status for a Colima profile
    Status {
        #[arg(default_value = "default")]
        profile: String,
    },

    /// Start a Colima profile (preserves colima.yaml unless override flags are supplied)
    Start {
        #[arg(default_value = "default")]
        profile: String,
        #[arg(long = "cpus", visible_alias = "cpu")]
        cpus: Option<u32>,
        #[arg(long)]
        memory: Option<u32>,
        #[arg(long)]
        disk: Option<u32>,
        #[arg(long)]
        runtime: Option<String>,
        #[arg(long)]
        arch: Option<String>,
        #[arg(long = "vm-type")]
        vm_type: Option<String>,
        #[arg(long = "k8s", visible_alias = "kubernetes")]
        k8s: bool,
    },

    /// Stop a running Colima profile
    Stop {
        #[arg(default_value = "default")]
        profile: String,
        #[arg(short, long)]
        force: bool,
    },

    /// Restart a Colima profile
    Restart {
        #[arg(default_value = "default")]
        profile: String,
    },

    /// Delete a Colima profile
    Delete {
        profile: String,
        #[arg(short, long)]
        force: bool,
    },

    /// Run an SSH command inside a Colima VM (or print SSH command when cmd is omitted)
    Ssh {
        #[arg(default_value = "default")]
        profile: String,
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        cmd: Vec<String>,
    },

    /// Create and join a K3s worker Colima node
    #[command(name = "create-worker")]
    CreateWorker {
        #[arg(long = "master-profile", default_value = "default")]
        master_profile: String,
        #[arg(long = "worker-profile", default_value = "k3s-worker-1")]
        worker_profile: String,
        #[arg(long, default_value_t = 2)]
        cpus: u32,
        #[arg(long, default_value_t = 2)]
        memory: u32,
        #[arg(long, default_value_t = 60)]
        disk: u32,
    },

    /// Passthrough arguments directly to `colima`
    #[command(external_subcommand)]
    Passthrough(Vec<String>),
}

// ============================================================================
// 2. Config
// ============================================================================

#[derive(Debug, Subcommand)]
pub enum ConfigCommand {
    /// Read managed colima.yaml fields for a profile
    Get {
        #[arg(long, default_value = "default")]
        profile: String,
    },

    /// Validate and preview colima.yaml edits without writing to disk
    Preview {
        /// Inline YAML/JSON snippet or `@path/to/file.yaml`
        #[arg(long)]
        yaml: String,
        #[arg(long, default_value = "default")]
        profile: String,
    },

    /// Apply colima.yaml edits atomically (optionally restarting the VM)
    Apply {
        /// Inline YAML/JSON snippet or `@path/to/file.yaml`
        #[arg(long)]
        yaml: String,
        #[arg(long, default_value = "default")]
        profile: String,
        #[arg(long)]
        restart: bool,
    },
}

// ============================================================================
// 3. Presets
// ============================================================================

#[derive(Debug, Subcommand)]
pub enum PresetsCommand {
    /// List saved user presets
    #[command(visible_alias = "ls")]
    List,

    /// Create or update a resource preset
    Save {
        #[arg(long)]
        id: String,
        #[arg(long)]
        name: String,
        #[arg(long)]
        cpus: u32,
        #[arg(long)]
        memory: u32,
        #[arg(long)]
        disk: u32,
        #[arg(long, default_value = "docker")]
        runtime: String,
    },

    /// Delete a saved preset by ID
    Delete {
        id: String,
    },

    /// List container snapshots recorded for presets
    Snapshots {
        #[arg(long, default_value = "default")]
        profile: String,
    },
}

// ============================================================================
// 4. Container
// ============================================================================

#[derive(Debug, Subcommand)]
pub enum ContainerCommand {
    /// List containers
    #[command(visible_alias = "ls")]
    List {
        #[arg(short, long)]
        all: bool,
    },

    /// Fetch recent container logs
    Logs {
        id: String,
        #[arg(short = 'n', long = "tail", visible_alias = "lines", default_value_t = 100)]
        tail: u32,
    },

    /// Inspect low-level container configuration
    Inspect {
        id: String,
    },

    /// Show non-streaming resource usage stats
    Stats {
        id: Option<String>,
    },

    /// Show running processes inside a container
    Top {
        id: String,
    },

    /// Start a stopped container
    Start {
        id: String,
    },

    /// Stop a running container
    Stop {
        id: String,
    },

    /// Restart a container
    Restart {
        id: String,
    },

    /// Pause all processes in a container
    Pause {
        id: String,
    },

    /// Unpause a paused container
    Unpause {
        id: String,
    },

    /// Rename an existing container
    Rename {
        id: String,
        new_name: String,
    },

    /// Remove a container
    #[command(visible_alias = "rm")]
    Remove {
        id: String,
        #[arg(short, long)]
        force: bool,
    },

    /// Execute a command inside a running container
    Exec {
        id: String,
        #[arg(required = true, trailing_var_arg = true, allow_hyphen_values = true)]
        cmd: Vec<String>,
    },

    /// Run a new container from an image
    Run {
        image: String,
        #[arg(long)]
        name: Option<String>,
        #[arg(short = 'p', long = "ports", visible_alias = "port")]
        ports: Vec<String>,
        #[arg(short = 'e', long = "env")]
        env: Vec<String>,
        #[arg(short, long)]
        detach: bool,
    },

    /// Passthrough arguments to the container runtime
    #[command(external_subcommand)]
    Passthrough(Vec<String>),
}

// ============================================================================
// 5. Image
// ============================================================================

#[derive(Debug, Subcommand)]
pub enum ImageCommand {
    /// List local images
    #[command(visible_alias = "ls")]
    List,

    /// Inspect image manifest and configuration
    Inspect {
        id: String,
    },

    /// Pull an image from a registry
    Pull {
        image: String,
    },

    /// Create a target tag referring to a source image
    Tag {
        source: String,
        target: String,
    },

    /// Remove a local image
    #[command(visible_alias = "rm")]
    Remove {
        id: String,
        #[arg(short, long)]
        force: bool,
    },

    /// Prune unused / dangling images
    Prune,

    /// Save an image to a tar archive (`docker save -o <output_tar> <image>`)
    Save {
        image: String,
        output_tar: String,
    },

    /// Load an image from a tar archive (`docker load -i <input_tar>`)
    Load {
        input_tar: String,
    },
}

// ============================================================================
// 6. Volume
// ============================================================================

#[derive(Debug, Subcommand)]
pub enum VolumeCommand {
    /// List volumes
    #[command(visible_alias = "ls")]
    List,

    /// Inspect a volume
    Inspect {
        name: String,
    },

    /// Create a new volume
    Create {
        name: String,
        #[arg(long, default_value = "local")]
        driver: String,
    },

    /// Remove a volume
    #[command(visible_alias = "rm")]
    Remove {
        name: String,
        #[arg(short, long)]
        force: bool,
    },

    /// Prune unused volumes
    Prune,
}

// ============================================================================
// 7. Network
// ============================================================================

#[derive(Debug, Subcommand)]
pub enum NetworkCommand {
    /// List networks
    #[command(visible_alias = "ls")]
    List,

    /// Inspect a network
    Inspect {
        name: String,
    },

    /// Create a network
    Create {
        name: String,
        #[arg(long, default_value = "bridge")]
        driver: String,
        #[arg(long, default_value = "")]
        subnet: String,
    },

    /// Remove a network
    #[command(visible_alias = "rm")]
    Remove {
        name: String,
    },

    /// Prune unused networks
    Prune,
}

// ============================================================================
// 8. Compose
// ============================================================================

#[derive(Debug, Subcommand)]
pub enum ComposeCommand {
    /// List Compose projects
    #[command(visible_alias = "ls")]
    List,

    /// Show containers belonging to a Compose project or directory
    Ps {
        project_or_dir: String,
    },

    /// Fetch logs from a Compose project
    Logs {
        project_or_dir: String,
        #[arg(short = 'n', long = "tail", default_value_t = 100)]
        tail: u32,
    },

    /// Create and start services from a Compose directory or file
    Up {
        dir: String,
        #[arg(long = "no-detach")]
        no_detach: bool,
    },

    /// Stop and remove a Compose project
    Down {
        project: String,
    },

    /// Start stopped services in a Compose project
    Start {
        project: String,
    },

    /// Stop running services in a Compose project
    Stop {
        project: String,
    },

    /// Restart services in a Compose project
    Restart {
        project: String,
    },

    /// Run static & port/image diagnostics on a Compose file or directory
    Diagnose {
        file_or_dir: String,
    },

    /// Generate an autofix proposal for a Compose file or directory
    #[command(name = "autofix-propose")]
    AutofixPropose {
        file_or_dir: String,
    },

    /// Apply a Compose autofix patch JSON string or `@patch.json`
    #[command(name = "autofix-apply")]
    AutofixApply {
        patch_json_or_file: String,
    },

    /// Undo a previously applied Compose autofix by backup/fix ID
    #[command(name = "autofix-undo")]
    AutofixUndo {
        backup_id: String,
    },

    /// List applied Compose autofix records
    #[command(name = "autofix-history")]
    AutofixHistory,

    /// Passthrough arguments to `docker compose`
    #[command(external_subcommand)]
    Passthrough(Vec<String>),
}

// ============================================================================
// 9. Kubernetes
// ============================================================================

#[derive(Debug, Subcommand)]
pub enum K8sCommand {
    /// Show cluster health and readiness summary
    Status,

    /// List available kubeconfig contexts
    Contexts,

    /// Switch the active kubeconfig context
    #[command(name = "use-context")]
    UseContext {
        ctx: String,
    },

    /// List Kubernetes namespaces
    Namespaces,

    /// List pods (`-n all` for all namespaces)
    Pods {
        #[arg(short = 'n', long = "namespace", default_value = "all")]
        namespace: String,
    },

    /// List deployments
    Deployments {
        #[arg(short = 'n', long = "namespace", default_value = "default")]
        namespace: String,
    },

    /// List services
    Services {
        #[arg(short = 'n', long = "namespace", default_value = "default")]
        namespace: String,
    },

    /// List cluster nodes
    Nodes,

    /// List cluster events
    Events {
        #[arg(short = 'n', long = "namespace", default_value = "default")]
        namespace: String,
    },

    /// List generic resources by kind (e.g. `configmaps`, `statefulsets`, `ingresses`)
    Resources {
        kind: String,
        #[arg(short = 'n', long = "namespace", default_value = "default")]
        namespace: String,
    },

    /// Fetch logs from a pod
    Logs {
        pod: String,
        #[arg(short = 'n', long = "namespace", default_value = "default")]
        namespace: String,
        #[arg(long = "tail", default_value_t = 100)]
        tail: u32,
    },

    /// Run `kubectl describe` on a resource
    Describe {
        kind: String,
        name: String,
        #[arg(short = 'n', long = "namespace", default_value = "default")]
        namespace: String,
    },

    /// Fetch live YAML manifest for a resource
    Yaml {
        kind: String,
        name: String,
        #[arg(short = 'n', long = "namespace", default_value = "default")]
        namespace: String,
    },

    /// Apply a Kubernetes YAML manifest file
    Apply {
        yaml_file: String,
        #[arg(short = 'n', long = "namespace", default_value = "")]
        namespace: String,
    },

    /// Scale a deployment to a replica count
    Scale {
        deployment: String,
        replicas: u32,
        #[arg(short = 'n', long = "namespace", default_value = "default")]
        namespace: String,
    },

    /// Rollout restart a workload (`deployment`, `statefulset`, `daemonset`)
    Restart {
        kind: String,
        name: String,
        #[arg(short = 'n', long = "namespace", default_value = "default")]
        namespace: String,
    },

    /// Delete a Kubernetes resource
    Delete {
        kind: String,
        name: String,
        #[arg(short = 'n', long = "namespace", default_value = "default")]
        namespace: String,
    },

    /// List, start, or stop kubectl port-forwards
    #[command(name = "port-forward")]
    PortForward {
        #[command(subcommand)]
        action: Option<PortForwardAction>,
    },

    /// Execute a command inside a pod (`cui k8s exec <pod> -n default -- <cmd...>`)
    Exec {
        pod: String,
        #[arg(short = 'n', long = "namespace", default_value = "default")]
        namespace: String,
        #[arg(short = 'c', long = "container", default_value = "")]
        container: String,
        #[arg(last = true, required = true)]
        cmd: Vec<String>,
    },

    /// List CustomResourceDefinitions (CRDs) in the cluster
    Crds,

    /// Passthrough arguments to `kubectl`
    #[command(external_subcommand)]
    Passthrough(Vec<String>),
}

#[derive(Debug, Subcommand)]
pub enum PortForwardAction {
    /// List active port-forwards
    #[command(visible_alias = "ls")]
    List,

    /// Start a background port-forward (`pod/my-pod` or `svc/my-svc`)
    Start {
        /// Target in `<kind>/<name>` or `<pod_name>` form
        target: String,
        local_port: u16,
        remote_port: u16,
        #[arg(short = 'n', long = "namespace", default_value = "default")]
        namespace: String,
    },

    /// Stop an active port-forward by local port
    Stop {
        local_port: u16,
    },
}

// ============================================================================
// 10. Kind
// ============================================================================

#[derive(Debug, Subcommand)]
pub enum KindCommand {
    /// List local kind clusters
    #[command(visible_alias = "ls")]
    List,

    /// Create a new kind cluster
    Create {
        name: String,
        #[arg(long, default_value = "")]
        image: String,
    },

    /// Delete a kind cluster
    Delete {
        name: String,
    },
}

// ============================================================================
// 11. Lima
// ============================================================================

#[derive(Debug, Subcommand)]
pub enum LimaCommand {
    /// List Lima VMs
    #[command(visible_alias = "ls")]
    List,

    /// Inspect Lima host & version info
    Info {
        #[arg(default_value = "")]
        name: String,
    },

    /// List available Lima VM templates
    Templates,

    /// Create a new Lima VM from a template
    Create {
        name: String,
        #[arg(long, default_value = "default")]
        template: String,
        #[arg(long, default_value_t = 2)]
        cpus: u32,
        #[arg(long, default_value_t = 2)]
        memory: u32,
        #[arg(long, default_value_t = 60)]
        disk: u32,
    },

    /// Start a Lima VM
    Start {
        name: String,
    },

    /// Stop a Lima VM
    Stop {
        name: String,
    },

    /// Delete a Lima VM
    Delete {
        name: String,
        #[arg(short, long)]
        force: bool,
    },

    /// Execute a command inside a Lima VM
    Shell {
        name: String,
        #[arg(required = true, trailing_var_arg = true, allow_hyphen_values = true)]
        cmd: Vec<String>,
    },

    /// Passthrough arguments to `limactl`
    #[command(external_subcommand)]
    Passthrough(Vec<String>),
}

// ============================================================================
// 12. Security
// ============================================================================

#[derive(Debug, Subcommand)]
pub enum SecurityCommand {
    /// Scan a local container image for CVEs via Trivy
    Scan {
        image: String,
        #[arg(long)]
        refresh: bool,
    },

    /// Generate an SBOM (`cyclonedx-json` or `spdx-json`) for a local image
    Sbom {
        image: String,
        #[arg(long, default_value = "cyclonedx-json")]
        format: String,
        /// Optional output file path (prints SBOM payload if omitted)
        #[arg(short, long)]
        output: Option<String>,
    },

    /// Run full vulnerability scan + rule evaluation + posture score
    Audit {
        image: String,
        #[arg(long)]
        refresh: bool,
    },

    /// Rank failing security rules by remediation score impact
    Triage {
        image: String,
    },

    /// View posture score scan history
    History {
        /// Optional image digest filter
        #[arg(long)]
        digest: Option<String>,
        #[arg(long, default_value_t = 25)]
        limit: i64,
    },

    /// Display the built-in image security rule pack
    Rules,

    /// Suggest cleaner/hardened base image alternatives
    Alternatives {
        image: String,
    },

    /// Detect Falco runtime security sensor state inside the VM
    #[command(name = "falco-state")]
    FalcoState,

    /// Read stored Falco runtime security events
    #[command(name = "falco-events")]
    FalcoEvents {
        #[arg(long, default_value_t = 50)]
        limit: i64,
    },

    /// List isolated image observation (detonation) sessions
    #[command(name = "detonation-list")]
    DetonationList,
}

// ============================================================================
// 13. Models
// ============================================================================

#[derive(Debug, Subcommand)]
pub enum ModelsCommand {
    /// List AI models installed in the VM runner
    #[command(visible_alias = "ls")]
    List {
        #[arg(long, default_value = "default")]
        profile: String,
    },

    /// Pull an AI model into the VM runner
    Pull {
        model: String,
        #[arg(long, default_value = "default")]
        profile: String,
    },

    /// Serve an AI model on a local port
    Serve {
        model: String,
        #[arg(long, default_value = "default")]
        profile: String,
        #[arg(long, default_value_t = 11434)]
        port: u16,
    },

    /// Delete an AI model from the VM runner
    Delete {
        model: String,
        #[arg(long, default_value = "default")]
        profile: String,
    },
}

// ============================================================================
// 14. Alerts
// ============================================================================

#[derive(Debug, Subcommand)]
pub enum AlertsCommand {
    /// List configured metric alert rules
    Rules,

    /// List recent fired alert events
    Events {
        #[arg(long, default_value_t = 50)]
        limit: i64,
    },
}

// ============================================================================
// 15. Self-Heal
// ============================================================================

#[derive(Debug, Subcommand)]
pub enum SelfHealCommand {
    /// Show whether autonomous self-healing is currently enabled
    Status,

    /// List self-healing rules and thresholds
    Rules,

    /// Show recent self-healing execution log entries
    Log {
        #[arg(long, default_value_t = 50)]
        limit: i64,
    },

    /// Enable the global self-healing switch
    Enable,

    /// Disable the global self-healing kill-switch
    Disable,
}

// ============================================================================
// 16. Memories (Knowledge Bank)
// ============================================================================

#[derive(Debug, Subcommand)]
pub enum MemoriesCommand {
    /// Match an error message against Knowledge Bank solutions & anti-patterns
    Query {
        text: String,
    },

    /// Full-text BM25 search across agent memories
    Search {
        text: String,
        #[arg(long, default_value_t = 5)]
        limit: u32,
    },

    /// List all stored agent memories
    #[command(visible_alias = "ls")]
    List,

    /// Add a new agent memory entry
    Add {
        content: String,
        #[arg(long, default_value = "preference")]
        category: String,
    },

    /// Delete an agent memory entry by ID
    Delete {
        id: String,
    },

    /// Teach the Knowledge Bank a new error pattern -> solution rule
    Learn {
        pattern: String,
        solution: String,
    },

    /// List or search built-in troubleshooting help articles
    Articles {
        search_query: Option<String>,
        #[arg(long, default_value = "en")]
        locale: String,
    },
}

// ============================================================================
// 17. Sessions
// ============================================================================

#[derive(Debug, Subcommand)]
pub enum SessionsCommand {
    /// List AI chat threads (excludes `colima-a2a-default-session` unless `--include-a2a`)
    #[command(visible_alias = "ls")]
    List {
        #[arg(long = "include-a2a")]
        include_a2a: bool,
    },

    /// Show message history for a session (defaults to `default`)
    History {
        session_id: Option<String>,
    },

    /// Clear messages in a session without deleting the thread
    Clear {
        session_id: Option<String>,
    },

    /// Delete a conversation thread and its messages
    Delete {
        session_id: String,
    },
}

// ============================================================================
// 18. UI
// ============================================================================

#[derive(Debug, Subcommand)]
pub enum UiCommand {
    /// Navigate the running ColimaUI Desktop window to a target page/section
    Navigate {
        page: String,
        #[arg(long)]
        section: Option<String>,
        #[arg(long)]
        toast: Option<String>,
    },
}

// ============================================================================
// 19. Events
// ============================================================================

#[derive(Debug, Args)]
pub struct EventsArgs {
    /// Comma-separated SSE topics to subscribe to (e.g. `containers,instances`)
    #[arg(long)]
    pub topics: Option<String>,

    /// Case-insensitive substring filter matched against event name or data
    #[arg(long)]
    pub filter: Option<String>,

    /// Stop after receiving N matching events (defaults to 10 when `--json` is set)
    #[arg(long)]
    pub limit: Option<usize>,

    /// Maximum wait time in seconds before closing stream when `--json` is used (default: 15s)
    #[arg(long = "timeout-secs", default_value_t = 15)]
    pub timeout_secs: u64,
}

// ============================================================================
// 21. Chat
// ============================================================================

#[derive(Debug, Args, Clone, PartialEq, Eq)]
pub struct ChatArgs {
    /// Natural-language prompt for ColimaUI's DevOps AI agent
    #[arg(required = true, trailing_var_arg = true, allow_hyphen_values = true)]
    pub message: Vec<String>,

    /// Conversation session ID (isolated from Desktop UI thread by default)
    #[arg(long, default_value = DEFAULT_A2A_SESSION_ID)]
    pub session: String,

    /// Allow mutating DevOps actions (start/stop/restart/pull/compose)
    #[arg(short = 'y', long = "yes")]
    pub yes: bool,

    /// Mirror this chat turn into the open ColimaUI Desktop AI panel
    #[arg(long = "mirror-ui")]
    pub mirror_ui: bool,

    /// Override AI provider (`ollama-local`, `ollama-cloud`, `openai`, `openai-compatible`, `anthropic`, `gemini`)
    #[arg(long)]
    pub provider: Option<String>,

    /// Override AI model name
    #[arg(long)]
    pub model: Option<String>,
}

// ============================================================================
// Legacy System Command (100% backward-compatible with `colimaui system ...`)
// ============================================================================

#[derive(Debug, Subcommand)]
pub enum SystemCommand {
    /// Check installed Colima / Docker / Lima / Kubectl versions
    Check,

    /// Show container runtime disk usage (`docker system df`)
    Df,

    /// Prune unused containers, networks, and dangling images
    Prune {
        #[arg(short, long)]
        all: bool,
    },

    /// Inspect host CPU, RAM, disk, and architecture specs
    Specs,

    /// Show CLI version and active container runtime
    Info,
}
