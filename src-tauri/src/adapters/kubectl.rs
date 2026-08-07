use async_trait::async_trait;
use std::process::Command;
use crate::adapters::traits::{AdapterError, Orchestrator};

pub struct KubectlAdapter;

impl KubectlAdapter {
    pub fn new() -> Self {
        Self
    }

    fn kubectl_cmd() -> Command {
        let mut cmd = Command::new(crate::path_util::resolve_binary("kubectl"));
        crate::path_util::apply_path_to_cmd(&mut cmd);
        cmd
    }
}

#[async_trait]
impl Orchestrator for KubectlAdapter {
    fn name(&self) -> &str {
        "kubectl"
    }

    async fn passthrough(&self, args: &[String]) -> Result<(), AdapterError> {
        let mut cmd = Self::kubectl_cmd();
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
                    Err(AdapterError::new("kubectl", "Command not found. Is kubectl installed?"))
                } else {
                    Err(AdapterError::new("kubectl", format!("Failed to execute passthrough: {}", e)))
                }
            }
        }
    }
}
