//! Docker adapter — implements `ContainerRuntime` using Docker CLI + Bollard SDK.
//!
//! Strategy: Try Bollard SDK first (faster, uses socket directly), fall back to
//! Docker CLI for operations not well-supported by Bollard or when SDK is unavailable.

use async_trait::async_trait;
use super::traits::*;

/// Docker container runtime adapter.
///
/// Uses Bollard SDK for listing (fast) and Docker CLI for lifecycle operations.
pub struct DockerAdapter {
    /// Docker host socket URL (e.g., "unix:///Users/mike/.colima/default/docker.sock")
    docker_host: Option<String>,
}

impl DockerAdapter {
    pub fn new(docker_host: Option<String>) -> Self {
        Self { docker_host }
    }

    /// Auto-detect Docker host from running Colima instance.
    pub fn auto_detect() -> Self {
        let host = crate::path_util::detect_docker_host().map(|(h, _)| h);
        Self { docker_host: host }
    }

    /// Get a Command pre-configured with the correct runtime binary and PATH.
    fn docker_cmd(&self) -> std::process::Command {
        let runtime_cmd = crate::commands::runtime::get_runtime_cmd();
        runtime_cmd
    }

    /// Execute a Docker CLI command on a blocking thread pool with timeout.
    async fn cli_output(&self, args: Vec<String>) -> Result<std::process::Output, AdapterError> {
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            tokio::task::spawn_blocking(move || {
                crate::commands::runtime::get_runtime_cmd()
                    .args(args.iter().map(|s| s.as_str()).collect::<Vec<&str>>())
                    .output()
                    .map_err(|e| AdapterError::new("docker", format!("Failed to run: {}", e)))
            }),
        )
        .await;

        match result {
            Ok(Ok(output)) => output,
            Ok(Err(e)) => Err(AdapterError::new("docker", format!("Task error: {}", e))),
            Err(_) => Err(AdapterError::new("docker", "Command timed out (daemon may be unresponsive)")),
        }
    }

    /// Parse NDJSON output from Docker CLI (each line is a JSON object).
    fn parse_ndjson<T: serde::de::DeserializeOwned>(stdout: &str) -> Vec<T> {
        stdout
            .lines()
            .filter(|l| !l.trim().is_empty())
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect()
    }
}

#[async_trait]
impl ContainerRuntime for DockerAdapter {
    fn name(&self) -> &str {
        "docker"
    }

    async fn list_containers(&self, all: bool) -> Result<Vec<Container>, AdapterError> {
        let mut args = vec!["ps".to_string(), "--format".into(), "json".into(), "--no-trunc".into()];
        if all {
            args.push("-a".into());
        }
        let output = self.cli_output(args).await?;
        if !output.status.success() {
            return Err(AdapterError::new("docker", String::from_utf8_lossy(&output.stderr).to_string()));
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        let containers: Vec<Container> = stdout
            .lines()
            .filter(|l| !l.trim().is_empty())
            .filter_map(|l| {
                let v: serde_json::Value = serde_json::from_str(l).ok()?;
                Some(Container {
                    id: v.get("ID").or(v.get("Id")).and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    name: v.get("Names").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    image: v.get("Image").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    status: v.get("Status").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    state: v.get("State").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    ports: v.get("Ports").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    created_at: v.get("CreatedAt").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    command: v.get("Command").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                })
            })
            .collect();
        Ok(containers)
    }

    async fn start_container(&self, id: &str) -> Result<String, AdapterError> {
        let output = self.cli_output(vec!["start".into(), id.to_string()]).await?;
        if !output.status.success() {
            return Err(AdapterError::new("docker", format!("start failed: {}", String::from_utf8_lossy(&output.stderr))));
        }
        Ok(format!("Container {} started", id))
    }

    async fn stop_container(&self, id: &str) -> Result<String, AdapterError> {
        let output = self.cli_output(vec!["stop".into(), id.to_string()]).await?;
        if !output.status.success() {
            return Err(AdapterError::new("docker", format!("stop failed: {}", String::from_utf8_lossy(&output.stderr))));
        }
        Ok(format!("Container {} stopped", id))
    }

    async fn restart_container(&self, id: &str) -> Result<String, AdapterError> {
        let output = self.cli_output(vec!["restart".into(), id.to_string()]).await?;
        if !output.status.success() {
            return Err(AdapterError::new("docker", format!("restart failed: {}", String::from_utf8_lossy(&output.stderr))));
        }
        Ok(format!("Container {} restarted", id))
    }

    async fn remove_container(&self, id: &str, force: bool) -> Result<String, AdapterError> {
        let mut args = vec!["rm".to_string()];
        if force { args.push("-f".into()); }
        args.push(id.to_string());
        let output = self.cli_output(args).await?;
        if !output.status.success() {
            return Err(AdapterError::new("docker", format!("rm failed: {}", String::from_utf8_lossy(&output.stderr))));
        }
        Ok(format!("Container {} removed", id))
    }

    async fn container_logs(&self, id: &str, lines: u32) -> Result<String, AdapterError> {
        let output = self.cli_output(vec![
            "logs".into(), "--tail".into(), lines.to_string(), "--timestamps".into(), id.to_string()
        ]).await?;
        if !output.status.success() {
            return Err(AdapterError::new("docker", format!("logs failed: {}", String::from_utf8_lossy(&output.stderr))));
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        Ok(if stdout.is_empty() { stderr.to_string() } else { stdout.to_string() })
    }

    async fn container_stats(&self, id: &str) -> Result<String, AdapterError> {
        let output = self.cli_output(vec![
            "stats".into(), "--no-stream".into(), "--format".into(), "json".into(), id.to_string()
        ]).await?;
        if !output.status.success() {
            return Err(AdapterError::new("docker", format!("stats failed: {}", String::from_utf8_lossy(&output.stderr))));
        }
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    async fn run_container(&self, config: RunConfig) -> Result<String, AdapterError> {
        let mut args = vec!["run".to_string()];
        if config.detach { args.push("-d".into()); }
        if config.remove_on_exit { args.push("--rm".into()); }
        if !config.name.is_empty() { args.push("--name".into()); args.push(config.name); }
        for port in &config.ports { args.push("-p".into()); args.push(port.clone()); }
        for env in &config.env_vars { args.push("-e".into()); args.push(env.clone()); }
        for vol in &config.volumes { args.push("-v".into()); args.push(vol.clone()); }
        for extra in &config.extra_args { args.push(extra.clone()); }
        args.push(config.image);

        let output = self.cli_output(args).await?;
        if !output.status.success() {
            return Err(AdapterError::new("docker", format!("run failed: {}", String::from_utf8_lossy(&output.stderr))));
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    async fn exec(&self, id: &str, command: &str) -> Result<String, AdapterError> {
        let output = self.cli_output(vec![
            "exec".into(), id.to_string(), "sh".into(), "-c".into(), command.to_string()
        ]).await?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if !stderr.is_empty() {
                return Err(AdapterError::new("docker", format!("exec failed: {}", stderr)));
            }
            return Err(AdapterError::new("docker", "exec failed with no output"));
        }
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    async fn inspect_container(&self, id: &str) -> Result<String, AdapterError> {
        let output = self.cli_output(vec!["inspect".into(), id.to_string()]).await?;
        if !output.status.success() {
            return Err(AdapterError::new("docker", format!("inspect failed: {}", String::from_utf8_lossy(&output.stderr))));
        }
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    async fn list_images(&self) -> Result<Vec<Image>, AdapterError> {
        let output = self.cli_output(vec!["images".into(), "--format".into(), "json".into(), "--no-trunc".into()]).await?;
        if !output.status.success() {
            return Err(AdapterError::new("docker", String::from_utf8_lossy(&output.stderr).to_string()));
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        let images: Vec<Image> = stdout
            .lines()
            .filter(|l| !l.trim().is_empty())
            .filter_map(|l| {
                let v: serde_json::Value = serde_json::from_str(l).ok()?;
                Some(Image {
                    id: v.get("ID").or(v.get("Id")).and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    repository: v.get("Repository").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    tag: v.get("Tag").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    size: v.get("Size").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    created_at: v.get("CreatedAt").or(v.get("CreatedSince")).and_then(|v| v.as_str()).unwrap_or("").to_string(),
                })
            })
            .collect();
        Ok(images)
    }

    async fn pull_image(&self, name: &str) -> Result<String, AdapterError> {
        let output = self.cli_output(vec!["pull".into(), name.to_string()]).await?;
        if !output.status.success() {
            return Err(AdapterError::new("docker", format!("pull failed: {}", String::from_utf8_lossy(&output.stderr))));
        }
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    async fn remove_image(&self, id: &str, force: bool) -> Result<String, AdapterError> {
        let mut args = vec!["rmi".to_string()];
        if force { args.push("-f".into()); }
        args.push(id.to_string());
        let output = self.cli_output(args).await?;
        if !output.status.success() {
            return Err(AdapterError::new("docker", format!("rmi failed: {}", String::from_utf8_lossy(&output.stderr))));
        }
        Ok(format!("Image {} removed", id))
    }

    async fn prune_images(&self) -> Result<String, AdapterError> {
        let output = self.cli_output(vec!["image".into(), "prune".into(), "-a".into(), "-f".into()]).await?;
        if !output.status.success() {
            return Err(AdapterError::new("docker", format!("prune failed: {}", String::from_utf8_lossy(&output.stderr))));
        }
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    async fn list_volumes(&self) -> Result<Vec<Volume>, AdapterError> {
        let output = self.cli_output(vec!["volume".into(), "ls".into(), "--format".into(), "json".into()]).await?;
        if !output.status.success() {
            return Err(AdapterError::new("docker", String::from_utf8_lossy(&output.stderr).to_string()));
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        let volumes: Vec<Volume> = stdout
            .lines()
            .filter(|l| !l.trim().is_empty())
            .filter_map(|l| {
                let v: serde_json::Value = serde_json::from_str(l).ok()?;
                Some(Volume {
                    name: v.get("Name").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    driver: v.get("Driver").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    mountpoint: v.get("Mountpoint").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    scope: v.get("Scope").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                })
            })
            .collect();
        Ok(volumes)
    }

    async fn create_volume(&self, name: &str) -> Result<String, AdapterError> {
        let output = self.cli_output(vec!["volume".into(), "create".into(), name.to_string()]).await?;
        if !output.status.success() {
            return Err(AdapterError::new("docker", format!("volume create failed: {}", String::from_utf8_lossy(&output.stderr))));
        }
        Ok(format!("Volume {} created", name))
    }

    async fn remove_volume(&self, name: &str) -> Result<String, AdapterError> {
        let output = self.cli_output(vec!["volume".into(), "rm".into(), name.to_string()]).await?;
        if !output.status.success() {
            return Err(AdapterError::new("docker", format!("volume rm failed: {}", String::from_utf8_lossy(&output.stderr))));
        }
        Ok(format!("Volume {} removed", name))
    }

    async fn list_networks(&self) -> Result<Vec<Network>, AdapterError> {
        let output = self.cli_output(vec!["network".into(), "ls".into(), "--format".into(), "json".into()]).await?;
        if !output.status.success() {
            return Err(AdapterError::new("docker", String::from_utf8_lossy(&output.stderr).to_string()));
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        let networks: Vec<Network> = stdout
            .lines()
            .filter(|l| !l.trim().is_empty())
            .filter_map(|l| {
                let v: serde_json::Value = serde_json::from_str(l).ok()?;
                Some(Network {
                    id: v.get("ID").or(v.get("Id")).and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    name: v.get("Name").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    driver: v.get("Driver").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    scope: v.get("Scope").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                })
            })
            .collect();
        Ok(networks)
    }

    async fn create_network(&self, name: &str, driver: &str) -> Result<String, AdapterError> {
        let mut args = vec!["network".to_string(), "create".into()];
        if !driver.is_empty() { args.push("--driver".into()); args.push(driver.to_string()); }
        args.push(name.to_string());
        let output = self.cli_output(args).await?;
        if !output.status.success() {
            return Err(AdapterError::new("docker", format!("network create failed: {}", String::from_utf8_lossy(&output.stderr))));
        }
        Ok(format!("Network {} created", name))
    }

    async fn remove_network(&self, name: &str) -> Result<String, AdapterError> {
        let output = self.cli_output(vec!["network".into(), "rm".into(), name.to_string()]).await?;
        if !output.status.success() {
            return Err(AdapterError::new("docker", format!("network rm failed: {}", String::from_utf8_lossy(&output.stderr))));
        }
        Ok(format!("Network {} removed", name))
    }

    async fn system_df(&self) -> Result<String, AdapterError> {
        let output = self.cli_output(vec!["system".into(), "df".into()]).await?;
        if !output.status.success() {
            return Err(AdapterError::new("docker", format!("system df failed: {}", String::from_utf8_lossy(&output.stderr))));
        }
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    async fn system_prune(&self, all: bool) -> Result<String, AdapterError> {
        let mut args = vec!["system".to_string(), "prune".into(), "-f".into()];
        if all { args.push("-a".into()); }
        let output = self.cli_output(args).await?;
        if !output.status.success() {
            return Err(AdapterError::new("docker", format!("system prune failed: {}", String::from_utf8_lossy(&output.stderr))));
        }
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    async fn passthrough(&self, args: &[String]) -> Result<(), AdapterError> {
        let mut cmd = crate::commands::runtime::get_runtime_cmd();
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
                    Err(AdapterError::new("docker", "Command not found. Is docker/podman installed?"))
                } else {
                    Err(AdapterError::new("docker", format!("Failed to execute passthrough: {}", e)))
                }
            }
        }
    }
}
