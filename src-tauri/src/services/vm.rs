//! VM service — high-level business logic layer for VM operations.
//!
//! Wraps a `VMManager` adapter, providing a unified API for all consumers.

use crate::adapters::traits::*;
use crate::adapters::colima::ColimaAdapter;

/// VM service wrapping a VM manager adapter.
pub struct VMService {
    manager: Box<dyn VMManager>,
}

impl VMService {
    /// Create a service with a specific VM manager adapter.
    pub fn new(manager: Box<dyn VMManager>) -> Self {
        Self { manager }
    }

    /// Create a service using the Colima adapter (default).
    pub fn colima() -> Self {
        Self {
            manager: Box::new(ColimaAdapter::new()),
        }
    }

    /// Get the name of the active VM manager.
    pub fn manager_name(&self) -> &str {
        self.manager.name()
    }

    // --- Delegate all operations to the adapter ---

    pub async fn list(&self) -> Result<Vec<VMInstance>, AdapterError> {
        self.manager.list().await
    }

    pub async fn start(&self, config: VMConfig) -> Result<String, AdapterError> {
        self.manager.start(config).await
    }

    pub async fn stop(&self, profile: &str, force: bool) -> Result<String, AdapterError> {
        self.manager.stop(profile, force).await
    }

    pub async fn delete(&self, profile: &str, force: bool) -> Result<String, AdapterError> {
        self.manager.delete(profile, force).await
    }

    pub async fn status(&self, profile: &str) -> Result<VMStatus, AdapterError> {
        self.manager.status(profile).await
    }

    pub async fn ssh_command(&self, profile: &str) -> Result<Vec<String>, AdapterError> {
        self.manager.ssh_command(profile).await
    }

    pub async fn passthrough(&self, args: &[String]) -> Result<(), AdapterError> {
        self.manager.passthrough(args).await
    }
}
