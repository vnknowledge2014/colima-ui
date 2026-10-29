//! Milestone 1 Adversarial Challenge & Stress-Test Suite
//!
//! Empirically tests all M1 socket probe and process liveness behaviors:
//! 1. Immediate EOF (Lima SSH mux mock)
//! 2. Socket probe timeout
//! 3. Stale/orphaned socket files
//! 4. HTTP response parsing (200 OK vs 500 / 404 / adversarial bodies)
//! 5. 2-second TTL cache hit, expiration, and invalidation
//! 6. Process liveness (`is_lima_instance_alive`) across PID/sock states
//! 7. 4-tier instance status transitions (Stopped, Starting, Unhealthy, Running)
//! 8. Multi-threaded concurrency and cache contention

use colima_ui_lib::instance_reader::resolve_instance_status;
use colima_ui_lib::path_util::{
    invalidate_probe_cache, is_lima_instance_alive, probe_docker_socket,
    probe_docker_socket_cached,
};
use std::io::{Read, Write};
use std::net::Shutdown;
use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

fn unique_sock_path(prefix: &str) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    let path = PathBuf::from(format!(
        "/tmp/cui-adv-{}-{}-{}.sock",
        prefix,
        std::process::id(),
        id
    ));
    let _ = std::fs::remove_file(&path);
    path
}

struct AutoCleanupPath(PathBuf);
impl Drop for AutoCleanupPath {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

static ADV_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
fn adv_test_exclusive() -> std::sync::MutexGuard<'static, ()> {
    ADV_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

// ---------------------------------------------------------------------------
// 1. Immediate EOF (Lima SSH Mux Mock)
// ---------------------------------------------------------------------------
#[test]
fn test_adv_immediate_eof_drop_stream() {
    let sock = unique_sock_path("eof-drop");
    let _guard = AutoCleanupPath(sock.clone());
    let listener = UnixListener::bind(&sock).expect("bind socket");

    let srv = std::thread::spawn(move || {
        if let Ok((stream, _)) = listener.accept() {
            // Drop stream immediately without reading or writing
            drop(stream);
        }
    });

    let start = Instant::now();
    let healthy = probe_docker_socket(&sock, Duration::from_millis(300));
    let elapsed = start.elapsed();

    assert!(!healthy, "Immediate EOF (stream drop) must report false");
    assert!(
        elapsed < Duration::from_millis(250),
        "Immediate EOF should return promptly without waiting for timeout, took {:?}",
        elapsed
    );
    let _ = srv.join();
}

#[test]
fn test_adv_immediate_eof_shutdown_both() {
    let sock = unique_sock_path("eof-shut");
    let _guard = AutoCleanupPath(sock.clone());
    let listener = UnixListener::bind(&sock).expect("bind socket");

    let srv = std::thread::spawn(move || {
        if let Ok((stream, _)) = listener.accept() {
            let _ = stream.shutdown(Shutdown::Both);
            drop(stream);
        }
    });

    let healthy = probe_docker_socket(&sock, Duration::from_millis(300));
    assert!(!healthy, "Immediate shutdown must report false");
    let _ = srv.join();
}

// ---------------------------------------------------------------------------
// 2. Socket Probe Timeout Scenario
// ---------------------------------------------------------------------------
#[test]
fn test_adv_socket_probe_timeout_bounded() {
    let sock = unique_sock_path("timeout");
    let _guard = AutoCleanupPath(sock.clone());
    let listener = UnixListener::bind(&sock).expect("bind socket");

    let srv = std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 64];
            let _ = stream.read(&mut buf);
            // Sleep longer than probe timeout (500ms > 100ms)
            std::thread::sleep(Duration::from_millis(500));
        }
    });

    let timeout = Duration::from_millis(100);
    let start = Instant::now();
    let healthy = probe_docker_socket(&sock, timeout);
    let elapsed = start.elapsed();

    assert!(!healthy, "Read timeout must report false");
    assert!(
        elapsed >= Duration::from_millis(90),
        "Probe should wait at least the timeout duration, took {:?}",
        elapsed
    );
    assert!(
        elapsed < Duration::from_millis(350),
        "Probe should time out promptly around 100ms, took {:?}",
        elapsed
    );
    let _ = srv.join();
}

// ---------------------------------------------------------------------------
// 3. Stale / Orphaned Socket Scenarios
// ---------------------------------------------------------------------------
#[test]
fn test_adv_orphaned_socket_file() {
    let sock = unique_sock_path("orphan");
    let _guard = AutoCleanupPath(sock.clone());
    let listener = UnixListener::bind(&sock).expect("bind socket");
    // Explicitly drop listener so file remains on disk with no process listening
    drop(listener);

    assert!(sock.exists(), "Socket file must exist on disk");
    let healthy = probe_docker_socket(&sock, Duration::from_millis(300));
    assert!(!healthy, "Orphaned socket without listener must report false");
}

#[test]
fn test_adv_regular_file_not_socket() {
    let reg_file = unique_sock_path("regular-file");
    let _guard = AutoCleanupPath(reg_file.clone());
    std::fs::write(&reg_file, b"not a unix socket").unwrap();

    assert!(reg_file.exists());
    let healthy = probe_docker_socket(&reg_file, Duration::from_millis(300));
    assert!(!healthy, "Regular file must return false");
}

#[test]
fn test_adv_nonexistent_socket() {
    let non_existent = PathBuf::from("/tmp/nonexistent-path-9876543210.sock");
    assert!(!non_existent.exists());
    let healthy = probe_docker_socket(&non_existent, Duration::from_millis(300));
    assert!(!healthy, "Nonexistent socket must return false");
}

// ---------------------------------------------------------------------------
// 4. HTTP Protocol Response Scenarios
// ---------------------------------------------------------------------------
#[test]
fn test_adv_http_200_ok() {
    let sock = unique_sock_path("http-200");
    let _guard = AutoCleanupPath(sock.clone());
    let listener = UnixListener::bind(&sock).expect("bind socket");

    let srv = std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 128];
            let _ = stream.read(&mut buf);
            let resp = b"HTTP/1.1 200 OK\r\nApi-Version: 1.48\r\nContent-Length: 2\r\n\r\nOK";
            let _ = stream.write_all(resp);
        }
    });

    let healthy = probe_docker_socket(&sock, Duration::from_millis(300));
    assert!(healthy, "HTTP 200 OK must report true");
    let _ = srv.join();
}

#[test]
fn test_adv_http_500_internal_error() {
    let sock = unique_sock_path("http-500");
    let _guard = AutoCleanupPath(sock.clone());
    let listener = UnixListener::bind(&sock).expect("bind socket");

    let srv = std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 128];
            let _ = stream.read(&mut buf);
            let resp = b"HTTP/1.1 500 Internal Server Error\r\nContent-Length: 5\r\n\r\nERROR";
            let _ = stream.write_all(resp);
        }
    });

    let healthy = probe_docker_socket(&sock, Duration::from_millis(300));
    assert!(!healthy, "HTTP 500 must report false");
    let _ = srv.join();
}

#[test]
fn test_adv_http_adversarial_contains_200_in_headers() {
    // Adversarial test: server returns 500 Error, but Content-Length is 200
    // "HTTP/1.1 500 Internal Server Error\r\nContent-Length: 200 \r\n\r\n"
    // Since naive check does head.contains(" 200 "), verify behavior!
    let sock = unique_sock_path("adv-200-header");
    let _guard = AutoCleanupPath(sock.clone());
    let listener = UnixListener::bind(&sock).expect("bind socket");

    let srv = std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 128];
            let _ = stream.read(&mut buf);
            let resp = b"HTTP/1.1 500 Internal Server Error\r\nContent-Length: 200 \r\n\r\n";
            let _ = stream.write_all(resp);
        }
    });

    let healthy = probe_docker_socket(&sock, Duration::from_millis(300));
    assert!(
        !healthy,
        "HTTP 500 with Content-Length 200 header must return false, got true"
    );
    let _ = srv.join();
}

#[test]
fn test_adv_http_adversarial_body_contains_200() {
    let sock = unique_sock_path("adv-200-body");
    let _guard = AutoCleanupPath(sock.clone());
    let listener = UnixListener::bind(&sock).expect("bind socket");

    let srv = std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 128];
            let _ = stream.read(&mut buf);
            let resp = b"HTTP/1.1 503 Service Unavailable\r\nContent-Type: application/json\r\n\r\n{\"error\": \"failed\", \"code\": 200}";
            let _ = stream.write_all(resp);
        }
    });

    let healthy = probe_docker_socket(&sock, Duration::from_millis(300));
    assert!(
        !healthy,
        "HTTP 503 with JSON body containing 200 must return false, got true"
    );
    let _ = srv.join();
}

#[test]
fn test_adv_http_adversarial_status_2000() {
    let sock = unique_sock_path("adv-2000-status");
    let _guard = AutoCleanupPath(sock.clone());
    let listener = UnixListener::bind(&sock).expect("bind socket");

    let srv = std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 128];
            let _ = stream.read(&mut buf);
            let resp = b"HTTP/1.1 2000 Unknown Status\r\n\r\n";
            let _ = stream.write_all(resp);
        }
    });

    let healthy = probe_docker_socket(&sock, Duration::from_millis(300));
    assert!(
        !healthy,
        "HTTP status code 2000 must return false, got true"
    );
    let _ = srv.join();
}

#[test]
fn test_adv_http_adversarial_truncated_status_line() {
    let sock = unique_sock_path("adv-trunc");
    let _guard = AutoCleanupPath(sock.clone());
    let listener = UnixListener::bind(&sock).expect("bind socket");

    let srv = std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 128];
            let _ = stream.read(&mut buf);
            let resp = b"HTTP/1.1";
            let _ = stream.write_all(resp);
        }
    });

    let healthy = probe_docker_socket(&sock, Duration::from_millis(300));
    assert!(
        !healthy,
        "Truncated HTTP status line must return false, got true"
    );
    let _ = srv.join();
}

#[test]
fn test_adv_http_adversarial_ssh_protocol_banner() {
    let sock = unique_sock_path("adv-ssh");
    let _guard = AutoCleanupPath(sock.clone());
    let listener = UnixListener::bind(&sock).expect("bind socket");

    let srv = std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 128];
            let _ = stream.read(&mut buf);
            let resp = b"SSH-2.0-OpenSSH_9.0 200\r\n";
            let _ = stream.write_all(resp);
        }
    });

    let healthy = probe_docker_socket(&sock, Duration::from_millis(300));
    assert!(
        !healthy,
        "SSH banner must return false even if it contains 200, got true"
    );
    let _ = srv.join();
}

#[test]
fn test_adv_malformed_garbage_bytes() {
    let sock = unique_sock_path("garbage");
    let _guard = AutoCleanupPath(sock.clone());
    let listener = UnixListener::bind(&sock).expect("bind socket");

    let srv = std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 128];
            let _ = stream.read(&mut buf);
            let resp = &[0xff, 0xfe, 0x00, 0x01, 0x7f, 0x88, 0x99];
            let _ = stream.write_all(resp);
        }
    });

    let healthy = probe_docker_socket(&sock, Duration::from_millis(300));
    assert!(!healthy, "Malformed binary garbage must report false without panic");
    let _ = srv.join();
}

// ---------------------------------------------------------------------------
// 5. 2-Second TTL Cache: Hit, Expiry, and Invalidation
// ---------------------------------------------------------------------------
#[test]
fn test_adv_cache_hit_and_expiry() {
    let _guard_lock = adv_test_exclusive();
    let sock = unique_sock_path("cache-adv");
    let _guard = AutoCleanupPath(sock.clone());
    let listener = UnixListener::bind(&sock).expect("bind socket");

    let srv = std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 128];
            let _ = stream.read(&mut buf);
            let resp = b"HTTP/1.1 200 OK\r\n\r\nOK";
            let _ = stream.write_all(resp);
        }
    });

    // 1. Initial probe succeeds
    let initial = probe_docker_socket_cached(&sock);
    assert!(initial, "Initial probe must succeed");
    let _ = srv.join();

    // 2. Remove socket from filesystem
    let _ = std::fs::remove_file(&sock);
    assert!(!sock.exists());

    // 3. Cache HIT: within 2s TTL, returns true even though file is gone
    let cached = probe_docker_socket_cached(&sock);
    assert!(cached, "Cache hit must return true within TTL");

    // 4. Cache EXPIRATION: wait > 2.0s
    std::thread::sleep(Duration::from_millis(2100));

    // 5. Now expired: probes filesystem, finds file gone -> returns false
    let expired = probe_docker_socket_cached(&sock);
    assert!(!expired, "Expired cache must re-probe and return false");
}

// ---------------------------------------------------------------------------
// 6. Process Liveness (`is_lima_instance_alive`)
// ---------------------------------------------------------------------------
#[test]
fn test_adv_is_lima_instance_alive_comprehensive() {
    let base_dir = PathBuf::from(format!(
        "/tmp/cui-adv-lima-{}-{}",
        std::process::id(),
        std::time::SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
    ));
    let _ = std::fs::create_dir_all(&base_dir);
    let _guard = AutoCleanupPath(base_dir.clone());

    // Case 1: Empty dir -> false
    assert!(!is_lima_instance_alive(&base_dir));

    // Case 2: ha.pid with dead PID -> false
    let ha_pid = base_dir.join("ha.pid");
    std::fs::write(&ha_pid, "99999999\n").unwrap();
    assert!(!is_lima_instance_alive(&base_dir));

    // Case 3: ha.pid with current live PID -> true
    std::fs::write(&ha_pid, format!("{}\n", std::process::id())).unwrap();
    assert!(is_lima_instance_alive(&base_dir));

    // Case 4: ha.sock exists but ECONNREFUSED (stale socket) -> MUST BE FALSE
    // Even though ha.pid has live PID, ha.sock exists and takes precedence!
    let ha_sock = base_dir.join("ha.sock");
    let stale_listener = UnixListener::bind(&ha_sock).unwrap();
    drop(stale_listener); // leaves dead socket file
    assert!(ha_sock.exists());
    assert!(
        !is_lima_instance_alive(&base_dir),
        "Stale ha.sock with ECONNREFUSED must report false even if ha.pid is alive"
    );

    // Case 5: ha.sock active listener -> true
    let _ = std::fs::remove_file(&ha_sock);
    let live_listener = UnixListener::bind(&ha_sock).unwrap();
    assert!(is_lima_instance_alive(&base_dir), "Active ha.sock must report true");
    drop(live_listener);
}

// ---------------------------------------------------------------------------
// 7. Instance Status Transitions
// ---------------------------------------------------------------------------
#[test]
fn test_adv_instance_status_transitions_strict() {
    let _guard_lock = adv_test_exclusive();
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    let home = PathBuf::from(format!("/tmp/cui-adv-status-home-{}", now));
    let _guard = AutoCleanupPath(home.clone());

    let profile = "adv-test";
    let profile_dir = home.join(profile);
    let lima_dir = home.join("_lima").join(format!("colima-{}", profile));
    std::fs::create_dir_all(&profile_dir).unwrap();
    std::fs::create_dir_all(&lima_dir).unwrap();

    // 1. Lima dead -> "Stopped"
    let status_stopped = resolve_instance_status(&home, profile, &lima_dir, "docker");
    assert_eq!(status_stopped, "Stopped");

    // Make Lima alive via live PID in ha.pid
    let ha_pid = lima_dir.join("ha.pid");
    std::fs::write(&ha_pid, format!("{}\n", std::process::id())).unwrap();
    assert!(is_lima_instance_alive(&lima_dir));

    // 2. Starting: uptime fresh (< 90s), socket dead
    let status_starting = resolve_instance_status(&home, profile, &lima_dir, "docker");
    assert_eq!(status_starting, "Starting");

    // 3. Unhealthy: set mtime on ha.pid to 120s ago (>= 90s)
    let cur_secs = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    let old_time = (cur_secs - 120) as libc::time_t;
    let c_path = std::ffi::CString::new(ha_pid.to_str().unwrap()).unwrap();
    let times = [
        libc::timeval { tv_sec: old_time, tv_usec: 0 },
        libc::timeval { tv_sec: old_time, tv_usec: 0 },
    ];
    unsafe {
        libc::utimes(c_path.as_ptr(), times.as_ptr());
    }

    let status_unhealthy = resolve_instance_status(&home, profile, &lima_dir, "docker");
    assert_eq!(status_unhealthy, "Unhealthy");

    // 4. Running: Docker socket responds 200 OK
    let sock_path = profile_dir.join("docker.sock");
    let listener = UnixListener::bind(&sock_path).unwrap();
    let srv = std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 128];
            let _ = stream.read(&mut buf);
            let resp = b"HTTP/1.1 200 OK\r\n\r\nOK";
            let _ = stream.write_all(resp);
        }
    });

    // Invalidate to avoid any negative cache
    invalidate_probe_cache();
    let status_running = resolve_instance_status(&home, profile, &lima_dir, "docker");
    assert_eq!(status_running, "Running");
    let _ = srv.join();

    // 5. Non-docker runtime (e.g. containerd) -> "Running"
    let status_containerd = resolve_instance_status(&home, profile, &lima_dir, "containerd");
    assert_eq!(status_containerd, "Running");
}

// ---------------------------------------------------------------------------
// 8. High-Concurrency Stress Test
// ---------------------------------------------------------------------------
#[test]
fn test_adv_concurrent_probe_cache_hammer() {
    let _guard_lock = adv_test_exclusive();
    let sock = unique_sock_path("adv-hammer");
    let _guard = AutoCleanupPath(sock.clone());
    let listener = UnixListener::bind(&sock).expect("bind socket");

    let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let stop_srv = stop.clone();

    let srv = std::thread::spawn(move || {
        listener.set_nonblocking(true).unwrap();
        while !stop_srv.load(Ordering::Relaxed) {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let mut buf = [0u8; 128];
                    let _ = stream.read(&mut buf);
                    let resp = b"HTTP/1.1 200 OK\r\n\r\nOK";
                    let _ = stream.write_all(resp);
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(_) => break,
            }
        }
    });

    let mut handles = Vec::new();
    for thread_idx in 0..16 {
        let sock_clone = sock.clone();
        let stop_worker = stop.clone();
        let handle = std::thread::spawn(move || {
            let mut iter = 0;
            while !stop_worker.load(Ordering::Relaxed) && iter < 100 {
                iter += 1;
                if thread_idx % 4 == 0 {
                    invalidate_probe_cache();
                }
                let _ = probe_docker_socket_cached(&sock_clone);
                let dummy = PathBuf::from(format!("/tmp/dummy-{}-{}.sock", thread_idx, iter));
                let _ = probe_docker_socket_cached(&dummy);
            }
        });
        handles.push(handle);
    }

    for h in handles {
        let _ = h.join();
    }
    stop.store(true, Ordering::Relaxed);
    let _ = srv.join();
}
