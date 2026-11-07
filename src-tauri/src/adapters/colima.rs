//! Colima adapter — implements `VMManager` for Colima VM instances.
//!
//! Wraps the `colima` CLI and reads `~/.colima/` filesystem for fast listing.

use async_trait::async_trait;
use super::traits::*;

/// Colima VM manager adapter.
pub struct ColimaAdapter;

impl ColimaAdapter {
    pub fn new() -> Self {
        Self
    }

    /// Get a pre-configured `colima` Command with correct PATH.
    fn colima_cmd() -> std::process::Command {
        let resolved = crate::path_util::resolve_binary("colima");
        let mut cmd = std::process::Command::new(&resolved);
        crate::path_util::apply_path_to_cmd(&mut cmd);
        cmd
    }
}

impl Default for ColimaAdapter {
    fn default() -> Self {
        Self::new()
    }
}

/// Build `colima start` CLI arguments, omitting `--runtime`/`--cpu`/`--memory`/`--disk`
/// unless explicitly specified so existing `~/.colima/<profile>/colima.yaml` is preserved.
pub fn build_colima_start_args(config: &VMConfig) -> Vec<String> {
    let mut args = vec!["start".to_string()];

    if config.profile != "default" && !config.profile.is_empty() {
        args.push("--profile".into());
        args.push(config.profile.clone());
    }

    if let Some(runtime) = config
        .runtime
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        args.push("--runtime".into());
        args.push(runtime.to_string());
    }
    if let Some(cpus) = config.cpus.filter(|&c| c > 0) {
        args.push("--cpu".into());
        args.push(cpus.to_string());
    }
    if let Some(memory) = config.memory.filter(|&m| m > 0) {
        args.push("--memory".into());
        args.push(memory.to_string());
    }
    if let Some(disk) = config.disk.filter(|&d| d > 0) {
        args.push("--disk".into());
        args.push(disk.to_string());
    }

    if !config.vm_type.is_empty() {
        args.push("--vm-type".into());
        args.push(config.vm_type.clone());
    }
    if !config.arch.is_empty() {
        args.push("--arch".into());
        args.push(config.arch.clone());
    }
    if !config.mount_type.is_empty() {
        args.push("--mount-type".into());
        args.push(config.mount_type.clone());
    }
    for mount in &config.mounts {
        args.push("--mount".into());
        args.push(mount.clone());
    }
    for dns in &config.dns {
        args.push("--dns".into());
        args.push(dns.clone());
    }
    if config.network_address {
        args.push("--network-address".into());
    }
    if config.kubernetes {
        args.push("--kubernetes".into());
        if !config.kubernetes_version.is_empty() {
            args.push("--kubernetes-version".into());
            args.push(config.kubernetes_version.clone());
        }
    }

    args
}

#[async_trait]
impl VMManager for ColimaAdapter {
    fn name(&self) -> &str {
        "colima"
    }

    async fn list(&self) -> Result<Vec<VMInstance>, AdapterError> {
        // Use the fast filesystem reader (shared with API server and Tauri commands)
        let instances = crate::instance_reader::list_instances_fast();
        Ok(instances
            .into_iter()
            .map(|i| VMInstance {
                name: i.name,
                status: i.status,
                arch: i.arch,
                cpus: i.cpus,
                memory: i.memory,
                disk: i.disk,
                runtime: i.runtime,
                address: i.address,
                kubernetes: i.kubernetes,
            })
            .collect())
    }

    async fn start(&self, config: VMConfig) -> Result<String, AdapterError> {
        let profile = if config.profile.is_empty() {
            "default".to_string()
        } else {
            config.profile.clone()
        };
        tokio::task::spawn_blocking(move || {
            let args = build_colima_start_args(&config);
            let args_ref: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
            let output = Self::colima_cmd()
                .args(&args_ref)
                .output()
                .map_err(|e| AdapterError::new("colima", format!("Failed to start: {}", e)))?;

            if !output.status.success() {
                return Err(AdapterError::new(
                    "colima",
                    format!("start failed: {}", String::from_utf8_lossy(&output.stderr)),
                ));
            }

            Ok(format!("Instance '{}' started successfully", profile))
        })
        .await
        .map_err(|e| AdapterError::new("colima", format!("Task join error: {}", e)))?
    }

    async fn stop(&self, profile: &str, force: bool) -> Result<String, AdapterError> {
        let profile = profile.to_string();
        tokio::task::spawn_blocking(move || {
            let mut args = vec!["stop".to_string()];

            if profile != "default" && !profile.is_empty() {
                args.push("--profile".into());
                args.push(profile.clone());
            }
            if force {
                args.push("--force".into());
            }

            let args_ref: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
            let output = Self::colima_cmd()
                .args(&args_ref)
                .output()
                .map_err(|e| AdapterError::new("colima", format!("Failed to stop: {}", e)))?;

            if !output.status.success() {
                return Err(AdapterError::new("colima", format!("stop failed: {}", String::from_utf8_lossy(&output.stderr))));
            }

            Ok(format!("Instance '{}' stopped", profile))
        })
        .await
        .map_err(|e| AdapterError::new("colima", format!("Task join error: {}", e)))?
    }

    async fn delete(&self, profile: &str, force: bool) -> Result<String, AdapterError> {
        let profile = profile.to_string();
        tokio::task::spawn_blocking(move || {
            let mut args = vec!["delete".to_string()];

            if profile != "default" && !profile.is_empty() {
                args.push("--profile".into());
                args.push(profile.clone());
            }
            if force {
                args.push("--force".into());
            }

            let args_ref: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
            let output = Self::colima_cmd()
                .args(&args_ref)
                .output()
                .map_err(|e| AdapterError::new("colima", format!("Failed to delete: {}", e)))?;

            if !output.status.success() {
                return Err(AdapterError::new("colima", format!("delete failed: {}", String::from_utf8_lossy(&output.stderr))));
            }

            Ok(format!("Instance '{}' deleted", profile))
        })
        .await
        .map_err(|e| AdapterError::new("colima", format!("Task join error: {}", e)))?
    }

    async fn status(&self, profile: &str) -> Result<VMStatus, AdapterError> {
        let profile = profile.to_string();
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            tokio::task::spawn_blocking(move || {
                let mut args = vec!["status", "--json", "--extended"];
                let profile_flag;
                if profile != "default" && !profile.is_empty() {
                    profile_flag = profile.clone();
                    args.push("--profile");
                    args.push(&profile_flag);
                }

                let output = Self::colima_cmd()
                    .args(&args)
                    .output()
                    .map_err(|e| AdapterError::new("colima", format!("Failed to get status: {}", e)))?;

                if !output.status.success() {
                    return Err(AdapterError::new("colima", format!("status failed: {}", String::from_utf8_lossy(&output.stderr))));
                }

                let stdout = String::from_utf8_lossy(&output.stdout);
                let status: VMStatus = serde_json::from_str(&stdout)
                    .map_err(|e| AdapterError::new("colima", format!("Failed to parse status: {}", e)))?;
                Ok(status)
            }),
        )
        .await;

        match result {
            Ok(Ok(status)) => status,
            Ok(Err(e)) => Err(AdapterError::new("colima", format!("Task join error: {}", e))),
            Err(_) => Err(AdapterError::new("colima", "status timed out (daemon may be unresponsive)")),
        }
    }

    async fn ssh_command(&self, profile: &str) -> Result<Vec<String>, AdapterError> {
        let mut args = vec!["ssh".to_string()];
        if profile != "default" && !profile.is_empty() {
            args.push("--profile".into());
            args.push(profile.to_string());
        }
        Ok(args)
    }

    async fn passthrough(&self, args: &[String]) -> Result<(), AdapterError> {
        let mut cmd = Self::colima_cmd();
        cmd.args(args);
        
        // Inherit IO so terminal interactive features work
        cmd.stdin(std::process::Stdio::inherit())
           .stdout(std::process::Stdio::inherit())
           .stderr(std::process::Stdio::inherit());
        
        match cmd.status() {
            Ok(status) => {
                if !status.success() {
                    let code = status.code().unwrap_or(1);
                    std::process::exit(code);
                }
                Ok(())
            },
            Err(e) => {
                if e.kind() == std::io::ErrorKind::NotFound {
                    Err(AdapterError::new("colima", "Command not found. Is colima installed?"))
                } else {
                    Err(AdapterError::new("colima", format!("Failed to execute passthrough: {}", e)))
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_colima_start_args_omits_unspecified_resource_flags() {
        let default_cfg = VMConfig {
            profile: "default".to_string(),
            ..Default::default()
        };
        assert_eq!(build_colima_start_args(&default_cfg), vec!["start"]);

        let named_cfg = VMConfig {
            profile: "dev".to_string(),
            ..Default::default()
        };
        assert_eq!(
            build_colima_start_args(&named_cfg),
            vec!["start", "--profile", "dev"]
        );
    }

    #[test]
    fn build_colima_start_args_appends_explicit_overrides() {
        let cfg = VMConfig {
            profile: "dev".to_string(),
            runtime: Some("containerd".to_string()),
            cpus: Some(4),
            memory: Some(8),
            disk: Some(100),
            ..Default::default()
        };
        assert_eq!(
            build_colima_start_args(&cfg),
            vec![
                "start",
                "--profile",
                "dev",
                "--runtime",
                "containerd",
                "--cpu",
                "4",
                "--memory",
                "8",
                "--disk",
                "100",
            ]
        );
    }
}
