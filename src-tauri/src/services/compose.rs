use std::sync::Arc;
use tokio::sync::RwLock;

use crate::adapters::traits::{ComposeManager, AdapterError};
use crate::adapters::compose::DockerComposeAdapter;

pub struct ComposeService {
    runtime: Box<dyn ComposeManager>,
    #[allow(dead_code)]
    state: Arc<RwLock<()>>,
}

impl ComposeService {
    pub async fn auto_detect() -> Self {
        // In the future, we could detect if podman-compose or docker-compose is available
        // For now, default to DockerComposeAdapter which resolves via get_runtime_cmd().
        let runtime: Box<dyn ComposeManager> = Box::new(DockerComposeAdapter::new());
        
        Self {
            runtime,
            state: Arc::new(RwLock::new(())),
        }
    }
    
    pub async fn passthrough(&self, args: &[String]) -> Result<(), AdapterError> {
        self.runtime.passthrough(args).await
    }
}
