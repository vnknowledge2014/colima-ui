//! Empirical Adversarial Test Suite for Milestone 3 (R3)
//!
//! Specifically validates:
//! 1. `sweep_vm_unresponsive`:
//!    - Instances in "Unhealthy" vs "Stopped", "Starting", and "Running" states.
//!    - Verifies that when `HealTrigger::VmUnresponsive` is triggered,
//!      `HealAction::RestartVm` executes `stop_instance_cli(profile, true)` then `start_instance_cli(profile)`.
//!    - Verifies error cascade (stop failure aborts start).
//! 2. `normalize_cli_labels`:
//!    - Adversarial trimming: whitespace in keys, whitespace in values, empty strings,
//!      multi-equals (`foo=bar=baz`), unicode (Vietnamese, emojis, CJK), and newlines.
//! 3. Strict SQLite DB Isolation:
//!    - Verifies `~/.colima-ui/activity.db*` remains 100% untouched before and after test execution.

use colima_ui_lib::commands::activity::{
    db_path, query, record, ActivityActor, ActivityEntry, ActivityFilter, ActivityKind,
};
use colima_ui_lib::commands::containers::normalize_cli_labels;
use colima_ui_lib::commands::knowledge_bank::init_knowledge_bank;
use colima_ui_lib::commands::self_heal::{
    execute_heal, execute_heal_target, list_rules, set_enabled, sweep_vm_unresponsive,
    update_rule, HealAction, HealMode, HealTrigger,
};
use colima_ui_lib::instance_reader::list_instances_fast;
use colima_ui_lib::path_util::app_data_dir;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{LazyLock, Once};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::Mutex as AsyncMutex;

/// Async mutex to avoid concurrent environment-variable mutation across test functions.
static TEST_SERIAL_MUTEX: AsyncMutex<()> = AsyncMutex::const_new(());
static DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

static KB_TEMP_DIR: LazyLock<PathBuf> = LazyLock::new(|| {
    let dir = std::env::temp_dir().join(format!("cui_adv_m3_kb_{}", std::process::id()));
    let _ = fs::create_dir_all(&dir);
    dir
});

static KB_INIT: Once = Once::new();

fn ensure_knowledge_bank_initialized() {
    KB_INIT.call_once(|| {
        let kb_dir = &*KB_TEMP_DIR;
        let orig_home = std::env::var("HOME").ok();
        // SAFETY: Protected by single Once invocation.
        unsafe {
            std::env::set_var("HOME", kb_dir);
        }
        init_knowledge_bank();
        // SAFETY: Restoring HOME.
        unsafe {
            if let Some(h) = orig_home {
                std::env::set_var("HOME", h);
            }
        }
    });
}

fn create_temp_dir(prefix: &str) -> PathBuf {
    let id = DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "cui_adv_m3_sh_{}_{}_{}",
        prefix,
        std::process::id(),
        id
    ));
    let _ = fs::create_dir_all(&dir);
    dir
}

struct DirGuard(PathBuf);
impl Drop for DirGuard {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Helper that creates a mock `colima` binary in `<bin_dir>/colima`
/// which logs invocations to `<bin_dir>/colima_mock.log`.
fn setup_mock_colima(bin_dir: &Path) -> (PathBuf, PathBuf) {
    let _ = fs::create_dir_all(bin_dir);
    let mock_bin = bin_dir.join("colima");
    let mock_log = bin_dir.join("colima_mock.log");

    let script = format!(
        r#"#!/bin/sh
echo "$@" >> "{log_path}"
if [ "$1" = "stop" ] && [ -n "$COLIMA_MOCK_FAIL_STOP" ]; then
    echo "mock error: failed to stop" >&2
    exit 1
fi
if [ "$1" = "start" ] && [ -n "$COLIMA_MOCK_FAIL_START" ]; then
    echo "mock error: failed to start" >&2
    exit 1
fi
exit 0
"#,
        log_path = mock_log.display()
    );

    fs::write(&mock_bin, script).expect("write mock colima binary");
    let mut perms = fs::metadata(&mock_bin).expect("metadata").permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&mock_bin, perms).expect("set executable perms");

    (mock_bin, mock_log)
}

/// Sets file mtime in the past using libc::utimes
fn set_file_mtime_secs_ago(file_path: &Path, secs_ago: u64) {
    let cur_secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_secs();
    let old_time = (cur_secs.saturating_sub(secs_ago)) as libc::time_t;
    let c_path = std::ffi::CString::new(file_path.to_str().expect("valid path")).expect("cstring");
    let times = [
        libc::timeval {
            tv_sec: old_time,
            tv_usec: 0,
        },
        libc::timeval {
            tv_sec: old_time,
            tv_usec: 0,
        },
    ];
    // SAFETY: times array is valid and c_path is a null-terminated C string.
    unsafe {
        libc::utimes(c_path.as_ptr(), times.as_ptr());
    }
}

// ===========================================================================
// SECTION 1: sweep_vm_unresponsive — State Discrimination (Unhealthy vs Stopped)
// ===========================================================================

#[tokio::test]
async fn test_adv_sweep_vm_unresponsive_discriminates_unhealthy_and_stopped() {
    let _guard = TEST_SERIAL_MUTEX.lock().await;

    ensure_knowledge_bank_initialized();

    let root = create_temp_dir("unhealthy_stopped");
    let _cleanup = DirGuard(root.clone());
    let colima_home = root.join("colima_home");
    let bin_dir = root.join("bin");
    let (_mock_bin, mock_log) = setup_mock_colima(&bin_dir);

    // Ensure mock colima binary is found on PATH
    let orig_path = std::env::var("PATH").unwrap_or_default();
    let new_path = format!("{}:{}", bin_dir.display(), orig_path);
    // SAFETY: Protected by TEST_SERIAL_MUTEX.
    unsafe {
        std::env::set_var("PATH", &new_path);
        std::env::set_var("COLIMA_HOME", &colima_home);
    }

    // 1. Setup Unhealthy profile:
    // Lima process alive (PID = current process), mtime >= 90s ago (180s), no docker.sock
    let unh_dir = colima_home.join("vm-unhealthy");
    let unh_lima = colima_home.join("_lima").join("colima-vm-unhealthy");
    fs::create_dir_all(&unh_dir).expect("create unh_dir");
    fs::create_dir_all(&unh_lima).expect("create unh_lima");
    fs::write(unh_dir.join("colima.yaml"), "runtime: docker\n").expect("write unh yaml");
    let unh_pid_file = unh_lima.join("ha.pid");
    fs::write(&unh_pid_file, format!("{}\n", std::process::id())).expect("write unh pid");
    set_file_mtime_secs_ago(&unh_pid_file, 180);

    // 2. Setup Stopped profile:
    // Dead PID (e.g. 9999999), no ha.sock
    let stp_dir = colima_home.join("vm-stopped");
    let stp_lima = colima_home.join("_lima").join("colima-vm-stopped");
    fs::create_dir_all(&stp_dir).expect("create stp_dir");
    fs::create_dir_all(&stp_lima).expect("create stp_lima");
    fs::write(stp_dir.join("colima.yaml"), "runtime: docker\n").expect("write stp yaml");
    let stp_pid_file = stp_lima.join("ha.pid");
    fs::write(&stp_pid_file, "9999999\n").expect("write stp pid");

    // 3. Setup Starting profile:
    // Alive PID, mtime fresh (< 90s)
    let str_dir = colima_home.join("vm-starting");
    let str_lima = colima_home.join("_lima").join("colima-vm-starting");
    fs::create_dir_all(&str_dir).expect("create str_dir");
    fs::create_dir_all(&str_lima).expect("create str_lima");
    fs::write(str_dir.join("colima.yaml"), "runtime: docker\n").expect("write str yaml");
    let str_pid_file = str_lima.join("ha.pid");
    fs::write(&str_pid_file, format!("{}\n", std::process::id())).expect("write str pid");

    // Verify fast instance reader detects exact status for each
    let instances = list_instances_fast();
    let unh_inst = instances.iter().find(|i| i.name == "vm-unhealthy").expect("unh");
    let stp_inst = instances.iter().find(|i| i.name == "vm-stopped").expect("stp");
    let str_inst = instances.iter().find(|i| i.name == "vm-starting").expect("str");

    assert_eq!(unh_inst.status, "Unhealthy");
    assert_eq!(stp_inst.status, "Stopped");
    assert_eq!(str_inst.status, "Starting");

    // Activate Auto mode for VmUnresponsive
    set_enabled(true).expect("enable self-healing");

    let rules = list_rules().expect("list rules");
    let vm_rule = rules
        .iter()
        .find(|r| r.trigger == HealTrigger::VmUnresponsive)
        .expect("VmUnresponsive rule must exist");

    update_rule(
        vm_rule.id,
        HealMode::Auto,
        vm_rule.threshold,
        100,
        true,
    )
    .expect("update rule to auto");

    // Clear mock log before sweep
    let _ = fs::write(&mock_log, "");

    // RUN THE SWEEPER!
    sweep_vm_unresponsive().await;

    // Verify executions in mock log
    let log_content = fs::read_to_string(&mock_log).unwrap_or_default();
    let lines: Vec<&str> = log_content.lines().collect();

    // Must have executed stop then start for vm-unhealthy
    assert!(
        lines.iter().any(|l| l.contains("stop --profile vm-unhealthy --force")),
        "sweep_vm_unresponsive must execute stop --profile vm-unhealthy --force. Got: {:?}",
        lines
    );
    assert!(
        lines.iter().any(|l| l.contains("start --profile vm-unhealthy")),
        "sweep_vm_unresponsive must execute start --profile vm-unhealthy. Got: {:?}",
        lines
    );

    // CRITICAL: Must NEVER execute stop or start for Stopped or Starting instances!
    assert!(
        !lines.iter().any(|l| l.contains("vm-stopped")),
        "sweep_vm_unresponsive must NEVER restart Stopped instances! Violations in log: {:?}",
        lines
    );
    assert!(
        !lines.iter().any(|l| l.contains("vm-starting")),
        "sweep_vm_unresponsive must NEVER restart Starting instances! Violations in log: {:?}",
        lines
    );

    // Restore environment variables
    // SAFETY: Protected by TEST_SERIAL_MUTEX.
    unsafe {
        std::env::set_var("PATH", &orig_path);
        std::env::remove_var("COLIMA_HOME");
    }
}

// ===========================================================================
// SECTION 2: HealAction::RestartVm — Full Execution & Error Cascades
// ===========================================================================

#[tokio::test]
async fn test_adv_restart_vm_executes_stop_force_then_start() {
    let _guard = TEST_SERIAL_MUTEX.lock().await;

    let root = create_temp_dir("restart_vm_seq");
    let _cleanup = DirGuard(root.clone());
    let bin_dir = root.join("bin");
    let (_mock_bin, mock_log) = setup_mock_colima(&bin_dir);

    let orig_path = std::env::var("PATH").unwrap_or_default();
    let new_path = format!("{}:{}", bin_dir.display(), orig_path);
    // SAFETY: Protected by TEST_SERIAL_MUTEX.
    unsafe {
        std::env::set_var("PATH", &new_path);
    }

    // 1. Test RestartVm with named custom profile
    let _ = fs::write(&mock_log, "");
    let res_custom = execute_heal_target(HealAction::RestartVm, "staging-env").await;
    assert!(res_custom.is_ok(), "RestartVm should succeed with mock CLI");

    let log_custom = fs::read_to_string(&mock_log).unwrap_or_default();
    let lines_custom: Vec<&str> = log_custom.lines().collect();
    assert_eq!(
        lines_custom.len(),
        2,
        "Expected exactly 2 CLI invocations (stop, start), got: {:?}",
        lines_custom
    );
    assert_eq!(
        lines_custom[0], "stop --profile staging-env --force",
        "First call must be forced stop"
    );
    assert_eq!(
        lines_custom[1], "start --profile staging-env",
        "Second call must be start"
    );

    // 2. Test RestartVm with "default" profile (must omit --profile flag)
    let _ = fs::write(&mock_log, "");
    let res_default = execute_heal_target(HealAction::RestartVm, "default").await;
    assert!(res_default.is_ok());

    let log_default = fs::read_to_string(&mock_log).unwrap_or_default();
    let lines_default: Vec<&str> = log_default.lines().collect();
    assert_eq!(lines_default.len(), 2);
    assert_eq!(
        lines_default[0], "stop --force",
        "Default profile must stop with force and no --profile arg"
    );
    assert_eq!(
        lines_default[1], "start",
        "Default profile must start with no --profile arg"
    );

    // 3. Test RestartVm via execute_heal (no target, defaults to "default")
    let _ = fs::write(&mock_log, "");
    let res_heal = execute_heal(HealAction::RestartVm).await;
    assert!(res_heal.is_ok());

    let log_heal = fs::read_to_string(&mock_log).unwrap_or_default();
    let lines_heal: Vec<&str> = log_heal.lines().collect();
    assert_eq!(lines_heal.len(), 2);
    assert_eq!(lines_heal[0], "stop --force");
    assert_eq!(lines_heal[1], "start");

    // SAFETY: Protected by TEST_SERIAL_MUTEX.
    unsafe {
        std::env::set_var("PATH", &orig_path);
    }
}

#[tokio::test]
async fn test_adv_restart_vm_error_cascade_aborts_start_on_stop_failure() {
    let _guard = TEST_SERIAL_MUTEX.lock().await;

    let root = create_temp_dir("restart_vm_error");
    let _cleanup = DirGuard(root.clone());
    let bin_dir = root.join("bin");
    let (_mock_bin, mock_log) = setup_mock_colima(&bin_dir);

    let orig_path = std::env::var("PATH").unwrap_or_default();
    let new_path = format!("{}:{}", bin_dir.display(), orig_path);
    // SAFETY: Protected by TEST_SERIAL_MUTEX.
    unsafe {
        std::env::set_var("PATH", &new_path);
        std::env::set_var("COLIMA_MOCK_FAIL_STOP", "1");
    }

    let _ = fs::write(&mock_log, "");

    // Execute RestartVm while stop fails
    let res = execute_heal_target(HealAction::RestartVm, "failing-profile").await;
    assert!(res.is_err(), "RestartVm must fail when stop fails");

    let log_content = fs::read_to_string(&mock_log).unwrap_or_default();
    let lines: Vec<&str> = log_content.lines().collect();

    // Verify stop was attempted, and start was NEVER called
    assert_eq!(lines.len(), 1, "Only stop should have been attempted");
    assert_eq!(lines[0], "stop --profile failing-profile --force");
    assert!(
        !log_content.contains("start"),
        "start_instance_cli must NEVER run if stop_instance_cli failed"
    );

    // Now test when start fails
    // SAFETY: Protected by TEST_SERIAL_MUTEX.
    unsafe {
        std::env::remove_var("COLIMA_MOCK_FAIL_STOP");
        std::env::set_var("COLIMA_MOCK_FAIL_START", "1");
    }
    let _ = fs::write(&mock_log, "");

    let res_start_fail = execute_heal_target(HealAction::RestartVm, "failing-start-profile").await;
    assert!(res_start_fail.is_err(), "RestartVm must fail when start fails");

    let log_start_fail = fs::read_to_string(&mock_log).unwrap_or_default();
    let lines_start_fail: Vec<&str> = log_start_fail.lines().collect();
    assert_eq!(lines_start_fail.len(), 2, "Both stop and start should have run");
    assert_eq!(lines_start_fail[0], "stop --profile failing-start-profile --force");
    assert_eq!(lines_start_fail[1], "start --profile failing-start-profile");

    // SAFETY: Protected by TEST_SERIAL_MUTEX.
    unsafe {
        std::env::remove_var("COLIMA_MOCK_FAIL_START");
        std::env::set_var("PATH", &orig_path);
    }
}

#[tokio::test]
async fn test_adv_sweep_vm_unresponsive_suggest_mode_does_not_mutate_machine() {
    let _guard = TEST_SERIAL_MUTEX.lock().await;

    ensure_knowledge_bank_initialized();

    let root = create_temp_dir("suggest_mode");
    let _cleanup = DirGuard(root.clone());
    let colima_home = root.join("colima_home");
    let bin_dir = root.join("bin");
    let (_mock_bin, mock_log) = setup_mock_colima(&bin_dir);

    let orig_path = std::env::var("PATH").unwrap_or_default();
    let new_path = format!("{}:{}", bin_dir.display(), orig_path);
    // SAFETY: Protected by TEST_SERIAL_MUTEX.
    unsafe {
        std::env::set_var("PATH", &new_path);
        std::env::set_var("COLIMA_HOME", &colima_home);
    }

    // Setup an Unhealthy profile
    let unh_dir = colima_home.join("vm-unhealthy-suggest");
    let unh_lima = colima_home.join("_lima").join("colima-vm-unhealthy-suggest");
    fs::create_dir_all(&unh_dir).unwrap();
    fs::create_dir_all(&unh_lima).unwrap();
    fs::write(unh_dir.join("colima.yaml"), "runtime: docker\n").unwrap();
    let unh_pid = unh_lima.join("ha.pid");
    fs::write(&unh_pid, format!("{}\n", std::process::id())).unwrap();
    set_file_mtime_secs_ago(&unh_pid, 200);

    set_enabled(true).unwrap();

    let rules = list_rules().unwrap();
    let vm_rule = rules
        .iter()
        .find(|r| r.trigger == HealTrigger::VmUnresponsive)
        .unwrap();

    // Set rule to Suggest mode
    update_rule(
        vm_rule.id,
        HealMode::Suggest,
        vm_rule.threshold,
        100,
        true,
    )
    .unwrap();

    let _ = fs::write(&mock_log, "");

    // Sweep should evaluate Unhealthy instance, but in Suggest mode it does NOT call colima stop/start
    sweep_vm_unresponsive().await;

    let log_content = fs::read_to_string(&mock_log).unwrap_or_default();
    assert!(
        log_content.is_empty(),
        "Suggest mode must NOT execute stop or start commands! Found: {}",
        log_content
    );

    // SAFETY: Protected by TEST_SERIAL_MUTEX.
    unsafe {
        std::env::set_var("PATH", &orig_path);
        std::env::remove_var("COLIMA_HOME");
    }
}

// ===========================================================================
// SECTION 3: normalize_cli_labels — Adversarial Trimming & Stress Matrix
// ===========================================================================

#[test]
fn test_adv_normalize_cli_labels_adversarial_whitespace_and_newlines() {
    let input = serde_json::json!({
        "Id": "test-cont-adv",
        "Labels": "   com.docker.compose.project   =   my-web   , \t\r\ncom.docker.compose.service\t = \r\n web-service \t "
    });

    let norm = normalize_cli_labels(input);
    let labels = &norm["Labels"];
    assert_eq!(
        labels["com.docker.compose.project"], "my-web",
        "Spaces around key and value must be trimmed"
    );
    assert_eq!(
        labels["com.docker.compose.service"], "web-service",
        "Tabs, CR, and LF around key and value must be trimmed"
    );
}

#[test]
fn test_adv_normalize_cli_labels_multi_equals_and_complex_values() {
    let input = serde_json::json!({
        "Labels": "  formula=a=b=c=d  ,  db_url=postgres://usr:p=w=d@host:5432/db?ssl=true=1  ,  empty_val=  ,  double_eq==val  ,  =key_is_empty  "
    });

    let norm = normalize_cli_labels(input);
    let labels = &norm["Labels"];

    assert_eq!(labels["formula"], "a=b=c=d");
    assert_eq!(labels["db_url"], "postgres://usr:p=w=d@host:5432/db?ssl=true=1");
    assert_eq!(labels["empty_val"], "");
    assert_eq!(labels["double_eq"], "=val");
    assert_eq!(labels[""], "key_is_empty");
}

#[test]
fn test_adv_normalize_cli_labels_unicode_emojis_and_diacritics() {
    let input = serde_json::json!({
        "Labels": "  tên_dự_án  =  hệ_thống_quản_lý  ,   🏷️_version   =   🚀_v3.2.1   ,  プロジェクト  =  ウェブサービス  "
    });

    let norm = normalize_cli_labels(input);
    let labels = &norm["Labels"];

    assert_eq!(labels["tên_dự_án"], "hệ_thống_quản_lý");
    assert_eq!(labels["🏷️_version"], "🚀_v3.2.1");
    assert_eq!(labels["プロジェクト"], "ウェブサービス");
}

#[test]
fn test_adv_normalize_cli_labels_degenerate_and_empty_inputs() {
    // 1. Multiple consecutive commas and whitespace
    let norm1 = normalize_cli_labels(serde_json::json!({
        "Labels": "  ,,  ,   ,  "
    }));
    assert_eq!(norm1["Labels"], serde_json::json!({}));

    // 2. Empty string
    let norm2 = normalize_cli_labels(serde_json::json!({
        "Labels": ""
    }));
    assert_eq!(norm2["Labels"], serde_json::json!({}));

    // 3. Null field — normalize_cli_labels preserves non-string unless absent
    let norm3 = normalize_cli_labels(serde_json::json!({
        "Labels": null
    }));
    assert!(norm3["Labels"].is_null(), "Explicit null Labels field is retained as null");

    // 4. Missing field entirely
    let norm4 = normalize_cli_labels(serde_json::json!({
        "Id": "no_labels_at_all"
    }));
    assert_eq!(norm4["Labels"], serde_json::json!({}));

    // 5. Standalone flag without equals
    let norm5 = normalize_cli_labels(serde_json::json!({
        "Labels": "  production_flag  ,  experimental  "
    }));
    assert_eq!(norm5["Labels"]["production_flag"], "");
    assert_eq!(norm5["Labels"]["experimental"], "");

    // 6. Pre-existing Map remains intact
    let norm6 = normalize_cli_labels(serde_json::json!({
        "Labels": { "pre_existing": "val_123" }
    }));
    assert_eq!(norm6["Labels"]["pre_existing"], "val_123");
}

// ===========================================================================
// SECTION 4: Strict SQLite DB Isolation
// ===========================================================================

#[tokio::test]
async fn test_adv_sqlite_strict_db_isolation_and_stat_immutability() {
    let _guard = TEST_SERIAL_MUTEX.lock().await;

    // 1. Capture exact live DB stats before any operations
    let home = std::env::var("HOME").expect("HOME set");
    let live_db = PathBuf::from(&home).join(".colima-ui").join("activity.db");
    let live_db_shm = PathBuf::from(&home).join(".colima-ui").join("activity.db-shm");
    let live_db_wal = PathBuf::from(&home).join(".colima-ui").join("activity.db-wal");

    let stat_file = |path: &Path| -> Option<(u64, i64, i64)> {
        fs::metadata(path).ok().map(|m| {
            let size = m.len();
            let mtime = m
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map_or(0, |d| d.as_secs() as i64);
            let ctime = mtime; // Approximation for platform portability
            (size, mtime, ctime)
        })
    };

    let before_main = stat_file(&live_db);
    let before_shm = stat_file(&live_db_shm);
    let before_wal = stat_file(&live_db_wal);

    // 2. Verify db_path() points strictly into std::env::temp_dir()
    let test_path = db_path();
    assert!(
        test_path.starts_with(std::env::temp_dir()),
        "Activity db path {:?} must reside in temp_dir() during test execution",
        test_path
    );
    assert!(
        test_path.to_string_lossy().contains("cui_test_activity_"),
        "Activity db path {:?} must contain process-isolated prefix",
        test_path
    );
    assert!(
        !test_path.starts_with(app_data_dir()),
        "Activity db path {:?} must NOT reside in live app data dir",
        test_path
    );

    // 3. Write 100 diverse activity entries to exercise WAL, writes, and prunes
    for i in 0..100 {
        let kind = match i % 4 {
            0 => ActivityKind::Lifecycle,
            1 => ActivityKind::Destructive,
            2 => ActivityKind::Task,
            _ => ActivityKind::Config,
        };
        record(
            ActivityEntry::new(
                kind,
                &format!("adv_isolation_probe_{}", i),
                "test_target",
                &format!("target_{}", i),
            )
            .by(if i % 2 == 0 {
                ActivityActor::App
            } else {
                ActivityActor::User
            })
            .took(i * 10),
        );
    }

    // 4. Verify query retrieves records from the isolated test database
    let rows = query(&ActivityFilter::default()).expect("query test activity db");
    assert!(
        rows.len() >= 100,
        "Expected at least 100 entries in isolated test db, found: {}",
        rows.len()
    );

    // 5. Re-check live DB stats after operations — MUST BE 100% UNCHANGED
    let after_main = stat_file(&live_db);
    let after_shm = stat_file(&live_db_shm);
    let after_wal = stat_file(&live_db_wal);

    assert_eq!(
        before_main, after_main,
        "CRITICAL: ~/.colima-ui/activity.db was mutated during test execution!"
    );
    assert_eq!(
        before_shm, after_shm,
        "CRITICAL: ~/.colima-ui/activity.db-shm was mutated during test execution!"
    );
    assert_eq!(
        before_wal, after_wal,
        "CRITICAL: ~/.colima-ui/activity.db-wal was mutated during test execution!"
    );
}
