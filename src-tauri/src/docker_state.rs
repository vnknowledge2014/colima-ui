use bollard::query_parameters::{EventsOptions, ListContainersOptions, ListImagesOptions};
use bollard::Docker;
use futures_util::stream::StreamExt;
use std::sync::{Arc, OnceLock};
use tauri::{AppHandle, Emitter};
use tokio::sync::RwLock;

static GLOBAL_DOCKER_STATE: OnceLock<Arc<RwLock<DockerState>>> = OnceLock::new();
static GLOBAL_APP_HANDLE: OnceLock<AppHandle> = OnceLock::new();

/// Initialize global references to DockerState and AppHandle for lifecycle notifications.
pub fn init_global_docker_state(state: Arc<RwLock<DockerState>>, app: AppHandle) {
    let _ = GLOBAL_DOCKER_STATE.set(state);
    let _ = GLOBAL_APP_HANDLE.set(app);
}

/// Centralized lifecycle helper called when an instance is stopping or being deleted.
/// Suppresses watcher reconnection, clears Docker caches, invalidates socket probe cache,
/// and broadcasts connection-lost and empty state events over both SSE and Tauri IPC.
pub fn notify_instance_stopping(profile: &str) {
    crate::path_util::invalidate_probe_cache();

    if let Some(state) = GLOBAL_DOCKER_STATE.get() {
        match state.try_write() {
            Ok(mut lock) => {
                lock.docker = None;
                lock.containers_cache = vec![];
                lock.images_cache = vec![];
                lock.suppressed = true;
            }
            Err(_) => {
                let state_clone = state.clone();
                tauri::async_runtime::spawn(async move {
                    let mut lock = state_clone.write().await;
                    lock.docker = None;
                    lock.containers_cache = vec![];
                    lock.images_cache = vec![];
                    lock.suppressed = true;
                });
            }
        }
    }

    let payload = serde_json::json!({ "profile": profile });
    let empty_state = serde_json::json!({ "containers": [], "images": [] });
    crate::sse::publish_sse_event("docker-connection-lost", &payload);
    crate::sse::publish_sse_event("docker-state-updated", &empty_state);

    if let Some(app) = GLOBAL_APP_HANDLE.get() {
        let _ = app.emit("docker-connection-lost", payload);
        let _ = app.emit("docker-state-updated", empty_state);
    }
}

/// Centralized lifecycle helper called when an instance has started or is starting.
/// Invalidates socket probe cache and unsets the suppressed flag so the watcher can reconnect.
pub fn notify_instance_started(_profile: &str) {
    crate::path_util::invalidate_probe_cache();

    if let Some(state) = GLOBAL_DOCKER_STATE.get() {
        match state.try_write() {
            Ok(mut lock) => {
                lock.suppressed = false;
            }
            Err(_) => {
                let state_clone = state.clone();
                tauri::async_runtime::spawn(async move {
                    let mut lock = state_clone.write().await;
                    lock.suppressed = false;
                });
            }
        }
    }
}

/// Connect to Docker via the Colima socket.
/// This is essential because macOS .app bundles don't inherit DOCKER_HOST,
/// and Colima doesn't create /var/run/docker.sock (which Bollard defaults to).
/// Connect to whichever daemon this machine has.
///
/// `pub(crate)` because the SSE watcher needs the *same* answer: it once had its
/// own copy, and when that copy was removed in favour of calling this, the
/// difference between them turned out to be the `connect_with_defaults()`
/// fallback — i.e. every machine running Docker Desktop rather than colima.
pub(crate) fn connect_bollard() -> Option<Docker> {
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
    /// True once the watcher has emitted a containerd-unsupported warning.
    /// Reset when no containerd profile is running, so a later switch to
    /// containerd is still reported.
    pub containerd_warned: bool,
}

impl DockerState {
    /// Creates a new DockerState. Never panics — if Docker is unavailable,
    /// `docker` is `None` and caches start empty.
    pub fn new() -> Self {
        let docker = connect_bollard();
        if docker.is_none() {
            let containerd = crate::instance_reader::running_containerd_profiles();
            if containerd.is_empty() {
                eprintln!("[DockerState] Docker daemon not reachable — starting with empty state");
            } else {
                eprintln!(
                    "[DockerState] Docker daemon not reachable — running Colima profile(s) use the unsupported containerd runtime: {}. Use `colima start --runtime docker`.",
                    containerd.join(", ")
                );
            }
        }
        Self {
            docker,
            containers_cache: vec![],
            images_cache: vec![],
            suppressed: false,
            containerd_warned: false,
        }
    }

    /// Returns a reference to the Docker client, or an error if not connected.
    #[allow(dead_code)]
    pub fn docker(&self) -> Result<&Docker, String> {
        self.docker.as_ref().ok_or_else(|| "Docker daemon is not connected".to_string())
    }
}

impl Default for DockerState {
    fn default() -> Self {
        Self::new()
    }
}

/// Resilient Docker event watcher with auto-reconnect.
/// Runs forever: connects → streams events → on disconnect, clears state → retries.
/// Push-based (no polling) — same approach as OrbStack.
pub async fn start_docker_watcher(app: AppHandle, state: Arc<RwLock<DockerState>>) {
    init_global_docker_state(state.clone(), app.clone());
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
                let containerd = crate::instance_reader::running_containerd_profiles();
                if !containerd.is_empty() {
                    let mut lock = state.write().await;
                    if !lock.containerd_warned {
                        lock.containerd_warned = true;
                        let payload = serde_json::json!({
                            "runtime": "containerd",
                            "profiles": containerd,
                        });
                        crate::sse::publish_sse_event("docker-unsupported-runtime", &payload);
                        let _ = app.emit("docker-unsupported-runtime", payload.clone());
                        eprintln!(
                            "[DockerWatcher] containerd runtime is unsupported for profile(s): {}. Use `colima start --runtime docker`.",
                            containerd.join(", ")
                        );
                    }
                } else {
                    state.write().await.containerd_warned = false;
                }
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

        // Initial fetch on (re)connect — push to frontend immediately.
        //
        // Both transports, not just Tauri. Browser-mode clients are driven by
        // container events, and a recovery produces none: after a VM restart
        // with nothing running there is simply nothing to report, so a client
        // that only reacts to events would sit on pre-outage data forever.
        if let Ok(data) = update_cache(&docker, &state).await {
            crate::sse::publish_sse_event("docker-state-updated", &data);
            let _ = app.emit("docker-state-updated", data);
        }
        // Notify frontend of reconnection so it can refetch volumes/networks/compose
        crate::sse::publish_sse_event("docker-reconnected", &serde_json::json!({}));
        let _ = app.emit("docker-reconnected", serde_json::json!({}));

        let (detected_host, detected_profile) = match crate::path_util::detect_docker_host() {
            Some((h, p)) => (h, p),
            None => (String::new(), "default".to_string()),
        };

        // Stream Docker events until connection drops
        // Trailing-edge debounce: when events arrive in rapid bursts (e.g., docker stop
        // fires kill→stop→die within ~200ms), wait for the burst to settle before
        // refreshing. This ensures we always capture the FINAL state, not an
        // intermediate transition state.
        let mut stream = docker.events(Some(EventsOptions::default()));
        let debounce_ms: u64 = 500; // wait 500ms after last event before fetching
        let mut heartbeat = tokio::time::interval(std::time::Duration::from_secs(5));
        heartbeat.tick().await; // Consume initial immediate tick

        let mut connection_lost_reason: Option<String> = None;

        loop {
            // Wait for the next event or periodic 5s ping heartbeat
            let event = tokio::select! {
                _ = heartbeat.tick() => {
                    let ping_res = tokio::time::timeout(std::time::Duration::from_secs(2), docker.ping()).await;
                    let is_ok = matches!(&ping_res, Ok(Ok(_)));
                    if !is_ok {
                        let reason = match ping_res {
                            Err(_) => "docker.ping() timed out after 2s".to_string(),
                            Ok(Err(e)) => format!("docker.ping() failed: {}", e),
                            _ => "docker ping failed".to_string(),
                        };
                        eprintln!("[DockerWatcher] Heartbeat ping failed ({}) — dropping connection", reason);
                        connection_lost_reason = Some(reason);
                        break;
                    }
                    continue;
                }
                ev = stream.next() => ev,
            };

            match event {
                Some(Ok(ev)) => {
                    // Fork before debouncing. This watcher only needs "something
                    // changed", but the events themselves carry information the
                    // debounce is about to destroy — five restarts in ten seconds
                    // collapse into one refresh here, and a crash-loop rule reading
                    // only this path would never see the loop. See `docker_events`.
                    if let Some(e) = crate::docker_events::from_bollard(&ev) {
                        crate::docker_events::publish(e);
                    }

                    // Event received — now drain any further events within the debounce window
                    // (trailing-edge: keep resetting the timer while events keep coming)
                    loop {
                        match tokio::time::timeout(
                            std::time::Duration::from_millis(debounce_ms),
                            stream.next(),
                        )
                        .await
                        {
                            Ok(Some(Ok(ev))) => {
                                // Drained for this watcher's purposes, but still
                                // published: these are exactly the events a burst
                                // is made of, and they are the ones that matter to
                                // anything counting.
                                if let Some(e) = crate::docker_events::from_bollard(&ev) {
                                    crate::docker_events::publish(e);
                                }
                                // Another event within the window — keep draining
                                continue;
                            }
                            Ok(Some(Err(e))) => {
                                // Stream error during drain — break to outer error handling
                                connection_lost_reason = Some(format!("event stream error during drain: {}", e));
                                break;
                            }
                            Ok(None) => {
                                // Stream ended during drain
                                connection_lost_reason = Some("event stream ended during drain".to_string());
                                break;
                            }
                            Err(_) => {
                                // Timeout — debounce window elapsed with no new events.
                                // NOW fetch the final state.
                                break;
                            }
                        }
                    }
                    if connection_lost_reason.is_some() {
                        break;
                    }
                    // Fetch and emit the settled state to BOTH Tauri IPC and SSE
                    if let Ok(data) = update_cache(&docker, &state).await {
                        crate::sse::publish_sse_event("docker-state-updated", &data);
                        let _ = app.emit("docker-state-updated", data);
                    }
                }
                Some(Err(e)) => {
                    // Event stream error — break to reconnect logic
                    connection_lost_reason = Some(format!("event stream error: {}", e));
                    break;
                }
                None => {
                    // Stream ended
                    connection_lost_reason = Some("event stream ended".to_string());
                    break;
                }
            }
        }

        // Stream ended or ping failed — check if Docker is actually gone or just a transient stream error
        let is_transient = if connection_lost_reason.as_deref().is_some_and(|r| r.starts_with("docker.ping()")) {
            false
        } else {
            matches!(
                tokio::time::timeout(std::time::Duration::from_secs(2), docker.ping()).await,
                Ok(Ok(_))
            )
        };

        if is_transient {
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
        crate::path_util::invalidate_probe_cache();

        let (host, profile) = match crate::path_util::detect_docker_host() {
            Some((h, p)) => (h, p),
            None => (detected_host.clone(), detected_profile.clone()),
        };
        let reason = connection_lost_reason.unwrap_or_else(|| "Docker daemon connection lost".to_string());
        let lost_payload = serde_json::json!({
            "host": host,
            "profile": profile,
            "reason": reason,
        });
        let empty = serde_json::json!({ "containers": [], "images": [] });

        crate::sse::publish_sse_event("docker-connection-lost", &lost_payload);
        crate::sse::publish_sse_event("docker-state-updated", &empty);
        let _ = app.emit("docker-connection-lost", lost_payload);
        let _ = app.emit("docker-state-updated", empty);

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
                        .map_or_else(
                            || "tcp".to_string(),
                            |t| format!("{:?}", t).to_lowercase().replace("\"", ""),
                        );
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
            "State": c.state.unwrap_or(bollard::models::ContainerSummaryStateEnum::EMPTY),
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

pub(crate) async fn update_cache(
    docker: &Docker,
    state: &Arc<RwLock<DockerState>>,
) -> Result<serde_json::Value, String> {
    let containers = match tokio::time::timeout(
        std::time::Duration::from_secs(5),
        docker.list_containers(Some(ListContainersOptions {
            all: true,
            ..Default::default()
        })),
    )
    .await
    {
        Ok(Ok(c)) => c,
        Ok(Err(e)) => {
            eprintln!("[DockerWatcher] list_containers error: {}", e);
            return Err(format!("update_cache: list_containers error: {}", e));
        }
        Err(_) => {
            return Err("update_cache: list_containers timed out".to_string());
        }
    };

    let images = match tokio::time::timeout(
        std::time::Duration::from_secs(5),
        docker.list_images(Some(ListImagesOptions {
            all: false,
            ..Default::default()
        })),
    )
    .await
    {
        Ok(Ok(i)) => i,
        Ok(Err(e)) => {
            eprintln!("[DockerWatcher] list_images error: {}", e);
            return Err(format!("update_cache: list_images error: {}", e));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn docker_state_initializes_empty_when_no_daemon() {
        let state = DockerState {
            docker: None,
            containers_cache: vec![],
            images_cache: vec![],
            suppressed: false,
            containerd_warned: false,
        };
        assert!(state.docker().is_err());
        assert!(state.containers_cache.is_empty());
        assert!(state.images_cache.is_empty());
    }

    #[test]
    fn map_containers_formats_ports_and_labels_consistently() {
        let summary: bollard::models::ContainerSummary = serde_json::from_value(serde_json::json!({
            "Id": "1234567890abcdef",
            "Names": ["/my-app"],
            "Image": "nginx:latest",
            "Status": "Up 2 hours",
            "State": "running",
            "Ports": [{
                "IP": "0.0.0.0",
                "PrivatePort": 80,
                "PublicPort": 8080,
                "Type": "tcp"
            }],
            "Created": 1700000000,
            "SizeRw": 1024,
            "Command": "nginx -g 'daemon off;'",
            "Labels": {
                "com.docker.compose.project": "demo"
            }
        })).expect("deserialize ContainerSummary");

        let mapped = map_containers(&[summary]);
        assert_eq!(mapped.len(), 1);
        assert_eq!(mapped[0]["Id"], "1234567890abcdef");
        assert_eq!(mapped[0]["Names"], "my-app");
        assert_eq!(mapped[0]["Ports"], "0.0.0.0:8080->80/tcp");
        assert_eq!(mapped[0]["Labels"]["com.docker.compose.project"], "demo");
    }

    #[test]
    fn map_images_strips_sha_prefix_and_handles_tags() {
        let img = bollard::models::ImageSummary {
            id: "sha256:abcdef1234567890".to_string(),
            repo_tags: vec!["alpine:3.18".to_string()],
            size: 5000000,
            created: 1700000000,
            ..Default::default()
        };

        let mapped = map_images(&[img]);
        assert_eq!(mapped.len(), 1);
        assert_eq!(mapped[0]["Id"], "abcdef1234567890");
        assert_eq!(mapped[0]["Repository"], "alpine");
        assert_eq!(mapped[0]["Tag"], "3.18");
    }

    #[tokio::test]
    async fn update_cache_fails_with_err_when_disconnected() {
        if let Ok(docker) = Docker::connect_with_unix("/tmp/nonexistent_mock_socket.sock", 1, bollard::API_DEFAULT_VERSION) {
            let state = Arc::new(RwLock::new(DockerState {
                docker: Some(docker.clone()),
                containers_cache: vec![serde_json::json!({"Id": "prev"})],
                images_cache: vec![],
                suppressed: false,
                containerd_warned: false,
            }));
            let res = update_cache(&docker, &state).await;
            assert!(res.is_err(), "update_cache must return Err on failure rather than Ok with empty vectors");
            let lock = state.read().await;
            assert_eq!(lock.containers_cache.len(), 1, "Cache must not be clobbered on failure");
        }
    }
}
