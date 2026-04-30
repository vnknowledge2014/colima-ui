use serde::Serialize;
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::Mutex;
use tokio::time::interval;

use crate::commands::colima::ColimaInstance;

/// Polling state shared between tasks
pub struct PollerState {
    pub instances: Arc<Mutex<Vec<ColimaInstance>>>,
}

impl Default for PollerState {
    fn default() -> Self {
        Self {
            instances: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

/// Payload emitted to frontend
#[derive(Debug, Clone, Serialize)]
pub struct InstancesUpdate {
    pub instances: Vec<ColimaInstance>,
    pub timestamp: u64,
}

/// Start background polling for instance status
pub fn start_instance_poller(app: &AppHandle) {
    let handle = app.clone();

    // Get or create state
    let state = app.state::<PollerState>();
    let instances = state.instances.clone();

    tauri::async_runtime::spawn(async move {
        let mut tick = interval(Duration::from_secs(5));

        loop {
            tick.tick().await;

            // Run colima list --json
            let result = tokio::task::spawn_blocking(|| {
                Command::new("colima")
                    .args(["list", "--json"])
                    .output()
            })
            .await;

            if let Ok(Ok(output)) = result {
                if output.status.success() {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    let parsed: Vec<ColimaInstance> = stdout
                        .lines()
                        .filter(|l| !l.trim().is_empty())
                        .filter_map(|l| serde_json::from_str(l).ok())
                        .collect();

                    // Update shared state
                    {
                        let mut guard = instances.lock().await;
                        *guard = parsed.clone();
                    }

                    let ts = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();

                    // Emit event to frontend
                    let _ = handle.emit("instances-update", InstancesUpdate {
                        instances: parsed,
                        timestamp: ts,
                    });
                }
            }
        }
    });
}
