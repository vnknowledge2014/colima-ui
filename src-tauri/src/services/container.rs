//! Container service — high-level business logic layer for container operations.
//!
//! Wraps a `ContainerRuntime` adapter, providing auto-detection of the active
//! runtime and a unified API for all consumers (CLI, UI, API, Agent).

use crate::adapters::traits::*;
use crate::adapters::docker::DockerAdapter;

/// Container service wrapping a runtime adapter.
pub struct ContainerService {
    runtime: Box<dyn ContainerRuntime>,
}

impl ContainerService {
    /// Create a service with a specific runtime adapter.
    pub fn new(runtime: Box<dyn ContainerRuntime>) -> Self {
        Self { runtime }
    }

    /// Auto-detect the active container runtime.
    ///
    /// Determines whether to use Docker or nerdctl based on Colima status.
    pub fn auto_detect() -> Self {
        let (runtime_name, _profile_name, _host_sock) = crate::commands::runtime::detect_runtime();
        let runtime: Box<dyn ContainerRuntime> = if runtime_name == "containerd" {
            Box::new(crate::adapters::nerdctl::NerdctlAdapter::new())
        } else {
            Box::new(DockerAdapter::auto_detect())
        };

        Self { runtime }
    }

    /// Get the name of the active runtime.
    pub fn runtime_name(&self) -> &str {
        self.runtime.name()
    }

    // --- Delegate all operations to the adapter ---

    pub async fn list_containers(&self, all: bool) -> Result<Vec<Container>, AdapterError> {
        self.runtime.list_containers(all).await
    }

    pub async fn start_container(&self, id: &str) -> Result<String, AdapterError> {
        self.runtime.start_container(id).await
    }

    pub async fn stop_container(&self, id: &str) -> Result<String, AdapterError> {
        self.runtime.stop_container(id).await
    }

    pub async fn restart_container(&self, id: &str) -> Result<String, AdapterError> {
        self.runtime.restart_container(id).await
    }

    pub async fn remove_container(&self, id: &str, force: bool) -> Result<String, AdapterError> {
        self.runtime.remove_container(id, force).await
    }

    pub async fn container_logs(&self, id: &str, lines: u32) -> Result<String, AdapterError> {
        self.runtime.container_logs(id, lines).await
    }

    pub async fn run_container(&self, config: RunConfig) -> Result<String, AdapterError> {
        self.runtime.run_container(config).await
    }

    pub async fn exec(&self, id: &str, command: &str) -> Result<String, AdapterError> {
        self.runtime.exec(id, command).await
    }

    pub async fn list_images(&self) -> Result<Vec<Image>, AdapterError> {
        self.runtime.list_images().await
    }

    pub async fn pull_image(&self, name: &str) -> Result<String, AdapterError> {
        self.runtime.pull_image(name).await
    }

    pub async fn remove_image(&self, id: &str, force: bool) -> Result<String, AdapterError> {
        self.runtime.remove_image(id, force).await
    }

    pub async fn list_volumes(&self) -> Result<Vec<Volume>, AdapterError> {
        self.runtime.list_volumes().await
    }

    pub async fn list_networks(&self) -> Result<Vec<Network>, AdapterError> {
        self.runtime.list_networks().await
    }

    pub async fn system_df(&self) -> Result<String, AdapterError> {
        self.runtime.system_df().await
    }

    pub async fn system_prune(&self, all: bool) -> Result<String, AdapterError> {
        self.runtime.system_prune(all).await
    }

    pub async fn passthrough(&self, args: &[String]) -> Result<(), AdapterError> {
        self.runtime.passthrough(args).await
    }
}
