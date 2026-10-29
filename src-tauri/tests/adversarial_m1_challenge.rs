//! Empirical Adversarial Stress Test Suite for Milestone 1
//!
//! Evaluates:
//! 1. `detect_docker_host` active context prioritization (`colima`, `colima-k8s`, custom)
//! 2. `detect_docker_host` bypassing corrupted/dead `DOCKER_HOST` (EOF, orphan, missing, whitespace)
//! 3. Deterministic alphabetical fallback ordering across arbitrary directory iteration
//! 4. `read_profile_runtime` performance benchmark (no subprocesses) and malformed YAML robustness
//! 5. `probe_docker_socket_cached` high-concurrency multi-threaded stress testing (no deadlocks or race conditions)

use colima_ui_lib::instance_reader::read_profile_runtime_in;
use colima_ui_lib::path_util::{
    detect_docker_host, invalidate_probe_cache, probe_docker_socket_cached,
};
use std::fs;
use std::io::{Read, Write};
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

/// Global serial mutex for tests that modify process environment variables (`COLIMA_HOME`, `DOCKER_CONFIG`, `DOCKER_HOST`).
static SERIAL_ENV_MUTEX: Mutex<()> = Mutex::new(());

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn unique_temp_dir(prefix: &str) -> PathBuf {
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = PathBuf::from(format!(
        "/tmp/cui-adv-test-{}-{}-{}",
        prefix,
        std::process::id(),
        id
    ));
    let _ = fs::create_dir_all(&dir);
    dir
}

struct DirCleanup(PathBuf);
impl Drop for DirCleanup {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Helper that spawns a background thread running an HTTP 200 Unix socket server.
fn spawn_mock_200_server(sock_path: &Path, stop_signal: Arc<AtomicBool>) -> thread::JoinHandle<()> {
    let _ = fs::remove_file(sock_path);
    let listener = UnixListener::bind(sock_path).expect("bind mock unix socket");
    let _ = listener.set_nonblocking(true);

    thread::spawn(move || {
        while !stop_signal.load(Ordering::Relaxed) {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let _ = stream.set_read_timeout(Some(Duration::from_millis(200)));
                    let mut buf = [0u8; 128];
                    let _ = stream.read(&mut buf);
                    let resp =
                        b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 2\r\n\r\nOK";
                    let _ = stream.write_all(resp);
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(5));
                }
                Err(_) => break,
            }
        }
    })
}

/// Helper that spawns a background thread simulating Lima SSH port forwarder EOF behavior
/// (accepts connection, immediately closes without writing anything).
fn spawn_mock_eof_server(sock_path: &Path, stop_signal: Arc<AtomicBool>) -> thread::JoinHandle<()> {
    let _ = fs::remove_file(sock_path);
    let listener = UnixListener::bind(sock_path).expect("bind mock eof socket");
    let _ = listener.set_nonblocking(true);

    thread::spawn(move || {
        while !stop_signal.load(Ordering::Relaxed) {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let _ = stream.set_read_timeout(Some(Duration::from_millis(100)));
                    let mut buf = [0u8; 64];
                    let _ = stream.read(&mut buf);
                    // Close immediately on read
                    drop(stream);
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(5));
                }
                Err(_) => break,
            }
        }
    })
}

/// Helper to configure a mock healthy Colima profile under `colima_home`.
/// Writes a live PID into `_lima/<lima_name>/ha.pid` and starts a mock 200 server on `docker.sock`.
fn setup_healthy_profile(
    colima_home: &Path,
    profile_name: &str,
    stop_signal: Arc<AtomicBool>,
) -> thread::JoinHandle<()> {
    let profile_dir = colima_home.join(profile_name);
    fs::create_dir_all(&profile_dir).unwrap();

    let lima_name = if profile_name == "default" {
        "colima".to_string()
    } else {
        format!("colima-{}", profile_name)
    };
    let lima_dir = colima_home.join("_lima").join(&lima_name);
    fs::create_dir_all(&lima_dir).unwrap();

    // Live PID (current process)
    let pid_file = lima_dir.join("ha.pid");
    fs::write(&pid_file, format!("{}\n", std::process::id())).unwrap();

    let sock_path = profile_dir.join("docker.sock");
    spawn_mock_200_server(&sock_path, stop_signal)
}

#[test]
fn test_detect_docker_host_active_context_prioritization() {
    let _lock = SERIAL_ENV_MUTEX.lock().unwrap();
    invalidate_probe_cache();

    let test_dir = unique_temp_dir("active-context");
    let _cleanup = DirCleanup(test_dir.clone());

    let colima_home = test_dir.join(".colima");
    let docker_dir = test_dir.join(".docker");
    fs::create_dir_all(&colima_home).unwrap();
    fs::create_dir_all(&docker_dir).unwrap();

    std::env::set_var("COLIMA_HOME", colima_home.to_str().unwrap());
    std::env::set_var("DOCKER_CONFIG", docker_dir.to_str().unwrap());
    std::env::remove_var("DOCKER_HOST");

    let stop = Arc::new(AtomicBool::new(false));

    // Setup 3 healthy profiles: "default", "k8s", and "alpha"
    let h1 = setup_healthy_profile(&colima_home, "default", stop.clone());
    let h2 = setup_healthy_profile(&colima_home, "k8s", stop.clone());
    let h3 = setup_healthy_profile(&colima_home, "alpha", stop.clone());

    // Give servers a few ms to start
    thread::sleep(Duration::from_millis(20));

    // Subtest 1: Active context is "colima-k8s" -> should prioritize "k8s"
    {
        invalidate_probe_cache();
        let cfg = serde_json::json!({ "currentContext": "colima-k8s" });
        fs::write(docker_dir.join("config.json"), cfg.to_string()).unwrap();

        let host = detect_docker_host();
        assert!(host.is_some(), "Expected detected host for colima-k8s");
        let (sock, profile) = host.unwrap();
        assert_eq!(
            profile, "k8s",
            "Expected profile 'k8s' to be prioritized over default and alpha, got: {}",
            profile
        );
        assert!(sock.contains("/k8s/docker.sock"));
    }

    // Subtest 2: Active context is "colima" -> should prioritize "default"
    {
        invalidate_probe_cache();
        let cfg = serde_json::json!({ "currentContext": "colima" });
        fs::write(docker_dir.join("config.json"), cfg.to_string()).unwrap();

        let host = detect_docker_host();
        assert!(host.is_some(), "Expected detected host for colima");
        let (sock, profile) = host.unwrap();
        assert_eq!(
            profile, "default",
            "Expected profile 'default' for currentContext: 'colima', got: {}",
            profile
        );
        assert!(sock.contains("/default/docker.sock"));
    }

    // Subtest 3: Active context is "colima-alpha" -> should prioritize "alpha"
    {
        invalidate_probe_cache();
        let cfg = serde_json::json!({ "currentContext": "colima-alpha" });
        fs::write(docker_dir.join("config.json"), cfg.to_string()).unwrap();

        let host = detect_docker_host();
        assert!(host.is_some(), "Expected detected host for colima-alpha");
        let (sock, profile) = host.unwrap();
        assert_eq!(
            profile, "alpha",
            "Expected profile 'alpha' to be prioritized, got: {}",
            profile
        );
        assert!(sock.contains("/alpha/docker.sock"));
    }

    // Subtest 4: Active context is unrelated "desktop-linux" -> should fallback to "default"
    {
        invalidate_probe_cache();
        let cfg = serde_json::json!({ "currentContext": "desktop-linux" });
        fs::write(docker_dir.join("config.json"), cfg.to_string()).unwrap();

        let host = detect_docker_host();
        assert!(host.is_some(), "Expected fallback detected host");
        let (sock, profile) = host.unwrap();
        assert_eq!(
            profile, "default",
            "Expected fallback to 'default' when currentContext is desktop-linux, got: {}",
            profile
        );
        assert!(sock.contains("/default/docker.sock"));
    }

    // Clean up mock servers
    stop.store(true, Ordering::Relaxed);
    let _ = h1.join();
    let _ = h2.join();
    let _ = h3.join();

    std::env::remove_var("COLIMA_HOME");
    std::env::remove_var("DOCKER_CONFIG");
}

#[test]
fn test_detect_docker_host_bypasses_corrupted_or_dead_docker_host() {
    let _lock = SERIAL_ENV_MUTEX.lock().unwrap();
    invalidate_probe_cache();

    let test_dir = unique_temp_dir("dead-docker-host");
    let _cleanup = DirCleanup(test_dir.clone());

    let colima_home = test_dir.join(".colima");
    fs::create_dir_all(&colima_home).unwrap();
    std::env::set_var("COLIMA_HOME", colima_home.to_str().unwrap());
    std::env::remove_var("DOCKER_CONFIG");

    let stop = Arc::new(AtomicBool::new(false));
    let h_healthy = setup_healthy_profile(&colima_home, "default", stop.clone());

    thread::sleep(Duration::from_millis(20));

    // Case 1: DOCKER_HOST points to a completely nonexistent socket file
    {
        invalidate_probe_cache();
        let dead_sock = test_dir.join("nonexistent.sock");
        std::env::set_var("DOCKER_HOST", format!("unix://{}", dead_sock.display()));

        let detected = detect_docker_host();
        assert!(
            detected.is_some(),
            "Must fall back to healthy Colima profile when DOCKER_HOST does not exist"
        );
        let (sock, profile) = detected.unwrap();
        assert_eq!(profile, "default");
        assert!(
            sock.contains("/default/docker.sock"),
            "Expected healthy profile socket, got: {}",
            sock
        );
    }

    // Case 2: DOCKER_HOST points to an orphaned socket file (file exists on disk, no process listening)
    {
        invalidate_probe_cache();
        let orphan_sock = test_dir.join("orphan.sock");
        let l = UnixListener::bind(&orphan_sock).unwrap();
        drop(l); // Socket file remains, but no listener
        assert!(orphan_sock.exists());

        std::env::set_var("DOCKER_HOST", format!("unix://{}", orphan_sock.display()));
        let detected = detect_docker_host();
        assert!(
            detected.is_some(),
            "Must fall back to healthy Colima profile when DOCKER_HOST is orphaned socket"
        );
        let (sock, profile) = detected.unwrap();
        assert_eq!(profile, "default");
        assert!(sock.contains("/default/docker.sock"));
    }

    // Case 3: DOCKER_HOST points to an EOF socket (accepts but sends 0-byte EOF immediately)
    {
        invalidate_probe_cache();
        let eof_sock = test_dir.join("eof.sock");
        let stop_eof = Arc::new(AtomicBool::new(false));
        let h_eof = spawn_mock_eof_server(&eof_sock, stop_eof.clone());
        thread::sleep(Duration::from_millis(15));

        std::env::set_var("DOCKER_HOST", format!("unix://{}", eof_sock.display()));
        let detected = detect_docker_host();
        assert!(
            detected.is_some(),
            "Must fall back to healthy Colima profile when DOCKER_HOST gives immediate EOF"
        );
        let (sock, profile) = detected.unwrap();
        assert_eq!(profile, "default");
        assert!(sock.contains("/default/docker.sock"));

        stop_eof.store(true, Ordering::Relaxed);
        let _ = h_eof.join();
    }

    // Case 4: DOCKER_HOST is whitespace or empty
    {
        invalidate_probe_cache();
        std::env::set_var("DOCKER_HOST", "   \t\n ");
        let detected = detect_docker_host();
        assert!(
            detected.is_some(),
            "Must fall back to healthy Colima profile when DOCKER_HOST is whitespace"
        );
        let (sock, profile) = detected.unwrap();
        assert_eq!(profile, "default");
        assert!(sock.contains("/default/docker.sock"));
    }

    // Case 5: DOCKER_HOST is a genuinely healthy live Unix socket
    {
        invalidate_probe_cache();
        let live_sock = test_dir.join("live_custom.sock");
        let stop_live = Arc::new(AtomicBool::new(false));
        let h_live = spawn_mock_200_server(&live_sock, stop_live.clone());
        thread::sleep(Duration::from_millis(15));

        std::env::set_var("DOCKER_HOST", format!("unix://{}", live_sock.display()));
        let detected = detect_docker_host();
        assert!(
            detected.is_some(),
            "Must use DOCKER_HOST when socket is alive and healthy"
        );
        let (sock, profile) = detected.unwrap();
        assert_eq!(sock, format!("unix://{}", live_sock.display()));
        assert_eq!(profile, "default");

        stop_live.store(true, Ordering::Relaxed);
        let _ = h_live.join();
    }

    stop.store(true, Ordering::Relaxed);
    let _ = h_healthy.join();

    std::env::remove_var("COLIMA_HOME");
    std::env::remove_var("DOCKER_HOST");
}

#[test]
fn test_detect_docker_host_deterministic_alphabetical_fallback() {
    let _lock = SERIAL_ENV_MUTEX.lock().unwrap();
    invalidate_probe_cache();

    let test_dir = unique_temp_dir("alphabetical-order");
    let _cleanup = DirCleanup(test_dir.clone());

    let colima_home = test_dir.join(".colima");
    fs::create_dir_all(&colima_home).unwrap();
    std::env::set_var("COLIMA_HOME", colima_home.to_str().unwrap());
    std::env::remove_var("DOCKER_CONFIG");
    std::env::remove_var("DOCKER_HOST");

    let stop = Arc::new(AtomicBool::new(false));

    // Notice: we do NOT create "default". We create "zebra", "beta", "alpha", "gamma".
    // All 4 are healthy.
    let h_zebra = setup_healthy_profile(&colima_home, "zebra", stop.clone());
    let h_beta = setup_healthy_profile(&colima_home, "beta", stop.clone());
    let h_alpha = setup_healthy_profile(&colima_home, "alpha", stop.clone());
    let h_gamma = setup_healthy_profile(&colima_home, "gamma", stop.clone());

    thread::sleep(Duration::from_millis(25));

    // Run 50 iterations. Without deterministic sorting, filesystem directory iteration
    // order could return "beta", "gamma", or "zebra" randomly depending on hash/inode order.
    for i in 0..50 {
        invalidate_probe_cache();
        let detected = detect_docker_host();
        assert!(detected.is_some(), "Iteration {}: expected detected host", i);
        let (_, profile) = detected.unwrap();
        assert_eq!(
            profile, "alpha",
            "Iteration {}: Expected deterministic alphabetical first 'alpha', but got '{}'",
            i, profile
        );
    }

    // Now simulate 'alpha' stopping (e.g. PID file deleted or made dead)
    let alpha_pid = colima_home.join("_lima/colima-alpha/ha.pid");
    fs::write(&alpha_pid, "999999999\n").unwrap(); // Dead PID

    invalidate_probe_cache();
    let detected_after_alpha_stop = detect_docker_host();
    assert!(detected_after_alpha_stop.is_some());
    let (_, profile2) = detected_after_alpha_stop.unwrap();
    assert_eq!(
        profile2, "beta",
        "When 'alpha' is stopped, next alphabetical candidate 'beta' must be chosen"
    );

    // Now simulate 'beta' stopping
    let beta_pid = colima_home.join("_lima/colima-beta/ha.pid");
    fs::write(&beta_pid, "999999999\n").unwrap();

    invalidate_probe_cache();
    let detected_after_beta_stop = detect_docker_host();
    assert!(detected_after_beta_stop.is_some());
    let (_, profile3) = detected_after_beta_stop.unwrap();
    assert_eq!(
        profile3, "gamma",
        "When 'alpha' and 'beta' are stopped, 'gamma' must be chosen"
    );

    stop.store(true, Ordering::Relaxed);
    let _ = h_zebra.join();
    let _ = h_beta.join();
    let _ = h_alpha.join();
    let _ = h_gamma.join();

    std::env::remove_var("COLIMA_HOME");
}

#[test]
fn test_read_profile_runtime_benchmark_and_robustness() {
    let test_dir = unique_temp_dir("runtime-benchmark");
    let _cleanup = DirCleanup(test_dir.clone());

    let profile_dir = test_dir.join("test-profile");
    fs::create_dir_all(&profile_dir).unwrap();

    // 1. Missing file -> should return "docker" without panic or delay
    assert_eq!(read_profile_runtime_in(&test_dir, "test-profile"), "docker");

    // 2. Syntax-corrupted YAML -> should return "docker" gracefully
    let yaml_file = profile_dir.join("colima.yaml");
    fs::write(&yaml_file, "::: this is completely broken yaml :::\n\t\t\x00\x01").unwrap();
    assert_eq!(read_profile_runtime_in(&test_dir, "test-profile"), "docker");

    // 3. Empty YAML -> "docker"
    fs::write(&yaml_file, "").unwrap();
    assert_eq!(read_profile_runtime_in(&test_dir, "test-profile"), "docker");

    // 4. YAML with integer runtime -> "docker"
    fs::write(&yaml_file, "runtime: 12345\n").unwrap();
    assert_eq!(read_profile_runtime_in(&test_dir, "test-profile"), "docker");

    // 5. YAML with empty runtime string -> "docker"
    fs::write(&yaml_file, "runtime: \"\"\n").unwrap();
    assert_eq!(read_profile_runtime_in(&test_dir, "test-profile"), "docker");

    // 6. Valid YAML with containerd -> "containerd"
    fs::write(&yaml_file, "runtime: containerd\n").unwrap();
    assert_eq!(read_profile_runtime_in(&test_dir, "test-profile"), "containerd");

    // 7. Valid YAML with incus -> "incus"
    fs::write(&yaml_file, "runtime: incus\nkubernetes:\n  enabled: false\n").unwrap();
    assert_eq!(read_profile_runtime_in(&test_dir, "test-profile"), "incus");

    // 8. Path traversal in profile name -> rejected by validation, returns "docker"
    assert_eq!(read_profile_runtime_in(&test_dir, "../../../etc"), "docker");
    assert_eq!(read_profile_runtime_in(&test_dir, "foo/bar"), "docker");

    // 9. Benchmark: Execute 1,000 reads of colima.yaml directly
    // If a subprocess like `colima status` were being spawned, 1,000 calls would take > 150 seconds!
    // Direct file reading and YAML parsing takes under ~1.5 seconds even in unoptimized debug build (~0.4ms/read).
    let start = Instant::now();
    for _ in 0..1000 {
        let r = read_profile_runtime_in(&test_dir, "test-profile");
        assert_eq!(r, "incus");
    }
    let elapsed = start.elapsed();
    println!("Benchmark: 1,000 direct profile runtime reads took {:?}", elapsed);
    assert!(
        elapsed < Duration::from_millis(1500),
        "1,000 direct file reads must be sub-1500ms in debug build (took {:?}), proving no subprocess is spawned",
        elapsed
    );
}

#[test]
fn test_probe_docker_socket_cached_concurrency_stress() {
    invalidate_probe_cache();

    let test_dir = unique_temp_dir("concurrency-stress");
    let _cleanup = DirCleanup(test_dir.clone());

    let healthy_sock = test_dir.join("healthy.sock");
    let eof_sock = test_dir.join("eof.sock");
    let orphan_sock = test_dir.join("orphan.sock");
    let nonexist_sock = test_dir.join("nonexistent.sock");

    let stop = Arc::new(AtomicBool::new(false));
    let h_healthy = spawn_mock_200_server(&healthy_sock, stop.clone());
    let h_eof = spawn_mock_eof_server(&eof_sock, stop.clone());

    let orphan_listener = UnixListener::bind(&orphan_sock).unwrap();
    drop(orphan_listener);

    thread::sleep(Duration::from_millis(20));

    // Spawn 50 worker threads that concurrently hammer probe_docker_socket_cached
    // and randomly invalidate the cache.
    let thread_count = 50;
    let iterations_per_thread = 50;
    let mut handles = Vec::new();

    let start_barrier = Arc::new(std::sync::Barrier::new(thread_count));

    for thread_idx in 0..thread_count {
        let b = start_barrier.clone();
        let s_healthy = healthy_sock.clone();
        let s_eof = eof_sock.clone();
        let s_orphan = orphan_sock.clone();
        let s_nonexist = nonexist_sock.clone();

        handles.push(thread::spawn(move || {
            b.wait();

            for iter in 0..iterations_per_thread {
                // Interleave cache invalidations on some threads
                if thread_idx % 7 == 0 && iter % 10 == 0 {
                    invalidate_probe_cache();
                }

                // Healthy socket should ALWAYS return true
                let res_healthy = probe_docker_socket_cached(&s_healthy);
                assert!(
                    res_healthy,
                    "Healthy socket must probe true under concurrent access"
                );

                // EOF socket should ALWAYS return false
                let res_eof = probe_docker_socket_cached(&s_eof);
                assert!(
                    !res_eof,
                    "EOF socket must probe false under concurrent access"
                );

                // Orphan socket should ALWAYS return false
                let res_orphan = probe_docker_socket_cached(&s_orphan);
                assert!(
                    !res_orphan,
                    "Orphan socket must probe false under concurrent access"
                );

                // Nonexistent socket should ALWAYS return false
                let res_nonexist = probe_docker_socket_cached(&s_nonexist);
                assert!(
                    !res_nonexist,
                    "Nonexistent socket must probe false under concurrent access"
                );
            }
        }));
    }

    for h in handles {
        h.join().expect("Worker thread panicked or deadlocked!");
    }

    stop.store(true, Ordering::Relaxed);
    let _ = h_healthy.join();
    let _ = h_eof.join();
}
