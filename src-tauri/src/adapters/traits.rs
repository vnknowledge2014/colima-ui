//! Core adapter traits for the unified DevOps wrapper architecture.
//!
//! All tool-specific adapters implement these traits, providing a common
//! interface that CLI, UI, and API consumers can use interchangeably.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::fmt;

// ============================================================
// Error Type
// ============================================================

/// Unified error type for all adapter operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdapterError {
    /// The adapter that produced the error (e.g., "docker", "colima")
    pub adapter: String,
    /// Human-readable error message
    pub message: String,
}

impl fmt::Display for AdapterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.adapter, self.message)
    }
}

impl std::error::Error for AdapterError {}

impl AdapterError {
    pub fn new(adapter: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            adapter: adapter.into(),
            message: message.into(),
        }
    }
}

// ============================================================
// Unified Data Types
// ============================================================

/// A container across any runtime (Docker, Podman, nerdctl).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Container {
    pub id: String,
    pub name: String,
    pub image: String,
    pub status: String,
    pub state: String,
    pub ports: String,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub command: String,
}

/// A container image.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Image {
    pub id: String,
    pub repository: String,
    pub tag: String,
    pub size: String,
    #[serde(default)]
    pub created_at: String,
}

/// A volume.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Volume {
    pub name: String,
    pub driver: String,
    #[serde(default)]
    pub mountpoint: String,
    #[serde(default)]
    pub scope: String,
}

/// A network.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Network {
    pub id: String,
    pub name: String,
    pub driver: String,
    #[serde(default)]
    pub scope: String,
}

/// Configuration for running a new container.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunConfig {
    pub image: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub ports: Vec<String>,
    #[serde(default)]
    pub env_vars: Vec<String>,
    #[serde(default)]
    pub volumes: Vec<String>,
    #[serde(default)]
    pub detach: bool,
    #[serde(default)]
    pub remove_on_exit: bool,
    #[serde(default)]
    pub extra_args: Vec<String>,
}

/// Prune options.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PruneOptions {
    pub all: bool,
}

/// Prune result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PruneResult {
    pub output: String,
}

/// A VM instance (Colima, Lima, etc.).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VMInstance {
    pub name: String,
    pub status: String,
    #[serde(default)]
    pub arch: String,
    #[serde(default)]
    pub cpus: u32,
    #[serde(default)]
    pub memory: u64,
    #[serde(default)]
    pub disk: u64,
    #[serde(default)]
    pub runtime: String,
    #[serde(default)]
    pub address: String,
    #[serde(default)]
    pub kubernetes: bool,
}

/// VM start configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VMConfig {
    pub profile: String,
    pub runtime: String,
    pub cpus: u32,
    pub memory: u32,
    pub disk: u32,
    #[serde(default)]
    pub vm_type: String,
    #[serde(default)]
    pub kubernetes: bool,
    #[serde(default)]
    pub kubernetes_version: String,
    #[serde(default)]
    pub arch: String,
    #[serde(default)]
    pub mount_type: String,
    #[serde(default)]
    pub mounts: Vec<String>,
    #[serde(default)]
    pub dns: Vec<String>,
    #[serde(default)]
    pub network_address: bool,
}

/// Detailed VM status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VMStatus {
    pub profile: String,
    pub status: String,
    pub arch: String,
    pub runtime: String,
    #[serde(default)]
    pub cpu_usage: String,
    #[serde(default)]
    pub memory_usage: String,
    #[serde(default)]
    pub disk_usage: String,
    #[serde(default)]
    pub address: String,
}

// ============================================================
// Traits
// ============================================================

/// Trait for container runtime operations (Docker, Podman, nerdctl).
#[async_trait]
pub trait ContainerRuntime: Send + Sync {
    /// Runtime name (e.g., "docker", "podman", "nerdctl")
    fn name(&self) -> &str;

    // --- Container lifecycle ---
    async fn list_containers(&self, all: bool) -> Result<Vec<Container>, AdapterError>;
    async fn start_container(&self, id: &str) -> Result<String, AdapterError>;
    async fn stop_container(&self, id: &str) -> Result<String, AdapterError>;
    async fn restart_container(&self, id: &str) -> Result<String, AdapterError>;
    async fn remove_container(&self, id: &str, force: bool) -> Result<String, AdapterError>;
    async fn container_logs(&self, id: &str, lines: u32) -> Result<String, AdapterError>;
    async fn container_stats(&self, id: &str) -> Result<String, AdapterError>;
    async fn run_container(&self, config: RunConfig) -> Result<String, AdapterError>;
    async fn exec(&self, id: &str, command: &str) -> Result<String, AdapterError>;
    async fn inspect_container(&self, id: &str) -> Result<String, AdapterError>;

    // --- Image management ---
    async fn list_images(&self) -> Result<Vec<Image>, AdapterError>;
    async fn pull_image(&self, name: &str) -> Result<String, AdapterError>;
    async fn remove_image(&self, id: &str, force: bool) -> Result<String, AdapterError>;
    async fn prune_images(&self) -> Result<String, AdapterError>;

    // --- Volume management ---
    async fn list_volumes(&self) -> Result<Vec<Volume>, AdapterError>;
    async fn create_volume(&self, name: &str) -> Result<String, AdapterError>;
    async fn remove_volume(&self, name: &str) -> Result<String, AdapterError>;

    // --- Network management ---
    async fn list_networks(&self) -> Result<Vec<Network>, AdapterError>;
    async fn create_network(&self, name: &str, driver: &str) -> Result<String, AdapterError>;
    async fn remove_network(&self, name: &str) -> Result<String, AdapterError>;

    // --- System ---
    async fn system_df(&self) -> Result<String, AdapterError>;
    async fn system_prune(&self, all: bool) -> Result<String, AdapterError>;

    // --- Universal Passthrough ---
    /// Execute an arbitrary command directly against the underlying CLI tool
    /// inheriting stdin/stdout/stderr for interactive terminal usage.
    async fn passthrough(&self, args: &[String]) -> Result<(), AdapterError>;
}

/// Trait for VM manager operations (Colima, Lima, Vagrant, etc.).
#[async_trait]
pub trait VMManager: Send + Sync {
    /// Manager name (e.g., "colima", "lima", "vagrant")
    fn name(&self) -> &str;

    async fn list(&self) -> Result<Vec<VMInstance>, AdapterError>;
    async fn start(&self, config: VMConfig) -> Result<String, AdapterError>;
    async fn stop(&self, profile: &str, force: bool) -> Result<String, AdapterError>;
    async fn delete(&self, profile: &str, force: bool) -> Result<String, AdapterError>;
    async fn status(&self, profile: &str) -> Result<VMStatus, AdapterError>;
    async fn ssh_command(&self, profile: &str) -> Result<Vec<String>, AdapterError>;

    // --- Universal Passthrough ---
    async fn passthrough(&self, args: &[String]) -> Result<(), AdapterError>;
}

/// Trait for Orchestrator operations (Kubernetes/kubectl, Helm, Kind).
#[async_trait]
pub trait Orchestrator: Send + Sync {
    /// Orchestrator name (e.g., "kubectl")
    fn name(&self) -> &str;

    // --- Universal Passthrough ---
    async fn passthrough(&self, args: &[String]) -> Result<(), AdapterError>;
}

/// Trait for Compose manager operations (docker-compose, podman-compose).
#[async_trait]
pub trait ComposeManager: Send + Sync {
    /// Manager name (e.g., "docker-compose")
    fn name(&self) -> &str;

    // --- Universal Passthrough ---
    async fn passthrough(&self, args: &[String]) -> Result<(), AdapterError>;
}
