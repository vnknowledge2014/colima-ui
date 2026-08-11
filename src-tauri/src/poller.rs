use serde::Serialize;
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

            // Use the fast filesystem reader (shared with Tauri command and API server)
            let result =
                tokio::task::spawn_blocking(|| crate::instance_reader::list_instances_fast()).await;

            if let Ok(parsed) = result {
                // Update shared state
                {
                    let mut guard = instances.lock().await;
                    // An instance changing state flips Colima and Docker
                    // between "installed but not running" and "usable", so the
                    // capability cache must be dropped now rather than waiting
                    // out its TTL — otherwise the UI keeps telling the user to
                    // start something they just started.
                    let changed = guard.len() != parsed.len()
                        || guard
                            .iter()
                            .zip(parsed.iter())
                            .any(|(old, new)| old.name != new.name || old.status != new.status);
                    if changed {
                        crate::commands::system_capabilities::invalidate();
                    }
                    *guard = parsed.clone();
                }

                // The tray reads from this poller, not from sse.rs — that one
                // is a separate publisher for browser mode and never fires in
                // the desktop app.
                crate::tray::refresh(&handle, &parsed);

                let ts = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();

                // Emit event to frontend
                let _ = handle.emit(
                    "instances-update",
                    InstancesUpdate {
                        instances: parsed,
                        timestamp: ts,
                    },
                );
            }
        }
    });
}
