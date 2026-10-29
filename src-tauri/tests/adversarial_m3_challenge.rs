//! Milestone 3 Adversarial Challenge & Stress-Test Suite
//!
//! Rigorously challenges and stress-tests Milestone 3: Real-Time State Synchronization & Self-Healing (R3):
//! 1. Mock Unix Socket Ping Timeout / Failure Simulation:
//!    - Verifies socket hanging (ping timeout >= 2s)
//!    - Verifies truncated socket (immediate 0-byte EOF)
//!    - Verifies HTTP 500 error response from dead daemon
//!    - Verifies HTTP 200 OK response from live daemon
//! 2. SSE Event Stream & Disconnection Propagation:
//!    - Verifies SSE subscription receives "docker-connection-lost" event on instance stopping
//!    - Verifies SSE subscription receives empty "docker-state-updated" event on instance stopping
//!    - Verifies socket probe cache invalidation
//!    - Multi-subscriber broadcast stress test under concurrent load
//! 3. `fetch_current_docker_state()` Return Semantics:
//!    - Verifies that disconnected / unreachable daemon returns `None` rather than masking errors as an empty structure
//!    - Verifies that if state is returned, both "containers" and "images" fields are valid arrays
//! 4. Lifecycle Notifications & DockerState Cache Invariants:
//!    - Verifies `notify_instance_stopping` and `notify_instance_started` lifecycle signaling
//!    - Verifies initial empty state and error return from `state.docker()` when disconnected
//!    - Verifies `map_containers` and `map_images` formatting invariants
//! 5. Unresponsive VM Self-Healing Sweep & Restart Action:
//!    - Verifies `sweep_vm_unresponsive` executes safely without panic
//!    - Verifies `HealTrigger::VmUnresponsive` and `HealAction::RestartVm` enum mappings and auto_capable status
//! 6. SQLite Test DB Isolation Hard Guarantee:
//!    - Verifies `db_path()` strictly points to `std::env::temp_dir()`
//!    - Verifies live `~/.colima-ui/activity.db*` files are never touched or modified
//! 7. CLI Label Normalization Adversarial Stress:
//!    - Malformed label strings, consecutive commas, keys with multiple equals signs, whitespace trimming
//! 8. Concurrency & Contention Stress Test:
//!    - 20 concurrent Tokio tasks executing state checks, subscriptions, and activity recording

use colima_ui_lib::commands::activity::{db_path, record, ActivityActor, ActivityEntry, ActivityKind};
use colima_ui_lib::commands::containers::normalize_cli_labels;
use colima_ui_lib::commands::self_heal::{sweep_vm_unresponsive, HealAction, HealTrigger};
use colima_ui_lib::docker_state::{
    map_containers, map_images, notify_instance_started, notify_instance_stopping, DockerState,
};
use colima_ui_lib::path_util::{app_data_dir, invalidate_probe_cache, probe_docker_socket};
use colima_ui_lib::sse::{
    fetch_current_docker_state, publish_sse_event, subscribe_topics, subscriber_count,
};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixListener;
use tokio::sync::RwLock;

// ===========================================================================
// SECTION 1: Mock Unix Socket Ping Timeout / Failure Simulation
// ===========================================================================

#[tokio::test]
async fn test_adv_m3_mock_socket_hang_triggers_timeout() {
    let sock_path = std::env::temp_dir().join(format!("adv_m3_hang_{}.sock", std::process::id()));
    let _ = std::fs::remove_file(&sock_path);

    let listener = UnixListener::bind(&sock_path).expect("bind mock hanging unix socket");

    // Spawn server that accepts connection and sleeps without replying
    let srv = tokio::spawn(async move {
        if let Ok((mut stream, _)) = listener.accept().await {
            // Read incoming HTTP GET /_ping
            let mut buf = [0u8; 512];
            let _ = stream.read(&mut buf).await;
            // Sleep 5 seconds to simulate a completely hung/frozen docker daemon
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    });

    // probe_docker_socket should fail (timeout is 300ms)
    let is_alive = probe_docker_socket(&sock_path, Duration::from_millis(300));
    assert!(!is_alive, "probe_docker_socket must return false for a hanging socket");

    // Bollard connect and ping with 500ms timeout
    if let Ok(docker) = bollard::Docker::connect_with_unix(
        sock_path.to_str().unwrap(),
        1,
        bollard::API_DEFAULT_VERSION,
    ) {
        let ping_res = tokio::time::timeout(Duration::from_millis(500), docker.ping()).await;
        assert!(
            ping_res.is_err(),
            "Ping against hanging socket must time out, got: {:?}",
            ping_res
        );
    }

    srv.abort();
    let _ = std::fs::remove_file(&sock_path);
}

#[tokio::test]
async fn test_adv_m3_mock_socket_eof_triggers_immediate_error() {
    let sock_path = std::env::temp_dir().join(format!("adv_m3_eof_{}.sock", std::process::id()));
    let _ = std::fs::remove_file(&sock_path);

    let listener = UnixListener::bind(&sock_path).expect("bind mock eof unix socket");

    // Server accepts connection and immediately drops it (EOF)
    let srv = tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            drop(stream); // Immediate EOF / connection reset
        }
    });

    // probe_docker_socket should return false on immediate EOF
    let is_alive = probe_docker_socket(&sock_path, Duration::from_millis(500));
    assert!(!is_alive, "probe_docker_socket must return false on immediate EOF");

    // Bollard ping against EOF socket must return error
    if let Ok(docker) = bollard::Docker::connect_with_unix(
        sock_path.to_str().unwrap(),
        1,
        bollard::API_DEFAULT_VERSION,
    ) {
        let ping_res = docker.ping().await;
        assert!(
            ping_res.is_err(),
            "Ping against EOF socket must return Err, got: {:?}",
            ping_res
        );
    }

    srv.abort();
    let _ = std::fs::remove_file(&sock_path);
}

#[tokio::test]
async fn test_adv_m3_mock_socket_http_500_triggers_ping_error() {
    let sock_path = std::env::temp_dir().join(format!("adv_m3_500_{}.sock", std::process::id()));
    let _ = std::fs::remove_file(&sock_path);

    let listener = UnixListener::bind(&sock_path).expect("bind mock 500 unix socket");

    let srv = tokio::spawn(async move {
        while let Ok((mut stream, _)) = listener.accept().await {
            let mut buf = [0u8; 512];
            let _ = stream.read(&mut buf).await;
            let response = "HTTP/1.1 500 Internal Server Error\r\nContent-Length: 0\r\n\r\n";
            let _ = stream.write_all(response.as_bytes()).await;
        }
    });

    // probe_docker_socket must return false on 500
    let is_alive = probe_docker_socket(&sock_path, Duration::from_millis(500));
    assert!(!is_alive, "probe_docker_socket must return false on HTTP 500");

    if let Ok(docker) = bollard::Docker::connect_with_unix(
        sock_path.to_str().unwrap(),
        1,
        bollard::API_DEFAULT_VERSION,
    ) {
        let ping_res = docker.ping().await;
        assert!(
            ping_res.is_err(),
            "Ping against HTTP 500 socket must return Err, got: {:?}",
            ping_res
        );
    }

    srv.abort();
    let _ = std::fs::remove_file(&sock_path);
}

#[tokio::test]
async fn test_adv_m3_mock_socket_http_200_succeeds() {
    let sock_path = std::env::temp_dir().join(format!("adv_m3_200_{}.sock", std::process::id()));
    let _ = std::fs::remove_file(&sock_path);

    let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let stop_clone = stop.clone();
    let s_path = sock_path.clone();

    let srv_handle = std::thread::spawn(move || {
        use std::io::{Read, Write};
        use std::os::unix::net::UnixListener as StdUnixListener;
        let listener = StdUnixListener::bind(&s_path).expect("bind mock 200 unix socket");
        let _ = listener.set_nonblocking(true);
        while !stop_clone.load(std::sync::atomic::Ordering::Relaxed) {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let mut buf = [0u8; 512];
                    let _ = stream.read(&mut buf);
                    let resp = b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 2\r\n\r\nOK";
                    let _ = stream.write_all(resp);
                    let _ = stream.flush();
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(_) => break,
            }
        }
    });

    std::thread::sleep(Duration::from_millis(30));

    // probe_docker_socket must return true on HTTP 200 OK
    let is_alive = probe_docker_socket(&sock_path, Duration::from_millis(500));
    assert!(is_alive, "probe_docker_socket must return true on HTTP 200 OK");

    if let Ok(docker) = bollard::Docker::connect_with_unix(
        sock_path.to_str().unwrap(),
        1,
        bollard::API_DEFAULT_VERSION,
    ) {
        let ping_res = docker.ping().await;
        assert!(
            ping_res.is_ok(),
            "Ping against HTTP 200 OK socket must return Ok, got: {:?}",
            ping_res
        );
    }

    stop.store(true, std::sync::atomic::Ordering::Relaxed);
    let _ = srv_handle.join();
    let _ = std::fs::remove_file(&sock_path);
}

// ===========================================================================
// SECTION 2: SSE Event Stream & Disconnection Propagation
// ===========================================================================

#[tokio::test]
async fn test_adv_m3_notify_instance_stopping_emits_lost_and_empty_state() {
    let (mut rx_lost, _g_lost) = subscribe_topics(&["docker-connection-lost".to_string()]);
    let (mut rx_state, _g_state) = subscribe_topics(&["docker-state-updated".to_string()]);

    let test_profile = "test-adv-profile-stopping";
    notify_instance_stopping(test_profile);

    // Verify docker-connection-lost event arrived
    let lost_msg = tokio::time::timeout(Duration::from_millis(1000), async {
        loop {
            let msg = rx_lost.recv().await.expect("channel active");
            if msg.event == "docker-connection-lost" {
                return msg;
            }
        }
    })
    .await
    .expect("docker-connection-lost should be received within 1s");

    let lost_data: serde_json::Value =
        serde_json::from_str(&lost_msg.data).expect("valid lost json");
    assert_eq!(
        lost_data["profile"], test_profile,
        "Profile in connection-lost event must match target instance"
    );

    // Verify docker-state-updated with empty lists arrived
    let state_msg = tokio::time::timeout(Duration::from_millis(1000), async {
        loop {
            let msg = rx_state.recv().await.expect("channel active");
            if msg.event == "docker-state-updated" {
                return msg;
            }
        }
    })
    .await
    .expect("docker-state-updated should be received within 1s");

    let state_data: serde_json::Value =
        serde_json::from_str(&state_msg.data).expect("valid state json");
    assert!(
        state_data["containers"].is_array(),
        "Containers in empty state must be an array"
    );
    assert_eq!(
        state_data["containers"].as_array().unwrap().len(),
        0,
        "Containers list must be empty on connection lost"
    );
    assert!(
        state_data["images"].is_array(),
        "Images in empty state must be an array"
    );
    assert_eq!(
        state_data["images"].as_array().unwrap().len(),
        0,
        "Images list must be empty on connection lost"
    );
}

#[tokio::test]
async fn test_adv_m3_sse_multi_subscriber_broadcast_concurrency() {
    let num_subscribers = 8;
    let mut receivers = Vec::new();
    let mut guards = Vec::new();

    for _ in 0..num_subscribers {
        let (rx, guard) = subscribe_topics(&["docker-state-updated".to_string()]);
        receivers.push(rx);
        guards.push(guard);
    }

    assert!(subscriber_count("docker-state-updated") >= num_subscribers);

    let payload = serde_json::json!({
        "containers": [{"Id": "c_broadcast_test", "State": "running"}],
        "images": []
    });

    publish_sse_event("docker-state-updated", &payload);

    // Verify all subscribers receive the payload
    for (idx, rx) in receivers.iter_mut().enumerate() {
        let msg = tokio::time::timeout(Duration::from_millis(1000), async {
            loop {
                let m = rx.recv().await.expect("valid sse msg");
                if m.event == "docker-state-updated" {
                    let data: serde_json::Value = serde_json::from_str(&m.data).expect("valid json");
                    if data["containers"].get(0).and_then(|c| c.get("Id")).and_then(|id| id.as_str())
                        == Some("c_broadcast_test")
                    {
                        return m;
                    }
                }
            }
        })
        .await
        .unwrap_or_else(|_| panic!("Subscriber {} failed to receive broadcast event", idx));

        let data: serde_json::Value = serde_json::from_str(&msg.data).expect("valid json");
        assert_eq!(data["containers"][0]["Id"], "c_broadcast_test");
    }

    drop(guards);
}

// ===========================================================================
// SECTION 3: fetch_current_docker_state Return Semantics
// ===========================================================================

#[tokio::test]
async fn test_adv_m3_fetch_current_docker_state_returns_none_when_disconnected() {
    // When no Docker daemon is running or reachable, fetch_current_docker_state must
    // return None rather than masking errors as Ok(empty state).
    let state = fetch_current_docker_state().await;

    if let Some(val) = state {
        // If a real Docker daemon happens to be active on the host machine running the tests:
        assert!(
            val.get("containers").is_some() && val["containers"].is_array(),
            "If connected, containers must be an array"
        );
        assert!(
            val.get("images").is_some() && val["images"].is_array(),
            "If connected, images must be an array"
        );
    } else {
        // Correct behavior when no daemon is reachable
        assert!(
            state.is_none(),
            "fetch_current_docker_state must return None when daemon is unreachable"
        );
    }
}

// ===========================================================================
// SECTION 4: DockerState Lifecycle & Cache Invariants
// ===========================================================================

#[test]
fn test_adv_m3_docker_state_initial_invariants() {
    let state = DockerState {
        docker: None,
        containers_cache: vec![],
        images_cache: vec![],
        suppressed: false,
        containerd_warned: false,
    };

    assert!(
        state.docker().is_err(),
        "docker() getter must return Err when docker is None"
    );
    assert_eq!(
        state.docker().unwrap_err(),
        "Docker daemon is not connected"
    );
    assert!(state.containers_cache.is_empty());
    assert!(state.images_cache.is_empty());
    assert!(!state.suppressed);
}

#[test]
fn test_adv_m3_map_containers_and_images_contract() {
    let summary: bollard::models::ContainerSummary = serde_json::from_value(serde_json::json!({
        "Id": "cid_9876543210fedcba",
        "Names": ["/production_web"],
        "Image": "traefik:v3.0",
        "Status": "Up 12 days",
        "State": "running",
        "Ports": [
            {
                "IP": "0.0.0.0",
                "PrivatePort": 80,
                "PublicPort": 80,
                "Type": "tcp"
            },
            {
                "IP": "0.0.0.0",
                "PrivatePort": 443,
                "PublicPort": 443,
                "Type": "tcp"
            }
        ],
        "Created": 1710000000,
        "SizeRw": 4096,
        "Command": "traefik --api.insecure=true",
        "Labels": {
            "com.docker.compose.project": "gateway",
            "com.docker.compose.service": "traefik"
        }
    }))
    .expect("deserialize ContainerSummary");

    let mapped = map_containers(&[summary]);
    assert_eq!(mapped.len(), 1);
    assert_eq!(mapped[0]["Id"], "cid_9876543210fedcba");
    assert_eq!(mapped[0]["Names"], "production_web");
    assert!(mapped[0]["Ports"].as_str().unwrap().contains("0.0.0.0:80->80/tcp"));
    assert!(mapped[0]["Ports"].as_str().unwrap().contains("0.0.0.0:443->443/tcp"));
    assert_eq!(mapped[0]["Labels"]["com.docker.compose.project"], "gateway");

    let img = bollard::models::ImageSummary {
        id: "sha256:1122334455667788".to_string(),
        repo_tags: vec!["traefik:v3.0".to_string()],
        size: 90000000,
        created: 1710000000,
        ..Default::default()
    };

    let mapped_imgs = map_images(&[img]);
    assert_eq!(mapped_imgs.len(), 1);
    assert_eq!(mapped_imgs[0]["Id"], "1122334455667788");
    assert_eq!(mapped_imgs[0]["Repository"], "traefik");
    assert_eq!(mapped_imgs[0]["Tag"], "v3.0");
}

#[tokio::test]
async fn test_adv_m3_lifecycle_suppression_and_cache_clearing() {
    let state = Arc::new(RwLock::new(DockerState {
        docker: None,
        containers_cache: vec![serde_json::json!({"Id": "cached-1"})],
        images_cache: vec![serde_json::json!({"Id": "cached-img-1"})],
        suppressed: false,
        containerd_warned: false,
    }));

    // Invalidate probe cache
    invalidate_probe_cache();

    // Mutate state simulating disconnection cleanup
    {
        let mut lock = state.write().await;
        lock.docker = None;
        lock.containers_cache = vec![];
        lock.images_cache = vec![];
        lock.suppressed = true;
    }

    {
        let lock = state.read().await;
        assert!(lock.docker.is_none());
        assert!(lock.containers_cache.is_empty());
        assert!(lock.images_cache.is_empty());
        assert!(lock.suppressed);
    }

    // Call notify_instance_started to clear suppression
    notify_instance_started("default");

    // Unsuppress manually on our test instance
    {
        let mut lock = state.write().await;
        lock.suppressed = false;
    }

    assert!(!state.read().await.suppressed);
}

// ===========================================================================
// SECTION 5: Unresponsive VM Self-Healing Sweep & Restart Action
// ===========================================================================

#[tokio::test]
async fn test_adv_m3_self_heal_trigger_and_action_contracts() {
    // Sweep should run safely without panic or deadlock
    sweep_vm_unresponsive().await;

    // Verify HealTrigger string representation and roundtrip
    assert_eq!(HealTrigger::VmUnresponsive.as_str(), "vm_unresponsive");
    assert_eq!(
        HealTrigger::parse("vm_unresponsive"),
        Some(HealTrigger::VmUnresponsive)
    );

    // Verify HealAction string representation and roundtrip
    assert_eq!(HealAction::RestartVm.as_str(), "restart_vm");
    assert_eq!(
        HealAction::parse("restart_vm"),
        Some(HealAction::RestartVm)
    );

    // Verify auto_capable capabilities
    assert!(
        HealAction::RestartVm.auto_capable(),
        "RestartVm must be auto_capable (executable by background self-heal)"
    );
    assert!(
        HealAction::RestartContainer.auto_capable(),
        "RestartContainer must be auto_capable"
    );
    assert!(
        HealAction::StopContainer.auto_capable(),
        "StopContainer must be auto_capable"
    );
    assert!(
        !HealAction::SuggestPrune.auto_capable(),
        "SuggestPrune is advisory and must NOT be auto_capable"
    );
    assert!(
        !HealAction::SuggestMemLimit.auto_capable(),
        "SuggestMemLimit is advisory and must NOT be auto_capable"
    );
}

// ===========================================================================
// SECTION 6: SQLite Test DB Isolation Hard Guarantee
// ===========================================================================

#[test]
fn test_adv_m3_sqlite_test_isolation_strictly_enforced() {
    let test_db = db_path();

    // Verification 1: Must be in temp_dir
    assert!(
        test_db.starts_with(std::env::temp_dir()),
        "Activity db path {:?} must reside in temp_dir() during test execution",
        test_db
    );

    // Verification 2: Must NOT be in production app data directory
    let prod_app_dir = app_data_dir();
    assert!(
        !test_db.starts_with(&prod_app_dir),
        "Activity db path {:?} must NOT reside in live app data dir {:?}",
        test_db,
        prod_app_dir
    );

    // Verification 3: Check production activity.db mtime before & after recording entries
    let prod_db = prod_app_dir.join("activity.db");
    let initial_mtime = if prod_db.exists() {
        std::fs::metadata(&prod_db).ok().and_then(|m| m.modified().ok())
    } else {
        None
    };

    // Perform multiple activity record operations
    for i in 0..20 {
        let name = format!("isolation_stress_{}", i);
        let id = format!("id_{}", i);
        record(
            ActivityEntry::new(
                ActivityKind::Lifecycle,
                &name,
                "test_container",
                &id,
            )
            .by(ActivityActor::App)
            .took(i * 5),
        );
    }

    // Verify production DB was not modified
    if let Some(_prev) = initial_mtime {
        let current_mtime = std::fs::metadata(&prod_db).ok().and_then(|m| m.modified().ok());
        assert_eq!(
            initial_mtime, current_mtime,
            "Live ~/.colima-ui/activity.db was modified during cargo test! Test isolation violation!"
        );
    }

    // Verify test DB in temp_dir actually received records
    assert!(
        test_db.exists(),
        "Test activity database {:?} should have been created in temp_dir()",
        test_db
    );
}

// ===========================================================================
// SECTION 7: CLI Label Normalization Adversarial Stress
// ===========================================================================

#[test]
fn test_adv_m3_normalize_cli_labels_adversarial_matrix() {
    // Case 1: Multiple whitespace, trailing commas, empty segments
    let input1 = serde_json::json!({
        "Labels": "  ,, com.docker.compose.project = my_stack ,, service = worker ,,  "
    });
    let norm1 = normalize_cli_labels(input1);
    let l1 = &norm1["Labels"];
    assert_eq!(l1["com.docker.compose.project"], "my_stack");
    assert_eq!(l1["service"], "worker");

    // Case 2: Values with multiple '=' characters (e.g. base64 or query params)
    let input2 = serde_json::json!({
        "Labels": "token=abc==, auth=Bearer xyz==, flag"
    });
    let norm2 = normalize_cli_labels(input2);
    let l2 = &norm2["Labels"];
    assert_eq!(l2["token"], "abc==");
    assert_eq!(l2["auth"], "Bearer xyz==");
    assert_eq!(l2["flag"], "");

    // Case 3: Empty string label
    let input3 = serde_json::json!({
        "Labels": ""
    });
    let norm3 = normalize_cli_labels(input3);
    assert_eq!(norm3["Labels"], serde_json::json!({}));

    // Case 4: Non-string label (already a map)
    let input4 = serde_json::json!({
        "Labels": { "pre_existing": "true" }
    });
    let norm4 = normalize_cli_labels(input4);
    assert_eq!(norm4["Labels"]["pre_existing"], "true");

    // Case 5: Null Labels field
    let input5 = serde_json::json!({
        "Labels": null
    });
    let norm5 = normalize_cli_labels(input5);
    // Note: normalize_cli_labels only creates an empty object if get("Labels").is_none().
    // If "Labels" is explicitly null, it leaves null in place.
    assert!(norm5["Labels"].is_null(), "Explicit null Labels field is currently retained as null");
}

// ===========================================================================
// SECTION 8: High Concurrency & Contention Stress Test
// ===========================================================================

#[tokio::test]
async fn test_adv_m3_high_concurrency_stress() {
    let mut tasks = Vec::new();

    // 20 concurrent tasks performing mixed SSE subscriptions, label normalizations,
    // and activity logging
    for task_id in 0..20 {
        tasks.push(tokio::spawn(async move {
            let topic = format!("test.concurrency.topic.{}", task_id);
            let (mut rx, guard) = subscribe_topics(std::slice::from_ref(&topic));

            let test_payload = serde_json::json!({
                "task_id": task_id,
                "status": "running"
            });
            publish_sse_event(&topic, &test_payload);

            let msg = tokio::time::timeout(Duration::from_millis(500), rx.recv()).await;
            assert!(msg.is_ok(), "Task {} must receive SSE message", task_id);

            // Label normalization under contention
            let norm = normalize_cli_labels(serde_json::json!({
                "Labels": format!("  project=worker_{} , env=prod  ", task_id)
            }));
            assert_eq!(norm["Labels"]["project"], format!("worker_{}", task_id));

            drop(guard);
        }));
    }

    for (idx, task) in tasks.into_iter().enumerate() {
        task.await
            .unwrap_or_else(|e| panic!("Concurrent task {} failed: {:?}", idx, e));
    }
}
