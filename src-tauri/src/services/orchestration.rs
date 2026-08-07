use std::sync::Arc;
use tokio::sync::RwLock;

use crate::adapters::traits::{Orchestrator, AdapterError};
use crate::adapters::kubectl::KubectlAdapter;

pub struct OrchestrationService {
    runtime: Box<dyn Orchestrator>,
    #[allow(dead_code)]
    state: Arc<RwLock<()>>,
}

impl OrchestrationService {
    pub async fn auto_detect() -> Self {
        // In the future, we might auto-detect between kubectl, helm, kind, etc.
        // For now, default to kubectl.
        let runtime: Box<dyn Orchestrator> = Box::new(KubectlAdapter::new());
        
        Self {
            runtime,
            state: Arc::new(RwLock::new(())),
        }
    }
    
    pub async fn passthrough(&self, args: &[String]) -> Result<(), AdapterError> {
        self.runtime.passthrough(args).await
    }
}
