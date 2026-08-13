use async_trait::async_trait;
use std::process::Command;
use crate::adapters::traits::{AdapterError, ComposeManager};

pub struct DockerComposeAdapter;

impl DockerComposeAdapter {
    pub fn new() -> Self {
        Self
    }

    fn compose_cmd() -> Command {
        // Technically this could be `docker compose` or `docker-compose`.
        // We'll use `docker-compose` as the fallback or assume modern `docker compose`.
        // For simplicity, let's use our existing runtime resolver.
        let mut cmd = crate::commands::runtime::get_runtime_cmd();
        cmd.arg("compose");
        cmd
    }
}

impl Default for DockerComposeAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ComposeManager for DockerComposeAdapter {
    fn name(&self) -> &str {
        "docker-compose"
    }

    async fn passthrough(&self, args: &[String]) -> Result<(), AdapterError> {
        let mut cmd = Self::compose_cmd();
        cmd.args(args);
        
        cmd.stdin(std::process::Stdio::inherit())
           .stdout(std::process::Stdio::inherit())
           .stderr(std::process::Stdio::inherit());
        
        match cmd.status() {
            Ok(status) => {
                if !status.success() {
                    std::process::exit(status.code().unwrap_or(1));
                }
                Ok(())
            },
            Err(e) => {
                if e.kind() == std::io::ErrorKind::NotFound {
                    Err(AdapterError::new("compose", "Command not found. Is docker installed?"))
                } else {
                    Err(AdapterError::new("compose", format!("Failed to execute passthrough: {}", e)))
                }
            }
        }
    }
}
