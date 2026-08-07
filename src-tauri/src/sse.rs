//! SSE (Server-Sent Events) broadcast infrastructure.
//!
//! Provides a pub/sub channel for pushing real-time updates to browser clients,
//! plus background watchers for Docker events and instance state changes.

use std::sync::OnceLock;
use tokio::sync::broadcast;

// ===== Broadcast Channel =====

#[derive(Clone, Debug)]
pub struct SseMessage {
    pub event: String,
    pub data: String,
}

static SSE_TX: OnceLock<broadcast::Sender<SseMessage>> = OnceLock::new();

pub fn get_sse_tx() -> broadcast::Sender<SseMessage> {
    SSE_TX
        .get_or_init(|| {
            let (tx, _) = broadcast::channel(64);
            tx
        })
        .clone()
}

/// Publish an event to all connected SSE browser clients
pub fn publish_sse_event(event_type: &str, data: &serde_json::Value) {
    let tx = get_sse_tx();
    let _ = tx.send(SseMessage {
        event: event_type.to_string(),
        data: data.to_string(),
    });
}

// ===== Background Watchers =====

/// Watch Docker events via bollard and broadcast state changes to SSE clients.
/// Connects once; if the stream errors, the watcher exits (same as the original).
pub async fn sse_docker_watcher() {
    use bollard::system::EventsOptions;
    use futures_util::stream::StreamExt;

    // Connect to Docker using detected socket
    let docker: Option<bollard::Docker> = match crate::path_util::detect_docker_host() {
        Some((host, _)) => {
            bollard::Docker::connect_with_local(
                host.trim_start_matches("unix://"),
                120,
                bollard::API_DEFAULT_VERSION,
            )
            .ok()
        }
        None => bollard::Docker::connect_with_defaults().ok(),
    };

    let docker = match docker {
        Some(d) => d,
        None => {
            eprintln!("[SSE] Could not connect to Docker — SSE Docker watcher disabled");
            return;
        }
    };

    // Initial push
    if let Some(data) = fetch_docker_state(&docker).await {
        publish_sse_event("docker-state-updated", &data);
    }

    // Watch events and push updates (trailing-edge debounce — same as Tauri watcher)
    let mut stream = docker.events(Some(EventsOptions::<String>::default()));
    let debounce_ms: u64 = 500;

    loop {
        let event = tokio::select! {
            ev = stream.next() => ev,
        };

        match event {
            Some(Ok(_)) => {
                // Drain burst of events, wait for 500ms silence before fetching
                loop {
                    match tokio::time::timeout(
                        std::time::Duration::from_millis(debounce_ms),
                        stream.next(),
                    )
                    .await
                    {
                        Ok(Some(Ok(_))) => continue,
                        Ok(Some(Err(_))) | Ok(None) => break,
                        Err(_) => break, // timeout — burst settled
                    }
                }
                if let Some(data) = fetch_docker_state(&docker).await {
                    publish_sse_event("docker-state-updated", &data);
                }
            }
            Some(Err(e)) => {
                eprintln!("[SSE] Docker event stream error: {}", e);
                break;
            }
            None => break,
        }
    }
}

/// Fetch current Docker containers + images and return as JSON.
async fn fetch_docker_state(docker: &bollard::Docker) -> Option<serde_json::Value> {
    use bollard::container::ListContainersOptions;
    use bollard::image::ListImagesOptions;

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

    let mut mapped_containers = Vec::new();
    for c in containers {
        let names = c.names.unwrap_or_default().join(", ").replace("/", "");
        let ports = match c.ports {
            Some(ports) => ports
                .iter()
                .map(|p| {
                    let typ_str = p
                        .typ
                        .as_ref()
                        .map(|t| format!("{:?}", t).to_lowercase().replace("\"", ""))
                        .unwrap_or_else(|| "tcp".to_string());
                    if let Some(ip) = &p.ip {
                        format!("{}:{}->{}/{}", ip, p.public_port.unwrap_or(0), p.private_port, typ_str)
                    } else {
                        format!("{}/{}", p.private_port, typ_str)
                    }
                })
                .collect::<Vec<String>>()
                .join(", "),
            None => "".to_string(),
        };

        mapped_containers.push(serde_json::json!({
            "id": c.id.unwrap_or_default(),
            "Names": names,
            "Image": c.image.unwrap_or_default(),
            "Status": c.status.unwrap_or_default(),
            "State": c.state.unwrap_or_default(),
            "Ports": ports,
            "CreatedAt": c.created.unwrap_or(0).to_string(),
            "Size": c.size_rw.unwrap_or(0).to_string(),
            "Command": c.command.unwrap_or_default(),
        }));
    }

    let mut mapped_images = Vec::new();
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

        mapped_images.push(serde_json::json!({
            "id": i.id.replace("sha256:", ""),
            "Repository": repo,
            "Tag": tag,
            "Size": i.size.to_string(),
            "CreatedAt": i.created.to_string(),
        }));
    }

    Some(serde_json::json!({
        "containers": mapped_containers,
        "images": mapped_images
    }))
}

/// Periodically publish instance state to SSE clients
pub async fn sse_instance_publisher() {
    let mut last_json = String::new();
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        let instances = crate::instance_reader::list_instances_fast();
        let data = serde_json::json!({ "instances": instances });
        let json = data.to_string();
        // Only publish if state actually changed (avoid noise)
        if json != last_json {
            publish_sse_event("instances-update", &data);
            last_json = json;
        }
    }
}
