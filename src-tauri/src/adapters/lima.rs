use async_trait::async_trait;
use std::process::Command;
use crate::adapters::traits::{AdapterError, VMConfig, VMInstance, VMManager, VMStatus};
use serde_json::Value;

pub struct LimaAdapter;

impl LimaAdapter {
    pub fn new() -> Self {
        Self
    }

    fn limactl_cmd() -> Command {
        let mut cmd = Command::new(crate::path_util::resolve_binary("limactl"));
        crate::path_util::apply_path_to_cmd(&mut cmd);
        cmd
    }
}

impl Default for LimaAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl VMManager for LimaAdapter {
    fn name(&self) -> &str {
        "lima"
    }

    async fn list(&self) -> Result<Vec<VMInstance>, AdapterError> {
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            tokio::task::spawn_blocking(move || {
                let output = Self::limactl_cmd()
                    .args(["list", "--json"])
                    .output()
                    .map_err(|e| AdapterError::new("lima", format!("Failed to list VMs: {}", e)))?;

                if !output.status.success() {
                    return Err(AdapterError::new("lima", format!("list failed: {}", String::from_utf8_lossy(&output.stderr))));
                }

                let stdout = String::from_utf8_lossy(&output.stdout);
                let mut instances = Vec::new();
                for line in stdout.lines() {
                    if line.trim().is_empty() {
                        continue;
                    }
                    if let Ok(v) = serde_json::from_str::<Value>(line) {
                        let name = v.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        let status = v.get("status").and_then(|v| v.as_str()).unwrap_or("Stopped").to_string();
                        
                        instances.push(VMInstance {
                            name,
                            status,
                            runtime: "lima".to_string(),
                            arch: v.get("arch").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                            kubernetes: false,
                            cpus: v.get("cpus").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
                            memory: v.get("memory").and_then(|v| v.as_u64()).unwrap_or(0),
                            disk: v.get("disk").and_then(|v| v.as_u64()).unwrap_or(0),
                            address: "".to_string(),
                        });
                    }
                }
                Ok(instances)
            }),
        )
        .await;

        match result {
            Ok(Ok(Ok(instances))) => Ok(instances),
            Ok(Ok(Err(e))) => Err(e),
            Ok(Err(e)) => Err(AdapterError::new("lima", format!("Task failed: {}", e))),
            Err(_) => Err(AdapterError::new("lima", "list timed out")),
        }
    }

    async fn start(&self, config: VMConfig) -> Result<String, AdapterError> {
        let profile = config.profile.clone();
        
        tokio::task::spawn_blocking(move || {
            let args = vec!["start".to_string(), profile.clone()];
            let output = Self::limactl_cmd()
                .args(&args)
                .output()
                .map_err(|e| AdapterError::new("lima", format!("Failed to start: {}", e)))?;

            if !output.status.success() {
                return Err(AdapterError::new("lima", format!("start failed: {}", String::from_utf8_lossy(&output.stderr))));
            }
            Ok(format!("Lima instance '{}' started", profile))
        }).await.map_err(|e| AdapterError::new("lima", format!("Task join error: {}", e)))?
    }

    async fn stop(&self, profile: &str, force: bool) -> Result<String, AdapterError> {
        let profile_clone = profile.to_string();
        tokio::task::spawn_blocking(move || {
            let mut args = vec!["stop".to_string()];
            if force {
                args.push("-f".into());
            }
            args.push(profile_clone.clone());
            
            let output = Self::limactl_cmd()
                .args(&args)
                .output()
                .map_err(|e| AdapterError::new("lima", format!("Failed to stop: {}", e)))?;

            if !output.status.success() {
                return Err(AdapterError::new("lima", format!("stop failed: {}", String::from_utf8_lossy(&output.stderr))));
            }
            Ok(format!("Lima instance '{}' stopped", profile_clone))
        }).await.map_err(|e| AdapterError::new("lima", format!("Task join error: {}", e)))?
    }

    async fn delete(&self, profile: &str, force: bool) -> Result<String, AdapterError> {
        let profile_clone = profile.to_string();
        tokio::task::spawn_blocking(move || {
            let mut args = vec!["delete".to_string()];
            if force {
                args.push("-f".into());
            }
            args.push(profile_clone.clone());
            
            let output = Self::limactl_cmd()
                .args(&args)
                .output()
                .map_err(|e| AdapterError::new("lima", format!("Failed to delete: {}", e)))?;

            if !output.status.success() {
                return Err(AdapterError::new("lima", format!("delete failed: {}", String::from_utf8_lossy(&output.stderr))));
            }
            Ok(format!("Lima instance '{}' deleted", profile_clone))
        }).await.map_err(|e| AdapterError::new("lima", format!("Task join error: {}", e)))?
    }

    async fn status(&self, profile: &str) -> Result<VMStatus, AdapterError> {
        // Fallback or basic implementation, since limactl list already gives basic status
        let instances = self.list().await?;
        for inst in instances {
            if inst.name == profile {
                return Ok(VMStatus {
                    profile: inst.name,
                    status: inst.status,
                    arch: inst.arch,
                    runtime: inst.runtime,
                    cpu_usage: "".to_string(),
                    memory_usage: "".to_string(),
                    disk_usage: "".to_string(),
                    address: "".to_string(),
                });
            }
        }
        Err(AdapterError::new("lima", format!("Instance {} not found", profile)))
    }

    async fn ssh_command(&self, profile: &str) -> Result<Vec<String>, AdapterError> {
        let mut args = vec!["shell".to_string()];
        if profile != "default" && !profile.is_empty() {
            args.push(profile.to_string());
        }
        Ok(args)
    }

    async fn passthrough(&self, args: &[String]) -> Result<(), AdapterError> {
        let mut cmd = Self::limactl_cmd();
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
                    Err(AdapterError::new("lima", "Command not found. Is limactl installed?"))
                } else {
                    Err(AdapterError::new("lima", format!("Failed to execute passthrough: {}", e)))
                }
            }
        }
    }
}
