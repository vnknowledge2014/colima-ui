use bollard::container::ListContainersOptions;
use bollard::image::ListImagesOptions;
use bollard::system::EventsOptions;
use bollard::Docker;
use futures_util::stream::StreamExt;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::sync::RwLock;

pub struct DockerState {
    pub docker: Option<Docker>,
    pub containers_cache: Vec<serde_json::Value>,
    pub images_cache: Vec<serde_json::Value>,
}

impl DockerState {
    /// Creates a new DockerState. Never panics — if Docker is unavailable,
    /// `docker` is `None` and caches start empty.
    pub fn new() -> Self {
        let docker = Docker::connect_with_defaults().ok();
        if docker.is_none() {
            eprintln!("[DockerState] Docker daemon not reachable — starting with empty state");
        }
        Self {
            docker,
            containers_cache: vec![],
            images_cache: vec![],
        }
    }

    /// Returns a reference to the Docker client, or an error if not connected.
    pub fn docker(&self) -> Result<&Docker, String> {
        self.docker.as_ref().ok_or_else(|| "Docker daemon is not connected".to_string())
    }
}

/// Resilient Docker event watcher with auto-reconnect.
/// Runs forever: connects → streams events → on disconnect, clears state → retries.
/// Push-based (no polling) — same approach as OrbStack.
pub async fn start_docker_watcher(app: AppHandle, state: Arc<RwLock<DockerState>>) {
    loop {
        // Try to connect (or reconnect) to Docker daemon
        let docker = match Docker::connect_with_defaults() {
            Ok(d) => {
                // Verify connection is actually alive with a ping
                if d.ping().await.is_err() {
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                    continue;
                }
                eprintln!("[DockerWatcher] Connected to Docker daemon");
                d
            }
            Err(_) => {
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                continue;
            }
        };

        // Update DockerState with fresh connection (so list_containers/list_images use it)
        {
            let mut lock = state.write().await;
            lock.docker = Some(docker.clone());
        }

        // Initial fetch on (re)connect — push to frontend immediately
        if let Ok(data) = update_cache(&docker, &state).await {
            let _ = app.emit("docker-state-updated", data);
        }

        // Stream Docker events until connection drops
        let mut stream = docker.events(Some(EventsOptions::<String>::default()));
        while let Some(event) = stream.next().await {
            match event {
                Ok(_) => {
                    if let Ok(data) = update_cache(&docker, &state).await {
                        let _ = app.emit("docker-state-updated", data);
                    }
                }
                Err(_) => {
                    // Connection error — break to reconnect
                    break;
                }
            }
        }

        // Stream ended (Docker stopped or connection lost)
        // Clear stale data and notify frontend
        {
            let mut lock = state.write().await;
            lock.docker = None;
            lock.containers_cache = vec![];
            lock.images_cache = vec![];
        }
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
    let containers = docker
        .list_containers(Some(ListContainersOptions::<String> {
            all: true,
            ..Default::default()
        }))
        .await
        .unwrap_or_default();

    let images = docker
        .list_images(Some(ListImagesOptions::<String> {
            all: false,
            ..Default::default()
        }))
        .await
        .unwrap_or_default();

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
