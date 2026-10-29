//! Milestone 2 Adversarial Challenge & Stress-Test Suite
//!
//! Empirically challenges and stress-tests Milestone 2 backend hardening:
//! 1. `StartConfig` Serde Deserialization Robustness:
//!    - Missing fields, empty objects, and partial payloads (`#[serde(default)]`)
//!    - Empty strings and special characters (Unicode, spaces, paths)
//!    - Unexpected unknown fields (tolerance)
//!    - Null field rejection (error handling without panic)
//!    - Negative integer rejection (error handling without panic)
//!    - Extreme integers (u64::MAX, u32::MAX, and overflow rejection)
//!    - Byte-to-GiB conversion edge cases (0, 1, 1073741823, 1073741824, 8589934592, 100 GiB, etc.)
//!    - Type mismatch rejections (strings for numbers, arrays for scalars, etc.)
//! 2. Subprocess Timeout Cancellation & Process Termination (`runtime::run`):
//!    - Prompt timeout error return (`Docker command timed out (daemon may be unresponsive)`)
//!    - Instant cancellation with sub-millisecond timeouts
//!    - Verification of `kill_on_drop(true)` terminating child process via PID and `libc::kill`
//!    - Verification that no orphaned background processes leak after timeouts
//!    - Verification of tiered timeout constants (DEFAULT_TIMEOUT 15s, LONG_OP_TIMEOUT 180s)
//! 3. High Concurrency Stress Test:
//!    - 50 concurrent OS threads executing 5,000 parallel deserializations and normalizations
//!    - High-throughput asynchronous Tokio tasks under maximum simultaneous contention

use colima_ui_lib::commands::colima::{normalize_bytes_to_gib, StartConfig, ONE_GIB_BYTES};
use colima_ui_lib::commands::runtime::{run, DEFAULT_TIMEOUT, LONG_OP_TIMEOUT};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Barrier};
use std::time::{Duration, Instant};

// ===========================================================================
// SECTION 1: StartConfig Serde Deserialization Robustness
// ===========================================================================

#[test]
fn test_adv_start_config_empty_payload() {
    let raw = "{}";
    let cfg: StartConfig = serde_json::from_str(raw).expect("Empty payload must deserialize cleanly");
    assert_eq!(cfg.profile, "");
    assert_eq!(cfg.runtime, "");
    assert_eq!(cfg.cpus, 0);
    assert_eq!(cfg.memory, 0);
    assert_eq!(cfg.disk, 0);
    assert_eq!(cfg.vm_type, "");
    assert!(!cfg.kubernetes);
    assert_eq!(cfg.kubernetes_version, "");
    assert_eq!(cfg.arch, "");
    assert_eq!(cfg.mount_type, "");
    assert!(cfg.mounts.is_empty());
    assert!(cfg.dns.is_empty());
    assert!(!cfg.network_address);
}

#[test]
fn test_adv_start_config_partial_payloads() {
    // Only profile
    let raw1 = r#"{"profile": "prod-cluster"}"#;
    let cfg1: StartConfig = serde_json::from_str(raw1).expect("Partial profile payload must deserialize");
    assert_eq!(cfg1.profile, "prod-cluster");
    assert_eq!(cfg1.cpus, 0);
    assert_eq!(cfg1.memory, 0);

    // Only resources
    let raw2 = r#"{"memory": 8, "disk": 60, "cpus": 4}"#;
    let cfg2: StartConfig = serde_json::from_str(raw2).expect("Partial resource payload must deserialize");
    assert_eq!(cfg2.profile, "");
    assert_eq!(cfg2.memory, 8);
    assert_eq!(cfg2.disk, 60);
    assert_eq!(cfg2.cpus, 4);

    // Only boolean and list flags
    let raw3 = r#"{"kubernetes": true, "mounts": ["/Users/mike/workspace:/workspace:w"], "network_address": true}"#;
    let cfg3: StartConfig = serde_json::from_str(raw3).expect("Partial flag payload must deserialize");
    assert!(cfg3.kubernetes);
    assert!(cfg3.network_address);
    assert_eq!(cfg3.mounts, vec!["/Users/mike/workspace:/workspace:w".to_string()]);
    assert_eq!(cfg3.profile, "");
}

#[test]
fn test_adv_start_config_empty_strings_and_unicode() {
    let raw = r#"{
        "profile": "🚀 colima-dev 🚀",
        "runtime": "docker",
        "vm_type": "vz",
        "kubernetes_version": "v1.31.1+k3s1",
        "arch": "aarch64",
        "mount_type": "virtiofs",
        "mounts": ["/Users/mike/Documents/colima-ui:/colima-ui"],
        "dns": ["1.1.1.1", "8.8.8.8"]
    }"#;
    let cfg: StartConfig = serde_json::from_str(raw).expect("Unicode and special characters must deserialize cleanly");
    assert_eq!(cfg.profile, "🚀 colima-dev 🚀");
    assert_eq!(cfg.runtime, "docker");
    assert_eq!(cfg.vm_type, "vz");
    assert_eq!(cfg.kubernetes_version, "v1.31.1+k3s1");
    assert_eq!(cfg.arch, "aarch64");
    assert_eq!(cfg.mount_type, "virtiofs");
    assert_eq!(cfg.mounts.len(), 1);
    assert_eq!(cfg.dns.len(), 2);
}

#[test]
fn test_adv_start_config_unknown_fields_tolerated() {
    let raw = r#"{
        "profile": "test-profile",
        "memory": 4,
        "unexpected_future_flag": true,
        "nested_metadata": { "author": "tester", "tags": [1, 2, 3] },
        "arbitrary_number": 42.5
    }"#;
    let cfg: StartConfig = serde_json::from_str(raw).expect("Unknown extraneous fields must be safely ignored");
    assert_eq!(cfg.profile, "test-profile");
    assert_eq!(cfg.memory, 4);
}

#[test]
fn test_adv_start_config_null_rejection_error_handling() {
    // Scalar and string fields cannot be null — serde must return an Err without panicking
    let null_cases = [
        r#"{"profile": null}"#,
        r#"{"memory": null}"#,
        r#"{"disk": null}"#,
        r#"{"cpus": null}"#,
        r#"{"kubernetes": null}"#,
        r#"{"mounts": null}"#,
        r#"{"dns": null}"#,
        r#"{"network_address": null}"#,
    ];

    for case in null_cases {
        let res: Result<StartConfig, _> = serde_json::from_str(case);
        assert!(
            res.is_err(),
            "Expected Serde Err when field is null in payload '{}', but got Ok: {:?}",
            case,
            res.unwrap()
        );
    }
}

#[test]
fn test_adv_start_config_negative_numbers_error_handling() {
    let negative_cases = [
        r#"{"memory": -1}"#,
        r#"{"memory": -1073741824}"#,
        r#"{"disk": -60}"#,
        r#"{"disk": -9223372036854775808}"#,
        r#"{"cpus": -4}"#,
        r#"{"cpus": -1}"#,
    ];

    for case in negative_cases {
        let res: Result<StartConfig, _> = serde_json::from_str(case);
        assert!(
            res.is_err(),
            "Expected Serde Err for negative integer in payload '{}', but got Ok: {:?}",
            case,
            res.unwrap()
        );
    }
}

#[test]
fn test_adv_start_config_extreme_integers_and_overflow() {
    // 1. u64::MAX for memory and disk must deserialize successfully without truncation
    let u64_max_str = u64::MAX.to_string();
    let raw_max = format!(r#"{{"memory": {}, "disk": {}}}"#, u64_max_str, u64_max_str);
    let mut cfg: StartConfig = serde_json::from_str(&raw_max).expect("u64::MAX must deserialize into memory and disk");
    assert_eq!(cfg.memory, u64::MAX);
    assert_eq!(cfg.disk, u64::MAX);

    // Normalizing u64::MAX should not overflow or panic
    cfg.normalize_resources();
    let expected_gib = u64::MAX / ONE_GIB_BYTES; // 17,179,869,183 GiB
    assert_eq!(cfg.memory, expected_gib);
    assert_eq!(cfg.disk, expected_gib);

    // 2. u32::MAX for cpus must deserialize successfully
    let u32_max_str = u32::MAX.to_string();
    let raw_cpu_max = format!(r#"{{"cpus": {}}}"#, u32_max_str);
    let cfg_cpu: StartConfig = serde_json::from_str(&raw_cpu_max).expect("u32::MAX must deserialize into cpus");
    assert_eq!(cfg_cpu.cpus, u32::MAX);

    // 3. Values exceeding u64::MAX must return Serde Err without panicking
    let overflow_u64 = "18446744073709551616"; // u64::MAX + 1
    let raw_overflow = format!(r#"{{"memory": {}}}"#, overflow_u64);
    let res_overflow: Result<StartConfig, _> = serde_json::from_str(&raw_overflow);
    assert!(res_overflow.is_err(), "u64 overflow must be rejected as an error");

    // 4. Values exceeding u32::MAX for cpus must return Serde Err without panicking
    let overflow_u32 = "4294967296"; // u32::MAX + 1
    let raw_cpu_overflow = format!(r#"{{"cpus": {}}}"#, overflow_u32);
    let res_cpu_overflow: Result<StartConfig, _> = serde_json::from_str(&raw_cpu_overflow);
    assert!(res_cpu_overflow.is_err(), "u32 overflow for cpus must be rejected as an error");

    // 5. Arbitrary giant integer string
    let huge_int = r#"{"memory": 999999999999999999999999999999999999999999999999999}"#;
    let res_huge: Result<StartConfig, _> = serde_json::from_str(huge_int);
    assert!(res_huge.is_err(), "Massive integer beyond representation must be rejected");
}

#[test]
fn test_adv_start_config_invalid_types_rejected() {
    let type_error_cases = [
        r#"{"memory": "8GB"}"#,
        r#"{"cpus": "four"}"#,
        r#"{"kubernetes": "true"}"#,
        r#"{"mounts": "/var/run/docker.sock"}"#, // string instead of array
        r#"{"mounts": [123, 456]}"#,            // integer array instead of string array
        r#"{"dns": "8.8.8.8"}"#,                // string instead of array
        r#"{"network_address": 1}"#,            // integer instead of bool
        r#"{"profile": ["default"]}"#,          // array instead of string
    ];

    for case in type_error_cases {
        let res: Result<StartConfig, _> = serde_json::from_str(case);
        assert!(
            res.is_err(),
            "Expected Serde Err for mismatched type in payload '{}', but got Ok: {:?}",
            case,
            res.unwrap()
        );
    }
}

// ===========================================================================
// SECTION 2: Byte-to-GiB Conversion Edge Cases
// ===========================================================================

#[test]
fn test_adv_byte_to_gib_conversion_exhaustive_cases() {
    // 1. Boundary: 0 bytes -> 0
    assert_eq!(normalize_bytes_to_gib(0), 0);

    // 2. Boundary: 1 byte -> 1 (treated as already GiB integer or tiny val)
    assert_eq!(normalize_bytes_to_gib(1), 1);

    // 3. Boundary: Small integers (typical user GiB values) -> unchanged
    assert_eq!(normalize_bytes_to_gib(2), 2);
    assert_eq!(normalize_bytes_to_gib(4), 4);
    assert_eq!(normalize_bytes_to_gib(8), 8);
    assert_eq!(normalize_bytes_to_gib(16), 16);
    assert_eq!(normalize_bytes_to_gib(60), 60);
    assert_eq!(normalize_bytes_to_gib(100), 100);
    assert_eq!(normalize_bytes_to_gib(1024), 1024);

    // 4. Exact predecessor of 1 GiB: 1073741823 bytes -> 1073741823
    let one_gib_minus_one = ONE_GIB_BYTES - 1; // 1_073_741_823
    assert_eq!(one_gib_minus_one, 1_073_741_823);
    assert_eq!(normalize_bytes_to_gib(one_gib_minus_one), 1_073_741_823);

    // 5. Exact 1 GiB: 1073741824 bytes -> 1
    assert_eq!(ONE_GIB_BYTES, 1_073_741_824);
    assert_eq!(normalize_bytes_to_gib(ONE_GIB_BYTES), 1);

    // 6. 1 GiB + 1 byte: 1073741825 bytes -> 1
    assert_eq!(normalize_bytes_to_gib(ONE_GIB_BYTES + 1), 1);

    // 7. 1.5 GiB: 1_610_612_736 bytes -> 1 (integer division clamps to 1)
    let one_point_five_gib = ONE_GIB_BYTES + (ONE_GIB_BYTES / 2);
    assert_eq!(normalize_bytes_to_gib(one_point_five_gib), 1);

    // 8. 2 GiB - 1 byte: 2_147_483_647 bytes -> 1
    let two_gib_minus_one = (2 * ONE_GIB_BYTES) - 1;
    assert_eq!(normalize_bytes_to_gib(two_gib_minus_one), 1);

    // 9. Exact 2 GiB: 2_147_483_648 bytes -> 2
    let two_gib = 2 * ONE_GIB_BYTES;
    assert_eq!(normalize_bytes_to_gib(two_gib), 2);

    // 10. Exact 8 GiB: 8_589_934_592 bytes -> 8
    let eight_gib = 8 * ONE_GIB_BYTES; // 8_589_934_592
    assert_eq!(eight_gib, 8_589_934_592);
    assert_eq!(normalize_bytes_to_gib(eight_gib), 8);

    // 11. Exact 100 GiB: 107_374_182_400 bytes -> 100
    let hundred_gib = 100 * ONE_GIB_BYTES; // 107_374_182_400
    assert_eq!(hundred_gib, 107_374_182_400);
    assert_eq!(normalize_bytes_to_gib(hundred_gib), 100);

    // 12. Non-exact large value: 8.9 GiB in bytes -> 8
    let eight_point_nine_gib = eight_gib + (ONE_GIB_BYTES * 9 / 10);
    assert_eq!(normalize_bytes_to_gib(eight_point_nine_gib), 8);

    // 13. Very large storage: 1 TiB in bytes (1024 GiB) -> 1024
    let one_tib_bytes = 1024 * ONE_GIB_BYTES;
    assert_eq!(normalize_bytes_to_gib(one_tib_bytes), 1024);
}

#[test]
fn test_adv_start_config_normalize_resources_lifecycle() {
    let raw = r#"{
        "profile": "large-node",
        "cpus": 16,
        "memory": 8589934592,
        "disk": 107374182400
    }"#;
    let mut cfg: StartConfig = serde_json::from_str(raw).expect("Valid payload with byte units");
    assert_eq!(cfg.memory, 8_589_934_592);
    assert_eq!(cfg.disk, 107_374_182_400);

    cfg.normalize_resources();
    assert_eq!(cfg.memory, 8, "8589934592 bytes must normalize to 8 GiB");
    assert_eq!(cfg.disk, 100, "107374182400 bytes must normalize to 100 GiB");

    // Re-normalizing already normalized config is idempotent
    cfg.normalize_resources();
    assert_eq!(cfg.memory, 8);
    assert_eq!(cfg.disk, 100);
}

// ===========================================================================
// SECTION 3: Subprocess Timeout Cancellation & Process Termination
// ===========================================================================

static ADV_PROC_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[tokio::test]
async fn test_adv_subprocess_timeout_returns_promptly() {
    let _lock = ADV_PROC_LOCK.lock().await;
    let start = Instant::now();
    // Requesting `events` from docker without any events will wait indefinitely if not cancelled.
    // Timeout is set to 150ms.
    let timeout = Duration::from_millis(150);
    let result = run(vec!["events".to_string()], timeout).await;
    let elapsed = start.elapsed();

    assert!(result.is_err(), "Commands that outlive timeout must return Err");
    let err_msg = result.unwrap_err();
    assert!(
        err_msg.contains("Docker command timed out"),
        "Error message must clearly state Docker command timed out, got: '{}'",
        err_msg
    );

    // Must return promptly around the 150ms mark, well within a reasonable bound (< 800ms)
    assert!(
        elapsed >= Duration::from_millis(130),
        "Timeout must wait at least the configured duration, elapsed: {:?}",
        elapsed
    );
    assert!(
        elapsed < Duration::from_millis(800),
        "Timeout must terminate promptly without hanging, took: {:?}",
        elapsed
    );
}

#[tokio::test]
async fn test_adv_subprocess_tiny_timeout_cancels_instantly() {
    let start = Instant::now();
    let result = run(vec!["version".to_string()], Duration::from_nanos(1)).await;
    let elapsed = start.elapsed();

    assert!(result.is_err(), "1-nanosecond deadline must fail with timeout");
    let err_msg = result.unwrap_err();
    assert!(
        err_msg.contains("Docker command timed out"),
        "Must report timeout error, got: '{}'",
        err_msg
    );
    assert!(
        elapsed < Duration::from_millis(100),
        "Instant timeout should return in < 100ms, took: {:?}",
        elapsed
    );
}

#[tokio::test]
async fn test_adv_subprocess_kill_on_drop_terminates_child_process() {
    // Empirically verify the core contract that tokio::process::Command with kill_on_drop(true)
    // terminates the underlying OS process when its execution future is dropped on timeout.
    let mut cmd = tokio::process::Command::new("/bin/sleep");
    cmd.arg("60");
    cmd.kill_on_drop(true);

    let mut child = cmd.spawn().expect("Spawn /bin/sleep 60");
    let pid = child.id().expect("Child must have PID") as libc::pid_t;

    // Verify process is currently running
    let is_alive_initially = unsafe { libc::kill(pid, 0) == 0 };
    assert!(is_alive_initially, "Child /bin/sleep process with PID {} must be running initially", pid);

    // Timeout after 50ms while child wants to sleep for 60 seconds
    let res = tokio::time::timeout(Duration::from_millis(50), child.wait()).await;
    assert!(res.is_err(), "Child wait must time out");

    // Dropping `child` happens here as it goes out of scope or is explicitly dropped
    drop(child);

    // Allow a brief moment (50ms) for OS signal delivery (SIGKILL)
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Verify process is now terminated
    let kill_ret = unsafe { libc::kill(pid, 0) };
    let errno = std::io::Error::last_os_error().raw_os_error();

    assert_eq!(
        kill_ret,
        -1,
        "Child process {} must no longer exist after drop with kill_on_drop(true)",
        pid
    );
    assert_eq!(
        errno,
        Some(libc::ESRCH),
        "Expected ESRCH (No such process), got: {:?}",
        errno
    );
}

#[tokio::test]
async fn test_adv_runtime_run_repeated_timeouts_no_process_leak() {
    let _lock = ADV_PROC_LOCK.lock().await;

    fn count_docker_events_processes() -> usize {
        let output = std::process::Command::new("pgrep")
            .args(["-f", "docker events"])
            .output();
        match output {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                stdout.lines().filter(|l| !l.trim().is_empty()).count()
            }
            Err(_) => 0,
        }
    }

    let initial_count = count_docker_events_processes();
    assert_eq!(
        initial_count, 0,
        "Precondition: No docker events processes should be running before test"
    );

    // Fire 5 sequential commands that time out
    for _ in 0..5 {
        let res = run(vec!["events".to_string()], Duration::from_millis(60)).await;
        assert!(res.is_err(), "Must time out");
    }

    // Give time for any background cleanup
    tokio::time::sleep(Duration::from_millis(100)).await;

    let final_count = count_docker_events_processes();
    assert_eq!(
        final_count, 0,
        "Running timed-out commands must not leak orphaned processes! Found: {}",
        final_count
    );
}

#[test]
fn test_adv_runtime_tiered_timeout_contract() {
    assert_eq!(DEFAULT_TIMEOUT, Duration::from_secs(15), "DEFAULT_TIMEOUT must be 15s");
    assert_eq!(LONG_OP_TIMEOUT, Duration::from_secs(180), "LONG_OP_TIMEOUT must be 180s (3 minutes)");
    assert!(
        LONG_OP_TIMEOUT > DEFAULT_TIMEOUT,
        "LONG_OP_TIMEOUT must be significantly longer than DEFAULT_TIMEOUT"
    );
}

// ===========================================================================
// SECTION 4: Concurrency Stress Testing
// ===========================================================================

#[test]
fn test_adv_concurrency_stress_50_threads_serde_and_normalization() {
    const NUM_THREADS: usize = 50;
    const ITERATIONS_PER_THREAD: usize = 100;

    let barrier = Arc::new(Barrier::new(NUM_THREADS));
    let total_ops = Arc::new(AtomicU64::new(0));
    let mut handles = Vec::with_capacity(NUM_THREADS);

    for thread_idx in 0..NUM_THREADS {
        let barrier_clone = Arc::clone(&barrier);
        let ops_clone = Arc::clone(&total_ops);

        let handle = std::thread::spawn(move || {
            // Synchronize all threads so they launch simultaneously
            barrier_clone.wait();

            for iter in 0..ITERATIONS_PER_THREAD {
                // Scenario A: Empty payload
                let cfg_empty: StartConfig = serde_json::from_str("{}").expect("empty payload");
                assert_eq!(cfg_empty.profile, "");

                // Scenario B: Partial payload with dynamic profile
                let profile_name = format!("worker-thread-{}-iter-{}", thread_idx, iter);
                let json_b = format!(
                    r#"{{"profile": "{}", "cpus": {}, "memory": {}, "disk": {}}}"#,
                    profile_name,
                    (thread_idx % 8) + 1,
                    8_589_934_592u64,
                    107_374_182_400u64
                );
                let mut cfg_b: StartConfig = serde_json::from_str(&json_b).expect("valid json b");
                assert_eq!(cfg_b.profile, profile_name);
                assert_eq!(cfg_b.memory, 8_589_934_592);
                assert_eq!(cfg_b.disk, 107_374_182_400);

                cfg_b.normalize_resources();
                assert_eq!(cfg_b.memory, 8);
                assert_eq!(cfg_b.disk, 100);

                // Scenario C: Boundary byte value (ONE_GIB_BYTES - 1 vs ONE_GIB_BYTES)
                let json_c = format!(
                    r#"{{"memory": {}, "disk": {}}}"#,
                    ONE_GIB_BYTES - 1,
                    ONE_GIB_BYTES
                );
                let mut cfg_c: StartConfig = serde_json::from_str(&json_c).expect("valid json c");
                cfg_c.normalize_resources();
                assert_eq!(cfg_c.memory, 1_073_741_823);
                assert_eq!(cfg_c.disk, 1);

                // Scenario D: Malformed payload must fail safely without panic
                let json_d = r#"{"memory": -500, "profile": null}"#;
                let res_d: Result<StartConfig, _> = serde_json::from_str(json_d);
                assert!(res_d.is_err(), "Malformed JSON must return Err");

                ops_clone.fetch_add(4, Ordering::Relaxed);
            }
        });

        handles.push(handle);
    }

    for h in handles {
        h.join().expect("All 50 worker threads must finish without panic");
    }

    let final_ops = total_ops.load(Ordering::SeqCst);
    assert_eq!(
        final_ops,
        (NUM_THREADS * ITERATIONS_PER_THREAD * 4) as u64,
        "Expected exactly 20,000 distinct concurrent operations to complete successfully"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn test_adv_concurrency_stress_parallel_tokio_tasks() {
    const NUM_TASKS: usize = 50;
    let mut join_handles = Vec::with_capacity(NUM_TASKS);

    for task_id in 0..NUM_TASKS {
        let handle = tokio::spawn(async move {
            for i in 0..50 {
                let bytes_mem = (i as u64 + 1) * ONE_GIB_BYTES;
                let payload = serde_json::json!({
                    "profile": format!("tokio-task-{}-{}", task_id, i),
                    "memory": bytes_mem,
                    "disk": bytes_mem * 5,
                    "kubernetes": i % 2 == 0
                });

                let serialized = payload.to_string();
                let mut cfg: StartConfig = serde_json::from_str(&serialized).expect("Tokio task deserialization");
                cfg.normalize_resources();

                assert_eq!(cfg.memory, i as u64 + 1);
                assert_eq!(cfg.disk, (i as u64 + 1) * 5);
            }
        });
        join_handles.push(handle);
    }

    for handle in join_handles {
        handle.await.expect("Tokio task panicked");
    }
}
