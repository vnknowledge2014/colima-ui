use bollard::container::ListContainersOptions;
use bollard::image::ListImagesOptions;
use bollard::system::EventsOptions;
use bollard::Docker;
use futures_util::stream::StreamExt;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::sync::RwLock;

/// Connect to Docker via the Colima socket.
/// This is essential because macOS .app bundles don't inherit DOCKER_HOST,
/// and Colima doesn't create /var/run/docker.sock (which Bollard defaults to).
fn connect_bollard() -> Option<Docker> {
    // Try Colima socket detection first
    if let Some((host, _)) = crate::path_util::detect_docker_host() {
        // host is like "unix:///Users/mike/.colima/default/docker.sock"
        let socket_path = host.trim_start_matches("unix://");
        if let Ok(d) = Docker::connect_with_unix(socket_path, 5, bollard::API_DEFAULT_VERSION) {
            return Some(d);
        }
    }
    // Fallback to defaults (works if /var/run/docker.sock exists or DOCKER_HOST is set)
    Docker::connect_with_defaults().ok()
}

pub struct DockerState {
    pub docker: Option<Docker>,
    pub containers_cache: Vec<serde_json::Value>,
    pub images_cache: Vec<serde_json::Value>,
    /// When true, the watcher must NOT reconnect or push data.
    /// Set by stop_instance/delete_instance before running the colima command.
    /// The watcher auto-clears this when the socket is truly gone (detect_docker_host() returns None).
    pub suppressed: bool,
}

impl DockerState {
    /// Creates a new DockerState. Never panics — if Docker is unavailable,
    /// `docker` is `None` and caches start empty.
    pub fn new() -> Self {
        let docker = connect_bollard();
        if docker.is_none() {
            eprintln!("[DockerState] Docker daemon not reachable — starting with empty state");
        }
        Self {
            docker,
            containers_cache: vec![],
            images_cache: vec![],
            suppressed: false,
        }
    }

    /// Returns a reference to the Docker client, or an error if not connected.
    #[allow(dead_code)]
    pub fn docker(&self) -> Result<&Docker, String> {
        self.docker.as_ref().ok_or_else(|| "Docker daemon is not connected".to_string())
    }
}

/// Resilient Docker event watcher with auto-reconnect.
/// Runs forever: connects → streams events → on disconnect, clears state → retries.
/// Push-based (no polling) — same approach as OrbStack.
pub async fn start_docker_watcher(app: AppHandle, state: Arc<RwLock<DockerState>>) {
    loop {
        // Check suppression flag — stop/delete in progress, don't reconnect
        {
            let is_suppressed = state.read().await.suppressed;
            if is_suppressed {
                // Socket might still exist during shutdown — check if it's truly gone
                if crate::path_util::detect_docker_host().is_none() {
                    // Socket gone → clear suppression so watcher can reconnect when instance starts again
                    state.write().await.suppressed = false;
                }
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                continue;
            }
        }

        // Try to connect (or reconnect) to Docker daemon via Colima socket
        let docker = match connect_bollard() {
            Some(d) => {
                // Verify connection is actually alive with a ping (with timeout)
                match tokio::time::timeout(std::time::Duration::from_secs(5), d.ping()).await {
                    Ok(Ok(_)) => {
                        eprintln!("[DockerWatcher] Connected to Docker daemon");
                        d
                    }
                    _ => {
                        eprintln!("[DockerWatcher] Docker ping failed or timed out");
                        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                        continue;
                    }
                }
            }
            None => {
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                continue;
            }
        };

        // Update DockerState with fresh connection (so list_containers/list_images use it)
        {
            let mut lock = state.write().await;
            // Double-check suppression in case stop was called between connect and here
            if lock.suppressed {
                continue;
            }
            lock.docker = Some(docker.clone());
        }

        // Initial fetch on (re)connect — push to frontend immediately
        if let Ok(data) = update_cache(&docker, &state).await {
            let _ = app.emit("docker-state-updated", data);
        }
        // Notify frontend of reconnection so it can refetch volumes/networks/compose
        let _ = app.emit("docker-reconnected", serde_json::json!({}));

        // Stream Docker events until connection drops
        // Trailing-edge debounce: when events arrive in rapid bursts (e.g., docker stop
        // fires kill→stop→die within ~200ms), wait for the burst to settle before
        // refreshing. This ensures we always capture the FINAL state, not an
        // intermediate transition state.
        let mut stream = docker.events(Some(EventsOptions::<String>::default()));
        let debounce_ms: u64 = 500; // wait 500ms after last event before fetching

        loop {
            // Wait for the next event
            let event = tokio::select! {
                ev = stream.next() => ev,
            };

            match event {
                Some(Ok(_)) => {
                    // Event received — now drain any further events within the debounce window
                    // (trailing-edge: keep resetting the timer while events keep coming)
                    loop {
                        match tokio::time::timeout(
                            std::time::Duration::from_millis(debounce_ms),
                            stream.next(),
                        )
                        .await
                        {
                            Ok(Some(Ok(_))) => {
                                // Another event within the window — keep draining
                                continue;
                            }
                            Ok(Some(Err(_))) => {
                                // Stream error during drain — break to outer error handling
                                break;
                            }
                            Ok(None) => {
                                // Stream ended during drain
                                break;
                            }
                            Err(_) => {
                                // Timeout — debounce window elapsed with no new events.
                                // NOW fetch the final state.
                                break;
                            }
                        }
                    }
                    // Fetch and emit the settled state
                    if let Ok(data) = update_cache(&docker, &state).await {
                        let _ = app.emit("docker-state-updated", data);
                    }
                }
                Some(Err(_)) => {
                    // Event stream error — break to reconnect logic
                    break;
                }
                None => {
                    // Stream ended
                    break;
                }
            }
        }

        // Stream ended — check if Docker is actually gone or just a transient stream error
        let ping_ok = matches!(
            tokio::time::timeout(std::time::Duration::from_secs(5), docker.ping()).await,
            Ok(Ok(_))
        );
        if ping_ok {
            // Docker is still alive — transient stream error.
            // Don't clear state, just reconnect the event stream.
            eprintln!("[DockerWatcher] Event stream interrupted but Docker is alive — reconnecting stream");
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            continue;
        }

        // Docker is truly unreachable — clear stale data and notify frontend
        {
            let mut lock = state.write().await;
            lock.docker = None;
            lock.containers_cache = vec![];
            lock.images_cache = vec![];
        }
        // Emit specific connection-lost event so frontend can clear ALL Docker state
        let _ = app.emit("docker-connection-lost", serde_json::json!({}));
        let _ = app.emit("docker-state-updated", serde_json::json!({
            "containers": [],
            "images": []
        }));

        eprintln!("[DockerWatcher] Connection lost — will retry in 2s");
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    }
}

/// Map bollard ContainerSummary to our JSON format (public for reuse by command handlers)
pub fn map_containers(containers: &[bollard::models::ContainerSummary]) -> Vec<serde_json::Value> {
    let mut mapped = Vec::new();
    for c in containers {
        let names = c.names.clone().unwrap_or_default().join(", ").replace("/", "");
        let ports = match &c.ports {
            Some(ports) => ports
                .iter()
                .map(|p| {
                    let typ_str = p
                        .typ
                        .as_ref()
                        .map(|t| format!("{:?}", t).to_lowercase().replace("\"", ""))
                        .unwrap_or_else(|| "tcp".to_string());
                    if let Some(ip) = &p.ip {
                        format!(
                            "{}:{}->{}/{}",
                            ip,
                            p.public_port.unwrap_or(0),
                            p.private_port,
                            typ_str
                        )
                    } else {
                        format!("{}/{}", p.private_port, typ_str)
                    }
                })
                .collect::<Vec<String>>()
                .join(", "),
            None => "".to_string(),
        };

        mapped.push(serde_json::json!({
            "Id": c.id.clone().unwrap_or_default(),
            "Names": names,
            "Image": c.image.clone().unwrap_or_default(),
            "Status": c.status.clone().unwrap_or_default(),
            "State": c.state.clone().unwrap_or_default(),
            "Ports": ports,
            "CreatedAt": c.created.unwrap_or(0).to_string(),
            "Size": c.size_rw.unwrap_or(0).to_string(),
            "Command": c.command.clone().unwrap_or_default(),
            // Compose grouping reads com.docker.compose.project from here.
            // Always an object, never null, so the frontend does not have to
            // special-case a container with no labels.
            "Labels": c.labels.clone().unwrap_or_default(),
        }));
    }
    mapped
}

/// Map bollard ImageSummary to our JSON format (public for reuse by command handlers)
pub fn map_images(images: &[bollard::models::ImageSummary]) -> Vec<serde_json::Value> {
    let mut mapped = Vec::new();
    for i in images {
        let tags = i.repo_tags.clone();
        let (repo, tag) = if !tags.is_empty() && tags[0] != "<none>:<none>" {
            let parts: Vec<&str> = tags[0].split(':').collect();
            if parts.len() == 2 {
                (parts[0].to_string(), parts[1].to_string())
            } else {
                (tags[0].clone(), "latest".to_string())
            }
        } else {
            ("<none>".to_string(), "<none>".to_string())
        };

        mapped.push(serde_json::json!({
            "Id": i.id.replace("sha256:", ""),
            "Repository": repo,
            "Tag": tag,
            "Size": i.size.to_string(),
            "CreatedAt": i.created.to_string(),
        }));
    }
    mapped
}

async fn update_cache(
    docker: &Docker,
    state: &Arc<RwLock<DockerState>>,
) -> Result<serde_json::Value, String> {
    let containers = match tokio::time::timeout(
        std::time::Duration::from_secs(5),
        docker.list_containers(Some(ListContainersOptions::<String> {
            all: true,
            ..Default::default()
        })),
    )
    .await
    {
        Ok(Ok(c)) => c,
        Ok(Err(e)) => {
            eprintln!("[DockerWatcher] list_containers error: {}", e);
            vec![]
        }
        Err(_) => {
            return Err("update_cache: list_containers timed out".to_string());
        }
    };

    let images = match tokio::time::timeout(
        std::time::Duration::from_secs(5),
        docker.list_images(Some(ListImagesOptions::<String> {
            all: false,
            ..Default::default()
        })),
    )
    .await
    {
        Ok(Ok(i)) => i,
        Ok(Err(e)) => {
            eprintln!("[DockerWatcher] list_images error: {}", e);
            vec![]
        }
        Err(_) => {
            return Err("update_cache: list_images timed out".to_string());
        }
    };

    let mapped_containers = map_containers(&containers);
    let mapped_images = map_images(&images);

    let mut lock = state.write().await;
    lock.containers_cache = mapped_containers.clone();
    lock.images_cache = mapped_images.clone();

    Ok(serde_json::json!({
        "containers": mapped_containers,
        "images": mapped_images
    }))
}
