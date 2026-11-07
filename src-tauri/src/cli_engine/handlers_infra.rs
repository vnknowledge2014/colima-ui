//! Dual-transport (`HttpOrDirect`) command handlers for infrastructure domains:
//! `vm`, `config`, `presets`, `container`, `image`, `volume`, `network`,
//! `compose`, `k8s`, `kind`, `lima`, `system`, and `status`.

use std::fs;
use std::path::Path;
use std::time::Duration;

use serde_json::{json, Value};

use crate::commands::{
    colima, colima_config, compose, compose_autofix, compose_autofix_apply, compose_diagnose,
    containers, k8s_cluster, k8s_resources, kind, knowledge_bank, kubernetes, lima, networks,
    runtime, system, volumes,
};
use crate::services::container::ContainerService;

use super::args::{
    ComposeCommand, ConfigCommand, ContainerCommand, ImageCommand, K8sCommand, KindCommand,
    LimaCommand, NetworkCommand, PortForwardAction, PresetsCommand, SystemCommand, VmCommand,
    VolumeCommand,
};
use super::client::{DesktopClient, HttpOutcome};
use super::ensure_kb_init;
use super::output::{format_bytes, print_human_value, print_json_ok, truncate};

// ============================================================================
// Shared Helpers (also reused by `mcp.rs`)
// ============================================================================

/// Convert `ConfigChanges` into camelCase JSON expected by `/api/instances/config/*`.
pub(crate) fn config_changes_to_json(c: &colima_config::ConfigChanges) -> Value {
    let mut map = serde_json::Map::new();
    if let Some(v) = c.cpu {
        map.insert("cpu".into(), json!(v));
    }
    if let Some(v) = c.memory {
        map.insert("memory".into(), json!(v));
    }
    if let Some(v) = c.disk {
        map.insert("disk".into(), json!(v));
    }
    if let Some(ref v) = c.runtime {
        map.insert("runtime".into(), json!(v));
    }
    if let Some(ref v) = c.vm_type {
        map.insert("vmType".into(), json!(v));
    }
    if let Some(ref v) = c.mount_type {
        map.insert("mountType".into(), json!(v));
    }
    if let Some(ref v) = c.dns {
        map.insert("dns".into(), json!(v));
    }
    if let Some(v) = c.network_address {
        map.insert("networkAddress".into(), json!(v));
    }
    if let Some(v) = c.kubernetes {
        map.insert("kubernetes".into(), json!(v));
    }
    Value::Object(map)
}

/// Read inline string or `@file` content.
pub(crate) fn read_inline_or_file(raw_or_at_file: &str) -> Result<String, String> {
    let trimmed = raw_or_at_file.trim();
    if let Some(file_path) = trimmed.strip_prefix('@') {
        let p = file_path.trim();
        if p.is_empty() {
            return Err("Empty file path after '@'".to_string());
        }
        fs::read_to_string(p).map_err(|e| format!("Failed to read '{p}': {e}"))
    } else {
        Ok(raw_or_at_file.to_string())
    }
}

/// Parse inline YAML/JSON or `@file` into `colima_config::ConfigChanges`, supporting
/// both flat UI keys (`cpu`, `vmType`, `kubernetes: true`) and nested `colima.yaml` keys
/// (`network.dns`, `network.address`, `kubernetes.enabled`).
pub(crate) fn parse_yaml_to_config_changes(
    raw_or_at_file: &str,
) -> Result<colima_config::ConfigChanges, String> {
    let content = read_inline_or_file(raw_or_at_file)?;
    if content.trim().is_empty() {
        return Err("Configuration YAML must not be empty".to_string());
    }

    let doc: Value = serde_yml::from_str(&content)
        .map_err(|e| format!("Invalid YAML/JSON configuration: {e}"))?;
    let Some(obj) = doc.as_object() else {
        return Err("Configuration YAML must be a key-value mapping".to_string());
    };

    let cpu = obj
        .get("cpu")
        .or_else(|| obj.get("cpus"))
        .and_then(Value::as_u64);
    let memory = obj.get("memory").and_then(Value::as_u64);
    let disk = obj.get("disk").and_then(Value::as_u64);
    let runtime = obj
        .get("runtime")
        .and_then(Value::as_str)
        .map(str::to_string);
    let vm_type = obj
        .get("vmType")
        .or_else(|| obj.get("vm_type"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let mount_type = obj
        .get("mountType")
        .or_else(|| obj.get("mount_type"))
        .and_then(Value::as_str)
        .map(str::to_string);

    let dns_val = obj.get("dns").or_else(|| doc.pointer("/network/dns"));
    let dns = dns_val.and_then(Value::as_array).map(|arr| {
        arr.iter()
            .filter_map(|item| item.as_str().map(str::to_string))
            .collect::<Vec<_>>()
    });

    let network_address = obj
        .get("networkAddress")
        .or_else(|| obj.get("network_address"))
        .or_else(|| doc.pointer("/network/address"))
        .and_then(Value::as_bool);

    let kubernetes = obj
        .get("kubernetes")
        .and_then(Value::as_bool)
        .or_else(|| doc.pointer("/kubernetes/enabled").and_then(Value::as_bool));

    Ok(colima_config::ConfigChanges {
        cpu,
        memory,
        disk,
        runtime,
        vm_type,
        mount_type,
        dns,
        network_address,
        kubernetes,
    })
}

/// Resolve a Compose directory to its manifest file (`compose.yaml`, `docker-compose.yml`, etc.) if needed.
pub(crate) fn resolve_compose_file_path(file_or_dir: &str) -> String {
    let p = Path::new(file_or_dir);
    if p.is_dir() {
        for candidate in [
            "compose.yaml",
            "compose.yml",
            "docker-compose.yaml",
            "docker-compose.yml",
        ] {
            let joined = p.join(candidate);
            if joined.is_file() {
                return joined.to_string_lossy().to_string();
            }
        }
    }
    file_or_dir.to_string()
}

/// Build a `colima::StartConfig` preserving `colima.yaml` unless explicit CLI overrides were given.
#[allow(clippy::too_many_arguments)]
pub(crate) fn build_vm_start_config(
    profile: &str,
    cpus: Option<u32>,
    memory: Option<u32>,
    disk: Option<u32>,
    runtime_opt: Option<String>,
    arch: Option<String>,
    vm_type: Option<String>,
    k8s: bool,
) -> colima::StartConfig {
    let norm_profile = if profile.trim().is_empty() {
        "default".to_string()
    } else {
        profile.trim().to_string()
    };
    let yaml_exists = crate::instance_reader::colima_home()
        .join(&norm_profile)
        .join("colima.yaml")
        .exists();

    let default_cpus = if yaml_exists { 0 } else { 2 };
    let default_mem = if yaml_exists { 0 } else { 2 };
    let default_disk = if yaml_exists { 0 } else { 60 };
    let default_runtime = if yaml_exists {
        String::new()
    } else {
        "docker".to_string()
    };

    colima::StartConfig {
        profile: norm_profile,
        runtime: runtime_opt.unwrap_or(default_runtime),
        cpus: cpus.unwrap_or(default_cpus),
        memory: u64::from(memory.unwrap_or(default_mem)),
        disk: u64::from(disk.unwrap_or(default_disk)),
        vm_type: vm_type.unwrap_or_default(),
        kubernetes: k8s,
        kubernetes_version: String::new(),
        arch: arch.unwrap_or_default(),
        mount_type: String::new(),
        mounts: Vec::new(),
        dns: Vec::new(),
        network_address: false,
    }
}

fn try_parse_json_str(raw: Value) -> Value {
    if let Value::String(ref s) = raw {
        let trimmed = s.trim();
        if trimmed.starts_with('{') || trimmed.starts_with('[') {
            if let Ok(parsed) = serde_json::from_str::<Value>(trimmed) {
                return parsed;
            }
        }
    }
    raw
}

// ============================================================================
// 1. VM Handler
// ============================================================================

pub async fn handle_vm(
    action: VmCommand,
    client: Option<&DesktopClient>,
    use_json: bool,
) -> Result<(), String> {
    match action {
        VmCommand::List => {
            let val = if let Some(c) = client {
                match c.get("/api/instances", &[], Duration::from_secs(10)).await {
                    HttpOutcome::Ok(v) => v,
                    HttpOutcome::ApiErr(e) => return Err(e),
                    HttpOutcome::Unavailable(_) => {
                        let list = crate::instance_reader::list_instances_fast();
                        serde_json::to_value(list).map_err(|e| e.to_string())?
                    }
                }
            } else {
                let list = crate::instance_reader::list_instances_fast();
                serde_json::to_value(list).map_err(|e| e.to_string())?
            };

            if use_json {
                print_json_ok(&val);
            } else if let Some(arr) = val.as_array() {
                println!(
                    "{:<20} {:<12} {:<8} {:<6} {:<10} {:<10} {:<10} ADDRESS",
                    "PROFILE", "STATUS", "ARCH", "CPUS", "MEMORY", "DISK", "RUNTIME"
                );
                for item in arr {
                    let name = item.get("name").and_then(Value::as_str).unwrap_or("-");
                    let status = item.get("status").and_then(Value::as_str).unwrap_or("-");
                    let arch = item.get("arch").and_then(Value::as_str).unwrap_or("-");
                    let cpus = item.get("cpus").and_then(Value::as_u64).unwrap_or(0);
                    let mem = item.get("memory").and_then(Value::as_u64).unwrap_or(0);
                    let disk = item.get("disk").and_then(Value::as_u64).unwrap_or(0);
                    let rt = item.get("runtime").and_then(Value::as_str).unwrap_or("-");
                    let addr = item.get("address").and_then(Value::as_str).unwrap_or("");
                    println!(
                        "{:<20} {:<12} {:<8} {:<6} {:<10} {:<10} {:<10} {}",
                        truncate(name, 19),
                        status,
                        arch,
                        cpus,
                        format_bytes(mem),
                        format_bytes(disk),
                        rt,
                        addr
                    );
                }
            } else {
                print_human_value(&val);
            }
            Ok(())
        }
        VmCommand::Status { profile } => {
            let val = if let Some(c) = client {
                match c
                    .get(
                        "/api/instances/status",
                        &[("profile", profile.as_str())],
                        Duration::from_secs(12),
                    )
                    .await
                {
                    HttpOutcome::Ok(v) => v,
                    HttpOutcome::ApiErr(e) => return Err(e),
                    HttpOutcome::Unavailable(_) => {
                        let st = colima::instance_status(profile)
                            .await
                            .map_err(|e| e.to_string())?;
                        serde_json::to_value(st).map_err(|e| e.to_string())?
                    }
                }
            } else {
                let st = colima::instance_status(profile)
                    .await
                    .map_err(|e| e.to_string())?;
                serde_json::to_value(st).map_err(|e| e.to_string())?
            };

            if use_json {
                print_json_ok(&val);
            } else {
                println!(
                    "Profile: {}",
                    val.get("profile").and_then(Value::as_str).unwrap_or("-")
                );
                println!(
                    "Status:  {}",
                    val.get("status").and_then(Value::as_str).unwrap_or("-")
                );
                println!(
                    "Arch:    {}",
                    val.get("arch").and_then(Value::as_str).unwrap_or("-")
                );
                println!(
                    "Runtime: {}",
                    val.get("runtime").and_then(Value::as_str).unwrap_or("-")
                );
                for (label, key) in [
                    ("CPU:    ", "cpu_usage"),
                    ("Memory: ", "memory_usage"),
                    ("Disk:   ", "disk_usage"),
                    ("Address:", "address"),
                ] {
                    if let Some(s) = val
                        .get(key)
                        .and_then(Value::as_str)
                        .filter(|s| !s.is_empty())
                    {
                        println!("{label} {s}");
                    }
                }
            }
            Ok(())
        }
        VmCommand::Start {
            profile,
            cpus,
            memory,
            disk,
            runtime,
            arch,
            vm_type,
            k8s,
        } => {
            let cfg = build_vm_start_config(&profile, cpus, memory, disk, runtime, arch, vm_type, k8s);
            let msg = if let Some(c) = client {
                let payload = serde_json::to_value(&cfg).map_err(|e| e.to_string())?;
                match c
                    .post_json("/api/instances/start", &[], &payload, Duration::from_secs(240))
                    .await
                {
                    HttpOutcome::Ok(v) => v.as_str().unwrap_or("Started").to_string(),
                    HttpOutcome::ApiErr(e) => return Err(e),
                    HttpOutcome::Unavailable(_) => colima::start_instance(cfg)
                        .await
                        .map_err(|e| e.to_string())?,
                }
            } else {
                colima::start_instance(cfg).await.map_err(|e| e.to_string())?
            };
            emit_message(&msg, use_json);
            Ok(())
        }
        VmCommand::Stop { profile, force } => {
            let force_str = if force { "true" } else { "false" };
            let msg = if let Some(c) = client {
                match c
                    .post_empty(
                        "/api/instances/stop",
                        &[("profile", profile.as_str()), ("force", force_str)],
                        Duration::from_secs(90),
                    )
                    .await
                {
                    HttpOutcome::Ok(v) => v.as_str().unwrap_or("Stopped").to_string(),
                    HttpOutcome::ApiErr(e) => return Err(e),
                    HttpOutcome::Unavailable(_) => colima::stop_instance_cli(profile, force)
                        .await
                        .map_err(|e| e.to_string())?,
                }
            } else {
                colima::stop_instance_cli(profile, force)
                    .await
                    .map_err(|e| e.to_string())?
            };
            emit_message(&msg, use_json);
            Ok(())
        }
        VmCommand::Restart { profile } => {
            if let Some(c) = client {
                let _ = c
                    .post_empty(
                        "/api/instances/stop",
                        &[("profile", profile.as_str()), ("force", "false")],
                        Duration::from_secs(90),
                    )
                    .await;
                let cfg = build_vm_start_config(&profile, None, None, None, None, None, None, false);
                let payload = serde_json::to_value(&cfg).map_err(|e| e.to_string())?;
                match c
                    .post_json("/api/instances/start", &[], &payload, Duration::from_secs(240))
                    .await
                {
                    HttpOutcome::Ok(_) => {
                        emit_message(&format!("Instance '{profile}' restarted"), use_json);
                        return Ok(());
                    }
                    HttpOutcome::ApiErr(e) => return Err(e),
                    HttpOutcome::Unavailable(_) => {}
                }
            }
            let _ = colima::stop_instance_cli(profile.clone(), false).await;
            let cfg = build_vm_start_config(&profile, None, None, None, None, None, None, false);
            colima::start_instance(cfg).await.map_err(|e| e.to_string())?;
            emit_message(&format!("Instance '{profile}' restarted"), use_json);
            Ok(())
        }
        VmCommand::Delete { profile, force } => {
            let force_str = if force { "true" } else { "false" };
            let msg = if let Some(c) = client {
                match c
                    .post_empty(
                        "/api/instances/delete",
                        &[("profile", profile.as_str()), ("force", force_str)],
                        Duration::from_secs(90),
                    )
                    .await
                {
                    HttpOutcome::Ok(v) => v.as_str().unwrap_or("Deleted").to_string(),
                    HttpOutcome::ApiErr(e) => return Err(e),
                    HttpOutcome::Unavailable(_) => colima::delete_instance_cli(profile, force)
                        .await
                        .map_err(|e| e.to_string())?,
                }
            } else {
                colima::delete_instance_cli(profile, force)
                    .await
                    .map_err(|e| e.to_string())?
            };
            emit_message(&msg, use_json);
            Ok(())
        }
        VmCommand::Ssh { profile, cmd } => {
            crate::validation::ensure_valid_profile(&profile)?;
            if cmd.is_empty() {
                let args = colima::get_ssh_command(profile)
                    .await
                    .map_err(|e| e.to_string())?;
                let full_cmd = format!("colima {}", args.join(" "));
                if use_json {
                    print_json_ok(&json!({ "profile": args, "command": full_cmd }));
                } else {
                    println!("{full_cmd}");
                }
                return Ok(());
            }

            let out = crate::helpers::run_blocking(move || {
                let mut argv = vec!["ssh".to_string()];
                if profile != "default" && !profile.is_empty() {
                    argv.push("--profile".to_string());
                    argv.push(profile);
                }
                argv.push("--".to_string());
                argv.extend(cmd);

                let mut proc = std::process::Command::new(crate::path_util::resolve_binary("colima"));
                crate::path_util::apply_path_to_cmd(&mut proc);
                let output = proc
                    .args(&argv)
                    .output()
                    .map_err(|e| format!("Failed to execute colima ssh: {e}"))?;
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                if output.status.success() {
                    Ok((stdout, stderr))
                } else {
                    Err(if stderr.trim().is_empty() {
                        format!("colima ssh exited with {}", output.status)
                    } else {
                        stderr.trim().to_string()
                    })
                }
            })
            .await?;

            if use_json {
                print_json_ok(&json!({ "stdout": out.0, "stderr": out.1 }));
            } else if !out.0.is_empty() {
                print!("{}", out.0);
                if !out.0.ends_with('\n') {
                    println!();
                }
            } else if !out.1.is_empty() {
                print!("{}", out.1);
            }
            Ok(())
        }
        VmCommand::CreateWorker {
            master_profile,
            worker_profile,
            cpus,
            memory,
            disk,
        } => {
            let body = json!({
                "masterProfile": master_profile,
                "workerProfile": worker_profile,
                "cpus": cpus,
                "memory": memory,
                "disk": disk,
            });
            let msg = if let Some(c) = client {
                match c
                    .post_json("/api/instances/create-worker", &[], &body, Duration::from_secs(300))
                    .await
                {
                    HttpOutcome::Ok(v) => v.as_str().unwrap_or("Worker created").to_string(),
                    HttpOutcome::ApiErr(e) => return Err(e),
                    HttpOutcome::Unavailable(_) => {
                        colima::create_worker_node(master_profile, worker_profile, cpus, memory)
                            .await
                            .map_err(|e| e.to_string())?
                    }
                }
            } else {
                colima::create_worker_node(master_profile, worker_profile, cpus, memory)
                    .await
                    .map_err(|e| e.to_string())?
            };
            emit_message(&msg, use_json);
            Ok(())
        }
        VmCommand::Passthrough(args) => {
            let svc = crate::services::vm::VMService::colima();
            svc.passthrough(&args).await.map_err(|e| e.to_string())
        }
    }
}

// ============================================================================
// 2. Config Handler
// ============================================================================

pub async fn handle_config(
    action: ConfigCommand,
    client: Option<&DesktopClient>,
    use_json: bool,
) -> Result<(), String> {
    match action {
        ConfigCommand::Get { profile } => {
            let val = get_config_snapshot_value(&profile, client).await?;
            if use_json {
                print_json_ok(&val);
            } else {
                print_human_value(&val);
            }
            Ok(())
        }
        ConfigCommand::Preview { yaml, profile } => {
            let changes = parse_yaml_to_config_changes(&yaml)?;
            let val = if let Some(c) = client {
                let body = json!({
                    "profile": profile,
                    "changes": config_changes_to_json(&changes),
                });
                match c
                    .post_json(
                        "/api/instances/config/preview",
                        &[],
                        &body,
                        Duration::from_secs(15),
                    )
                    .await
                {
                    HttpOutcome::Ok(v) => v,
                    HttpOutcome::ApiErr(e) => return Err(e),
                    HttpOutcome::Unavailable(_) => {
                        let res = colima_config::preview_colima_config(profile, changes)
                            .await
                            .map_err(|e| e.to_string())?;
                        serde_json::to_value(res).map_err(|e| e.to_string())?
                    }
                }
            } else {
                let res = colima_config::preview_colima_config(profile, changes)
                    .await
                    .map_err(|e| e.to_string())?;
                serde_json::to_value(res).map_err(|e| e.to_string())?
            };

            if use_json {
                print_json_ok(&val);
            } else {
                print_human_value(&val);
            }
            Ok(())
        }
        ConfigCommand::Apply {
            yaml,
            profile,
            restart,
        } => {
            let changes = parse_yaml_to_config_changes(&yaml)?;
            let snapshot = get_config_snapshot_value(&profile, client).await?;
            let expected_mtime = snapshot.get("mtime").and_then(Value::as_i64).unwrap_or(0);

            let mut val = if let Some(c) = client {
                let body = json!({
                    "profile": profile,
                    "changes": config_changes_to_json(&changes),
                    "expectedMtime": expected_mtime,
                });
                match c
                    .post_json(
                        "/api/instances/config/apply",
                        &[],
                        &body,
                        Duration::from_secs(20),
                    )
                    .await
                {
                    HttpOutcome::Ok(v) => v,
                    HttpOutcome::ApiErr(e) => return Err(e),
                    HttpOutcome::Unavailable(_) => {
                        let res = colima_config::apply_colima_config(
                            profile.clone(),
                            changes,
                            expected_mtime,
                        )
                        .await
                        .map_err(|e| e.to_string())?;
                        serde_json::to_value(res).map_err(|e| e.to_string())?
                    }
                }
            } else {
                let res =
                    colima_config::apply_colima_config(profile.clone(), changes, expected_mtime)
                        .await
                        .map_err(|e| e.to_string())?;
                serde_json::to_value(res).map_err(|e| e.to_string())?
            };

            if restart {
                let _ = colima::stop_instance_cli(profile.clone(), false).await;
                let start_cfg =
                    build_vm_start_config(&profile, None, None, None, None, None, None, false);
                colima::start_instance(start_cfg)
                    .await
                    .map_err(|e| e.to_string())?;
                if let Some(obj) = val.as_object_mut() {
                    obj.insert("restarted".into(), json!(true));
                }
            }

            if use_json {
                print_json_ok(&val);
            } else {
                print_human_value(&val);
            }
            Ok(())
        }
    }
}

pub(crate) async fn get_config_snapshot_value(
    profile: &str,
    client: Option<&DesktopClient>,
) -> Result<Value, String> {
    if let Some(c) = client {
        match c
            .get(
                "/api/instances/config",
                &[("profile", profile)],
                Duration::from_secs(10),
            )
            .await
        {
            HttpOutcome::Ok(v) => return Ok(v),
            HttpOutcome::ApiErr(e) => return Err(e),
            HttpOutcome::Unavailable(_) => {}
        }
    }
    let snap = colima_config::get_colima_config(profile.to_string())
        .await
        .map_err(|e| e.to_string())?;
    serde_json::to_value(snap).map_err(|e| e.to_string())
}

// ============================================================================
// 3. Presets Handler
// ============================================================================

pub async fn handle_presets(
    action: PresetsCommand,
    client: Option<&DesktopClient>,
    use_json: bool,
) -> Result<(), String> {
    ensure_kb_init();
    match action {
        PresetsCommand::List => {
            let val = if let Some(c) = client {
                match c.get("/api/presets", &[], Duration::from_secs(10)).await {
                    HttpOutcome::Ok(v) => v,
                    HttpOutcome::ApiErr(e) => return Err(e),
                    HttpOutcome::Unavailable(_) => {
                        let map = knowledge_bank::get_all_presets().await?;
                        serde_json::to_value(map).map_err(|e| e.to_string())?
                    }
                }
            } else {
                let map = knowledge_bank::get_all_presets().await?;
                serde_json::to_value(map).map_err(|e| e.to_string())?
            };

            if use_json {
                print_json_ok(&val);
            } else if let Some(obj) = val.as_object() {
                if obj.is_empty() {
                    println!("No saved presets.");
                } else {
                    println!("{:<20} CONFIG", "PRESET ID");
                    for (id, cfg) in obj {
                        let cfg_str = cfg.as_str().unwrap_or("");
                        println!("{:<20} {}", truncate(id, 19), cfg_str);
                    }
                }
            } else {
                print_human_value(&val);
            }
            Ok(())
        }
        PresetsCommand::Save {
            id,
            name,
            cpus,
            memory,
            disk,
            runtime,
        } => {
            let config_json = serde_json::to_string(&json!({
                "id": id,
                "name": name,
                "cpus": cpus,
                "memory": memory,
                "disk": disk,
                "runtime": runtime,
            }))
            .map_err(|e| e.to_string())?;

            let msg = if let Some(c) = client {
                let body = json!({ "id": id, "configJson": config_json });
                match c
                    .post_json("/api/presets", &[], &body, Duration::from_secs(10))
                    .await
                {
                    HttpOutcome::Ok(v) => v.as_str().unwrap_or("Preset saved").to_string(),
                    HttpOutcome::ApiErr(e) => return Err(e),
                    HttpOutcome::Unavailable(_) => knowledge_bank::save_preset(id, config_json)
                        .await
                        .map_err(|e| e.to_string())?,
                }
            } else {
                knowledge_bank::save_preset(id, config_json)
                    .await
                    .map_err(|e| e.to_string())?
            };
            emit_message(&msg, use_json);
            Ok(())
        }
        PresetsCommand::Delete { id } => {
            let msg = if let Some(c) = client {
                let body = json!({ "id": id });
                match c
                    .post_json("/api/presets/delete", &[], &body, Duration::from_secs(10))
                    .await
                {
                    HttpOutcome::Ok(v) => v.as_str().unwrap_or("Preset deleted").to_string(),
                    HttpOutcome::ApiErr(e) => return Err(e),
                    HttpOutcome::Unavailable(_) => knowledge_bank::delete_preset(id)
                        .await
                        .map_err(|e| e.to_string())?,
                }
            } else {
                knowledge_bank::delete_preset(id)
                    .await
                    .map_err(|e| e.to_string())?
            };
            emit_message(&msg, use_json);
            Ok(())
        }
        PresetsCommand::Snapshots { profile } => {
            let val = if let Some(c) = client {
                match c
                    .get(
                        "/api/presets/snapshots",
                        &[("profile", profile.as_str())],
                        Duration::from_secs(10),
                    )
                    .await
                {
                    HttpOutcome::Ok(v) => v,
                    HttpOutcome::ApiErr(e) => return Err(e),
                    HttpOutcome::Unavailable(_) => {
                        let snaps = knowledge_bank::list_all_preset_snapshots(profile)
                            .await
                            .map_err(|e| e.to_string())?;
                        serde_json::to_value(snaps).map_err(|e| e.to_string())?
                    }
                }
            } else {
                let snaps = knowledge_bank::list_all_preset_snapshots(profile)
                    .await
                    .map_err(|e| e.to_string())?;
                serde_json::to_value(snaps).map_err(|e| e.to_string())?
            };

            if use_json {
                print_json_ok(&val);
            } else {
                print_human_value(&val);
            }
            Ok(())
        }
    }
}

// ============================================================================
// 4. Container Handler
// ============================================================================

pub async fn handle_container(
    action: ContainerCommand,
    client: Option<&DesktopClient>,
    use_json: bool,
) -> Result<(), String> {
    match action {
        ContainerCommand::List { all } => {
            let all_str = if all { "true" } else { "false" };
            let val = if let Some(c) = client {
                match c
                    .get("/api/containers", &[("all", all_str)], Duration::from_secs(15))
                    .await
                {
                    HttpOutcome::Ok(v) => v,
                    HttpOutcome::ApiErr(e) => return Err(e),
                    HttpOutcome::Unavailable(_) => {
                        let list = containers::list_containers_cli(all).await?;
                        serde_json::to_value(list).map_err(|e| e.to_string())?
                    }
                }
            } else {
                let list = containers::list_containers_cli(all).await?;
                serde_json::to_value(list).map_err(|e| e.to_string())?
            };

            if use_json {
                print_json_ok(&val);
            } else if let Some(arr) = val.as_array() {
                println!(
                    "{:<15} {:<25} {:<30} {:<20} PORTS",
                    "CONTAINER ID", "NAME", "IMAGE", "STATUS"
                );
                for item in arr {
                    let id = item
                        .get("id")
                        .or_else(|| item.get("ID"))
                        .and_then(Value::as_str)
                        .unwrap_or("-");
                    let short_id: String = id.chars().take(12).collect();
                    let name = item
                        .get("name")
                        .or_else(|| item.get("Names"))
                        .and_then(Value::as_str)
                        .unwrap_or("-");
                    let image = item
                        .get("image")
                        .or_else(|| item.get("Image"))
                        .and_then(Value::as_str)
                        .unwrap_or("-");
                    let status = item
                        .get("status")
                        .or_else(|| item.get("Status"))
                        .and_then(Value::as_str)
                        .unwrap_or("-");
                    let ports = item
                        .get("ports")
                        .or_else(|| item.get("Ports"))
                        .and_then(Value::as_str)
                        .unwrap_or("");
                    println!(
                        "{:<15} {:<25} {:<30} {:<20} {}",
                        short_id,
                        truncate(name, 24),
                        truncate(image, 29),
                        truncate(status, 19),
                        ports
                    );
                }
                println!("\n{} containers total", arr.len());
            } else {
                print_human_value(&val);
            }
            Ok(())
        }
        ContainerCommand::Logs { id, tail } => {
            let tail_str = tail.to_string();
            let logs = if let Some(c) = client {
                match c
                    .get(
                        "/api/containers/logs",
                        &[("containerId", id.as_str()), ("lines", tail_str.as_str())],
                        Duration::from_secs(15),
                    )
                    .await
                {
                    HttpOutcome::Ok(v) => v.as_str().unwrap_or_default().to_string(),
                    HttpOutcome::ApiErr(e) => return Err(e),
                    HttpOutcome::Unavailable(_) => containers::container_logs(id, tail)
                        .await
                        .map_err(|e| e.to_string())?,
                }
            } else {
                containers::container_logs(id, tail)
                    .await
                    .map_err(|e| e.to_string())?
            };

            if use_json {
                print_json_ok(&json!({ "logs": logs }));
            } else {
                print!("{logs}");
                if !logs.is_empty() && !logs.ends_with('\n') {
                    println!();
                }
            }
            Ok(())
        }
        ContainerCommand::Inspect { id } => {
            let raw = if let Some(c) = client {
                match c
                    .get(
                        "/api/containers/inspect",
                        &[("containerId", id.as_str())],
                        Duration::from_secs(15),
                    )
                    .await
                {
                    HttpOutcome::Ok(v) => try_parse_json_str(v),
                    HttpOutcome::ApiErr(e) => return Err(e),
                    HttpOutcome::Unavailable(_) => {
                        let s = containers::inspect_container(id)
                            .await
                            .map_err(|e| e.to_string())?;
                        try_parse_json_str(Value::String(s))
                    }
                }
            } else {
                let s = containers::inspect_container(id)
                    .await
                    .map_err(|e| e.to_string())?;
                try_parse_json_str(Value::String(s))
            };

            if use_json {
                print_json_ok(&raw);
            } else {
                print_human_value(&raw);
            }
            Ok(())
        }
        ContainerCommand::Stats { id } => {
            let raw = if let Some(c) = client {
                let outcome = match &id {
                    Some(cid) => {
                        c.get(
                            "/api/containers/stats",
                            &[("containerId", cid.as_str())],
                            Duration::from_secs(15),
                        )
                        .await
                    }
                    None => {
                        c.get("/api/containers/stats/all", &[], Duration::from_secs(15))
                            .await
                    }
                };
                match outcome {
                    HttpOutcome::Ok(v) => v.as_str().unwrap_or_default().to_string(),
                    HttpOutcome::ApiErr(e) => return Err(e),
                    HttpOutcome::Unavailable(_) => ContainerService::auto_detect()
                        .container_stats(id.as_deref())
                        .await
                        .map_err(|e| e.to_string())?,
                }
            } else {
                ContainerService::auto_detect()
                    .container_stats(id.as_deref())
                    .await
                    .map_err(|e| e.to_string())?
            };

            let entries: Vec<Value> = raw
                .lines()
                .filter(|l| !l.trim().is_empty())
                .filter_map(|l| serde_json::from_str::<Value>(l).ok())
                .collect();

            if use_json {
                if entries.is_empty() && !raw.trim().is_empty() {
                    print_json_ok(&json!({ "raw": raw }));
                } else {
                    print_json_ok(&entries);
                }
            } else if entries.is_empty() && !raw.trim().is_empty() {
                print!("{raw}");
            } else {
                println!(
                    "{:<14} {:<24} {:<10} {:<24} {:<10} {:<22} {:<22} PIDS",
                    "CONTAINER ID", "NAME", "CPU %", "MEM USAGE / LIMIT", "MEM %", "NET I/O", "BLOCK I/O"
                );
                for entry in &entries {
                    let cid = entry
                        .get("ID")
                        .or_else(|| entry.get("Container"))
                        .and_then(Value::as_str)
                        .unwrap_or("-");
                    let short_id: String = cid.chars().take(12).collect();
                    let name = entry.get("Name").and_then(Value::as_str).unwrap_or("-");
                    let cpu = entry.get("CPUPerc").and_then(Value::as_str).unwrap_or("-");
                    let mem_usage = entry.get("MemUsage").and_then(Value::as_str).unwrap_or("-");
                    let mem_perc = entry.get("MemPerc").and_then(Value::as_str).unwrap_or("-");
                    let net_io = entry.get("NetIO").and_then(Value::as_str).unwrap_or("-");
                    let block_io = entry.get("BlockIO").and_then(Value::as_str).unwrap_or("-");
                    let pids = entry.get("PIDs").and_then(Value::as_str).unwrap_or("-");

                    println!(
                        "{:<14} {:<24} {:<10} {:<24} {:<10} {:<22} {:<22} {}",
                        short_id,
                        truncate(name, 23),
                        truncate(cpu, 9),
                        truncate(mem_usage, 23),
                        truncate(mem_perc, 9),
                        truncate(net_io, 21),
                        truncate(block_io, 21),
                        pids
                    );
                }
            }
            Ok(())
        }
        ContainerCommand::Top { id } => {
            let out = if let Some(c) = client {
                match c
                    .get(
                        "/api/containers/top",
                        &[("containerId", id.as_str())],
                        Duration::from_secs(15),
                    )
                    .await
                {
                    HttpOutcome::Ok(v) => v.as_str().unwrap_or_default().to_string(),
                    HttpOutcome::ApiErr(e) => return Err(e),
                    HttpOutcome::Unavailable(_) => containers::container_top(id)
                        .await
                        .map_err(|e| e.to_string())?,
                }
            } else {
                containers::container_top(id)
                    .await
                    .map_err(|e| e.to_string())?
            };
            emit_raw_or_json(&out, "processes", use_json);
            Ok(())
        }
        ContainerCommand::Start { id } => {
            run_simple_container_action("start", &id, false, client, use_json).await
        }
        ContainerCommand::Stop { id } => {
            run_simple_container_action("stop", &id, false, client, use_json).await
        }
        ContainerCommand::Restart { id } => {
            run_simple_container_action("restart", &id, false, client, use_json).await
        }
        ContainerCommand::Pause { id } => {
            run_simple_container_action("pause", &id, false, client, use_json).await
        }
        ContainerCommand::Unpause { id } => {
            run_simple_container_action("unpause", &id, false, client, use_json).await
        }
        ContainerCommand::Remove { id, force } => {
            run_simple_container_action("remove", &id, force, client, use_json).await
        }
        ContainerCommand::Rename { id, new_name } => {
            let msg = if let Some(c) = client {
                let body = json!({ "containerId": id, "newName": new_name });
                match c
                    .post_json("/api/containers/rename", &[], &body, Duration::from_secs(15))
                    .await
                {
                    HttpOutcome::Ok(v) => v.as_str().unwrap_or("Renamed").to_string(),
                    HttpOutcome::ApiErr(e) => return Err(e),
                    HttpOutcome::Unavailable(_) => containers::rename_container(id, new_name)
                        .await
                        .map_err(|e| e.to_string())?,
                }
            } else {
                containers::rename_container(id, new_name)
                    .await
                    .map_err(|e| e.to_string())?
            };
            emit_message(&msg, use_json);
            Ok(())
        }
        ContainerCommand::Exec { id, cmd } => {
            let joined = cmd.join(" ");
            let out = if let Some(c) = client {
                let body = json!({ "containerId": id, "command": joined });
                match c
                    .post_json("/api/containers/exec", &[], &body, Duration::from_secs(60))
                    .await
                {
                    HttpOutcome::Ok(v) => v.as_str().unwrap_or_default().to_string(),
                    HttpOutcome::ApiErr(e) => return Err(e),
                    HttpOutcome::Unavailable(_) => containers::container_exec(id, joined)
                        .await
                        .map_err(|e| e.to_string())?,
                }
            } else {
                containers::container_exec(id, joined)
                    .await
                    .map_err(|e| e.to_string())?
            };
            emit_raw_or_json(&out, "output", use_json);
            Ok(())
        }
        ContainerCommand::Run {
            image,
            name,
            ports,
            env,
            detach,
        } => {
            let cname = name.unwrap_or_default();
            let out = if let Some(c) = client {
                let body = json!({
                    "image": image,
                    "name": cname,
                    "ports": ports,
                    "envVars": env,
                    "volumes": [],
                    "detach": detach,
                    "removeOnExit": false,
                    "extraArgs": [],
                });
                match c
                    .post_json("/api/containers/run", &[], &body, Duration::from_secs(300))
                    .await
                {
                    HttpOutcome::Ok(v) => v.as_str().unwrap_or_default().to_string(),
                    HttpOutcome::ApiErr(e) => return Err(e),
                    HttpOutcome::Unavailable(_) => containers::run_container(
                        image,
                        cname,
                        ports,
                        env,
                        Vec::new(),
                        detach,
                        false,
                        Vec::new(),
                    )
                    .await
                    .map_err(|e| e.to_string())?,
                }
            } else {
                containers::run_container(
                    image,
                    cname,
                    ports,
                    env,
                    Vec::new(),
                    detach,
                    false,
                    Vec::new(),
                )
                .await
                .map_err(|e| e.to_string())?
            };
            emit_message(&out, use_json);
            Ok(())
        }
        ContainerCommand::Passthrough(args) => ContainerService::auto_detect()
            .passthrough(&args)
            .await
            .map_err(|e| e.to_string()),
    }
}

async fn run_simple_container_action(
    verb: &str,
    id: &str,
    force: bool,
    client: Option<&DesktopClient>,
    use_json: bool,
) -> Result<(), String> {
    if let Some(c) = client {
        let path = format!("/api/containers/{verb}");
        let force_str = if force { "true" } else { "false" };
        let params: Vec<(&str, &str)> = if verb == "remove" {
            vec![("containerId", id), ("force", force_str)]
        } else {
            vec![("containerId", id)]
        };
        match c.post_empty(&path, &params, Duration::from_secs(30)).await {
            HttpOutcome::Ok(v) => {
                emit_message(v.as_str().unwrap_or("OK"), use_json);
                return Ok(());
            }
            HttpOutcome::ApiErr(e) => return Err(e),
            HttpOutcome::Unavailable(_) => {}
        }
    }

    let cid = id.to_string();
    let msg = match verb {
        "start" => containers::start_container(cid).await.map_err(|e| e.to_string())?,
        "stop" => containers::stop_container(cid).await.map_err(|e| e.to_string())?,
        "restart" => containers::restart_container(cid).await.map_err(|e| e.to_string())?,
        "pause" => containers::pause_container(cid).await.map_err(|e| e.to_string())?,
        "unpause" => containers::unpause_container(cid).await.map_err(|e| e.to_string())?,
        "remove" => containers::remove_container(cid, force).await.map_err(|e| e.to_string())?,
        _ => return Err(format!("Unsupported container action: {verb}")),
    };
    emit_message(&msg, use_json);
    Ok(())
}

// ============================================================================
// 5. Image Handler
// ============================================================================

pub async fn handle_image(
    action: ImageCommand,
    client: Option<&DesktopClient>,
    use_json: bool,
) -> Result<(), String> {
    match action {
        ImageCommand::List => {
            let val = if let Some(c) = client {
                match c.get("/api/images", &[], Duration::from_secs(15)).await {
                    HttpOutcome::Ok(v) => v,
                    HttpOutcome::ApiErr(e) => return Err(e),
                    HttpOutcome::Unavailable(_) => {
                        let imgs = ContainerService::auto_detect()
                            .list_images()
                            .await
                            .map_err(|e| e.to_string())?;
                        serde_json::to_value(imgs).map_err(|e| e.to_string())?
                    }
                }
            } else {
                let imgs = ContainerService::auto_detect()
                    .list_images()
                    .await
                    .map_err(|e| e.to_string())?;
                serde_json::to_value(imgs).map_err(|e| e.to_string())?
            };

            if use_json {
                print_json_ok(&val);
            } else if let Some(arr) = val.as_array() {
                println!("{:<40} {:<15} {:<15}", "REPOSITORY", "TAG", "SIZE");
                for img in arr {
                    let repo = img
                        .get("repository")
                        .or_else(|| img.get("Repository"))
                        .and_then(Value::as_str)
                        .unwrap_or("<none>");
                    let tag = img
                        .get("tag")
                        .or_else(|| img.get("Tag"))
                        .and_then(Value::as_str)
                        .unwrap_or("<none>");
                    let size = img
                        .get("size")
                        .or_else(|| img.get("Size"))
                        .and_then(Value::as_str)
                        .unwrap_or("-");
                    println!(
                        "{:<40} {:<15} {:<15}",
                        truncate(repo, 39),
                        truncate(tag, 14),
                        size
                    );
                }
            } else {
                print_human_value(&val);
            }
            Ok(())
        }
        ImageCommand::Inspect { id } => {
            let val = if let Some(c) = client {
                match c
                    .get(
                        "/api/images/inspect",
                        &[("imageId", id.as_str())],
                        Duration::from_secs(15),
                    )
                    .await
                {
                    HttpOutcome::Ok(v) => try_parse_json_str(v),
                    HttpOutcome::ApiErr(e) => return Err(e),
                    HttpOutcome::Unavailable(_) => {
                        let s = containers::inspect_image(id)
                            .await
                            .map_err(|e| e.to_string())?;
                        try_parse_json_str(Value::String(s))
                    }
                }
            } else {
                let s = containers::inspect_image(id)
                    .await
                    .map_err(|e| e.to_string())?;
                try_parse_json_str(Value::String(s))
            };
            if use_json {
                print_json_ok(&val);
            } else {
                print_human_value(&val);
            }
            Ok(())
        }
        ImageCommand::Pull { image } => {
            let msg = if let Some(c) = client {
                match c
                    .post_empty(
                        "/api/images/pull",
                        &[("imageName", image.as_str())],
                        Duration::from_secs(300),
                    )
                    .await
                {
                    HttpOutcome::Ok(v) => v.as_str().unwrap_or("Pulled").to_string(),
                    HttpOutcome::ApiErr(e) => return Err(e),
                    HttpOutcome::Unavailable(_) => ContainerService::auto_detect()
                        .pull_image(&image)
                        .await
                        .map_err(|e| e.to_string())?,
                }
            } else {
                ContainerService::auto_detect()
                    .pull_image(&image)
                    .await
                    .map_err(|e| e.to_string())?
            };
            emit_message(&msg, use_json);
            Ok(())
        }
        ImageCommand::Tag { source, target } => {
            let msg = if let Some(c) = client {
                let body = json!({ "source": source, "target": target });
                match c
                    .post_json("/api/images/tag", &[], &body, Duration::from_secs(20))
                    .await
                {
                    HttpOutcome::Ok(v) => v.as_str().unwrap_or("Tagged").to_string(),
                    HttpOutcome::ApiErr(e) => return Err(e),
                    HttpOutcome::Unavailable(_) => containers::tag_image(source, target)
                        .await
                        .map_err(|e| e.to_string())?,
                }
            } else {
                containers::tag_image(source, target)
                    .await
                    .map_err(|e| e.to_string())?
            };
            emit_message(&msg, use_json);
            Ok(())
        }
        ImageCommand::Remove { id, force } => {
            let force_str = if force { "true" } else { "false" };
            let msg = if let Some(c) = client {
                match c
                    .post_empty(
                        "/api/images/remove",
                        &[("imageId", id.as_str()), ("force", force_str)],
                        Duration::from_secs(30),
                    )
                    .await
                {
                    HttpOutcome::Ok(v) => v.as_str().unwrap_or("Removed").to_string(),
                    HttpOutcome::ApiErr(e) => return Err(e),
                    HttpOutcome::Unavailable(_) => ContainerService::auto_detect()
                        .remove_image(&id, force)
                        .await
                        .map_err(|e| e.to_string())?,
                }
            } else {
                ContainerService::auto_detect()
                    .remove_image(&id, force)
                    .await
                    .map_err(|e| e.to_string())?
            };
            emit_message(&msg, use_json);
            Ok(())
        }
        ImageCommand::Prune => {
            let msg = if let Some(c) = client {
                match c
                    .post_empty("/api/images/prune", &[], Duration::from_secs(60))
                    .await
                {
                    HttpOutcome::Ok(v) => v.as_str().unwrap_or("Pruned").to_string(),
                    HttpOutcome::ApiErr(e) => return Err(e),
                    HttpOutcome::Unavailable(_) => ContainerService::auto_detect()
                        .prune_images()
                        .await
                        .map_err(|e| e.to_string())?,
                }
            } else {
                ContainerService::auto_detect()
                    .prune_images()
                    .await
                    .map_err(|e| e.to_string())?
            };
            emit_message(&msg, use_json);
            Ok(())
        }
        ImageCommand::Save { image, output_tar } => {
            let out = runtime::run(
                vec!["save".into(), "-o".into(), output_tar.clone(), image.clone()],
                runtime::LONG_OP_TIMEOUT,
            )
            .await?;
            if !out.status.success() {
                return Err(format!(
                    "image save failed: {}",
                    String::from_utf8_lossy(&out.stderr).trim()
                ));
            }
            if use_json {
                print_json_ok(&json!({ "image": image, "output_tar": output_tar, "saved": true }));
            } else {
                println!("Saved '{image}' to {output_tar}");
            }
            Ok(())
        }
        ImageCommand::Load { input_tar } => {
            let out = runtime::run(
                vec!["load".into(), "-i".into(), input_tar.clone()],
                runtime::LONG_OP_TIMEOUT,
            )
            .await?;
            if !out.status.success() {
                return Err(format!(
                    "image load failed: {}",
                    String::from_utf8_lossy(&out.stderr).trim()
                ));
            }
            let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
            emit_message(
                if stdout.is_empty() {
                    &input_tar
                } else {
                    &stdout
                },
                use_json,
            );
            Ok(())
        }
    }
}

// ============================================================================
// 6. Volume Handler
// ============================================================================

pub async fn handle_volume(
    action: VolumeCommand,
    client: Option<&DesktopClient>,
    use_json: bool,
) -> Result<(), String> {
    match action {
        VolumeCommand::List => {
            let val = if let Some(c) = client {
                match c.get("/api/volumes", &[], Duration::from_secs(15)).await {
                    HttpOutcome::Ok(v) => v,
                    HttpOutcome::ApiErr(e) => return Err(e),
                    HttpOutcome::Unavailable(_) => {
                        let vols = volumes::list_volumes().await.map_err(|e| e.to_string())?;
                        serde_json::to_value(vols).map_err(|e| e.to_string())?
                    }
                }
            } else {
                let vols = volumes::list_volumes().await.map_err(|e| e.to_string())?;
                serde_json::to_value(vols).map_err(|e| e.to_string())?
            };

            if use_json {
                print_json_ok(&val);
            } else if let Some(arr) = val.as_array() {
                println!("{:<36} {:<14} MOUNTPOINT", "VOLUME NAME", "DRIVER");
                for v in arr {
                    let name = v.get("name").and_then(Value::as_str).unwrap_or("-");
                    let drv = v.get("driver").and_then(Value::as_str).unwrap_or("-");
                    let mp = v.get("mountpoint").and_then(Value::as_str).unwrap_or("");
                    println!("{:<36} {:<14} {}", truncate(name, 35), drv, mp);
                }
            } else {
                print_human_value(&val);
            }
            Ok(())
        }
        VolumeCommand::Inspect { name } => {
            let val = if let Some(c) = client {
                match c
                    .get(
                        "/api/volumes/inspect",
                        &[("name", name.as_str())],
                        Duration::from_secs(15),
                    )
                    .await
                {
                    HttpOutcome::Ok(v) => try_parse_json_str(v),
                    HttpOutcome::ApiErr(e) => return Err(e),
                    HttpOutcome::Unavailable(_) => {
                        let s = volumes::inspect_volume(name)
                            .await
                            .map_err(|e| e.to_string())?;
                        try_parse_json_str(Value::String(s))
                    }
                }
            } else {
                let s = volumes::inspect_volume(name)
                    .await
                    .map_err(|e| e.to_string())?;
                try_parse_json_str(Value::String(s))
            };
            if use_json {
                print_json_ok(&val);
            } else {
                print_human_value(&val);
            }
            Ok(())
        }
        VolumeCommand::Create { name, driver } => {
            let msg = if let Some(c) = client {
                let body = json!({ "name": name, "driver": driver });
                match c
                    .post_json("/api/volumes/create", &[], &body, Duration::from_secs(15))
                    .await
                {
                    HttpOutcome::Ok(v) => v.as_str().unwrap_or("Created").to_string(),
                    HttpOutcome::ApiErr(e) => return Err(e),
                    HttpOutcome::Unavailable(_) => volumes::create_volume(name, driver)
                        .await
                        .map_err(|e| e.to_string())?,
                }
            } else {
                volumes::create_volume(name, driver)
                    .await
                    .map_err(|e| e.to_string())?
            };
            emit_message(&msg, use_json);
            Ok(())
        }
        VolumeCommand::Remove { name, force } => {
            let force_str = if force { "true" } else { "false" };
            let msg = if let Some(c) = client {
                match c
                    .post_empty(
                        "/api/volumes/remove",
                        &[("name", name.as_str()), ("force", force_str)],
                        Duration::from_secs(15),
                    )
                    .await
                {
                    HttpOutcome::Ok(v) => v.as_str().unwrap_or("Removed").to_string(),
                    HttpOutcome::ApiErr(e) => return Err(e),
                    HttpOutcome::Unavailable(_) => volumes::remove_volume(name, force)
                        .await
                        .map_err(|e| e.to_string())?,
                }
            } else {
                volumes::remove_volume(name, force)
                    .await
                    .map_err(|e| e.to_string())?
            };
            emit_message(&msg, use_json);
            Ok(())
        }
        VolumeCommand::Prune => {
            let msg = if let Some(c) = client {
                match c
                    .post_empty("/api/volumes/prune", &[], Duration::from_secs(30))
                    .await
                {
                    HttpOutcome::Ok(v) => v.as_str().unwrap_or("Pruned").to_string(),
                    HttpOutcome::ApiErr(e) => return Err(e),
                    HttpOutcome::Unavailable(_) => {
                        volumes::prune_volumes().await.map_err(|e| e.to_string())?
                    }
                }
            } else {
                volumes::prune_volumes().await.map_err(|e| e.to_string())?
            };
            emit_message(&msg, use_json);
            Ok(())
        }
    }
}

// ============================================================================
// 7. Network Handler
// ============================================================================

pub async fn handle_network(
    action: NetworkCommand,
    client: Option<&DesktopClient>,
    use_json: bool,
) -> Result<(), String> {
    match action {
        NetworkCommand::List => {
            let val = if let Some(c) = client {
                match c.get("/api/networks", &[], Duration::from_secs(15)).await {
                    HttpOutcome::Ok(v) => v,
                    HttpOutcome::ApiErr(e) => return Err(e),
                    HttpOutcome::Unavailable(_) => {
                        let nets = networks::list_networks().await.map_err(|e| e.to_string())?;
                        serde_json::to_value(nets).map_err(|e| e.to_string())?
                    }
                }
            } else {
                let nets = networks::list_networks().await.map_err(|e| e.to_string())?;
                serde_json::to_value(nets).map_err(|e| e.to_string())?
            };

            if use_json {
                print_json_ok(&val);
            } else if let Some(arr) = val.as_array() {
                println!("{:<15} {:<28} {:<14} SCOPE", "NETWORK ID", "NAME", "DRIVER");
                for n in arr {
                    let id = n.get("id").and_then(Value::as_str).unwrap_or("-");
                    let short_id: String = id.chars().take(12).collect();
                    let name = n.get("name").and_then(Value::as_str).unwrap_or("-");
                    let drv = n.get("driver").and_then(Value::as_str).unwrap_or("-");
                    let scope = n.get("scope").and_then(Value::as_str).unwrap_or("-");
                    println!("{:<15} {:<28} {:<14} {}", short_id, truncate(name, 27), drv, scope);
                }
            } else {
                print_human_value(&val);
            }
            Ok(())
        }
        NetworkCommand::Inspect { name } => {
            let val = if let Some(c) = client {
                match c
                    .get(
                        "/api/networks/inspect",
                        &[("name", name.as_str())],
                        Duration::from_secs(15),
                    )
                    .await
                {
                    HttpOutcome::Ok(v) => try_parse_json_str(v),
                    HttpOutcome::ApiErr(e) => return Err(e),
                    HttpOutcome::Unavailable(_) => {
                        let s = networks::inspect_network(name)
                            .await
                            .map_err(|e| e.to_string())?;
                        try_parse_json_str(Value::String(s))
                    }
                }
            } else {
                let s = networks::inspect_network(name)
                    .await
                    .map_err(|e| e.to_string())?;
                try_parse_json_str(Value::String(s))
            };
            if use_json {
                print_json_ok(&val);
            } else {
                print_human_value(&val);
            }
            Ok(())
        }
        NetworkCommand::Create {
            name,
            driver,
            subnet,
        } => {
            let msg = if let Some(c) = client {
                let body = json!({ "name": name, "driver": driver, "subnet": subnet });
                match c
                    .post_json("/api/networks/create", &[], &body, Duration::from_secs(15))
                    .await
                {
                    HttpOutcome::Ok(v) => v.as_str().unwrap_or("Created").to_string(),
                    HttpOutcome::ApiErr(e) => return Err(e),
                    HttpOutcome::Unavailable(_) => networks::create_network(name, driver, subnet)
                        .await
                        .map_err(|e| e.to_string())?,
                }
            } else {
                networks::create_network(name, driver, subnet)
                    .await
                    .map_err(|e| e.to_string())?
            };
            emit_message(&msg, use_json);
            Ok(())
        }
        NetworkCommand::Remove { name } => {
            let msg = if let Some(c) = client {
                match c
                    .post_empty(
                        "/api/networks/remove",
                        &[("name", name.as_str())],
                        Duration::from_secs(15),
                    )
                    .await
                {
                    HttpOutcome::Ok(v) => v.as_str().unwrap_or("Removed").to_string(),
                    HttpOutcome::ApiErr(e) => return Err(e),
                    HttpOutcome::Unavailable(_) => networks::remove_network(name)
                        .await
                        .map_err(|e| e.to_string())?,
                }
            } else {
                networks::remove_network(name)
                    .await
                    .map_err(|e| e.to_string())?
            };
            emit_message(&msg, use_json);
            Ok(())
        }
        NetworkCommand::Prune => {
            let msg = if let Some(c) = client {
                match c
                    .post_empty("/api/networks/prune", &[], Duration::from_secs(30))
                    .await
                {
                    HttpOutcome::Ok(v) => v.as_str().unwrap_or("Pruned").to_string(),
                    HttpOutcome::ApiErr(e) => return Err(e),
                    HttpOutcome::Unavailable(_) => {
                        networks::prune_networks().await.map_err(|e| e.to_string())?
                    }
                }
            } else {
                networks::prune_networks().await.map_err(|e| e.to_string())?
            };
            emit_message(&msg, use_json);
            Ok(())
        }
    }
}

// ============================================================================
// 8. Compose Handler
// ============================================================================

pub async fn handle_compose(
    action: ComposeCommand,
    client: Option<&DesktopClient>,
    use_json: bool,
) -> Result<(), String> {
    match action {
        ComposeCommand::List => {
            let val = if let Some(c) = client {
                match c.get("/api/compose", &[], Duration::from_secs(15)).await {
                    HttpOutcome::Ok(v) => v,
                    HttpOutcome::ApiErr(e) => return Err(e),
                    HttpOutcome::Unavailable(_) => {
                        let p = compose::list_compose_projects()
                            .await
                            .map_err(|e| e.to_string())?;
                        serde_json::to_value(p).map_err(|e| e.to_string())?
                    }
                }
            } else {
                let p = compose::list_compose_projects()
                    .await
                    .map_err(|e| e.to_string())?;
                serde_json::to_value(p).map_err(|e| e.to_string())?
            };

            if use_json {
                print_json_ok(&val);
            } else if let Some(arr) = val.as_array() {
                println!("{:<26} {:<18} CONFIG FILES", "PROJECT", "STATUS");
                for item in arr {
                    let name = item.get("name").and_then(Value::as_str).unwrap_or("-");
                    let status = item.get("status").and_then(Value::as_str).unwrap_or("-");
                    let cf = item
                        .get("configFiles")
                        .or_else(|| item.get("config_files"))
                        .and_then(Value::as_str)
                        .unwrap_or("");
                    println!("{:<26} {:<18} {}", truncate(name, 25), truncate(status, 17), cf);
                }
            } else {
                print_human_value(&val);
            }
            Ok(())
        }
        ComposeCommand::Ps { project_or_dir } => {
            let out = if let Some(c) = client {
                match c
                    .get(
                        "/api/compose/ps",
                        &[("projectName", project_or_dir.as_str())],
                        Duration::from_secs(15),
                    )
                    .await
                {
                    HttpOutcome::Ok(v) => v.as_str().unwrap_or_default().to_string(),
                    HttpOutcome::ApiErr(e) => return Err(e),
                    HttpOutcome::Unavailable(_) => compose::compose_ps(project_or_dir)
                        .await
                        .map_err(|e| e.to_string())?,
                }
            } else {
                compose::compose_ps(project_or_dir)
                    .await
                    .map_err(|e| e.to_string())?
            };
            emit_raw_or_json(&out, "containers", use_json);
            Ok(())
        }
        ComposeCommand::Logs {
            project_or_dir,
            tail,
        } => {
            let tail_str = tail.to_string();
            let out = if let Some(c) = client {
                match c
                    .get(
                        "/api/compose/logs",
                        &[
                            ("projectName", project_or_dir.as_str()),
                            ("lines", tail_str.as_str()),
                        ],
                        Duration::from_secs(15),
                    )
                    .await
                {
                    HttpOutcome::Ok(v) => v.as_str().unwrap_or_default().to_string(),
                    HttpOutcome::ApiErr(e) => return Err(e),
                    HttpOutcome::Unavailable(_) => compose::compose_logs(project_or_dir, tail)
                        .await
                        .map_err(|e| e.to_string())?,
                }
            } else {
                compose::compose_logs(project_or_dir, tail)
                    .await
                    .map_err(|e| e.to_string())?
            };
            emit_raw_or_json(&out, "logs", use_json);
            Ok(())
        }
        ComposeCommand::Up { dir, no_detach } => {
            let detach = !no_detach;
            let msg = if let Some(c) = client {
                let body = json!({ "projectDir": dir, "detach": detach });
                match c
                    .post_json("/api/compose/up", &[], &body, Duration::from_secs(300))
                    .await
                {
                    HttpOutcome::Ok(v) => v.as_str().unwrap_or("Compose up complete").to_string(),
                    HttpOutcome::ApiErr(e) => return Err(e),
                    HttpOutcome::Unavailable(_) => compose::compose_up(dir, detach)
                        .await
                        .map_err(|e| e.to_string())?,
                }
            } else {
                compose::compose_up(dir, detach)
                    .await
                    .map_err(|e| e.to_string())?
            };
            emit_message(&msg, use_json);
            Ok(())
        }
        ComposeCommand::Down { project } => {
            run_compose_project_action("down", project, client, use_json).await
        }
        ComposeCommand::Start { project } => {
            run_compose_project_action("start", project, client, use_json).await
        }
        ComposeCommand::Stop { project } => {
            run_compose_project_action("stop", project, client, use_json).await
        }
        ComposeCommand::Restart { project } => {
            run_compose_project_action("restart", project, client, use_json).await
        }
        ComposeCommand::Diagnose { file_or_dir } => {
            ensure_kb_init();
            let resolved = resolve_compose_file_path(&file_or_dir);
            let val = if let Some(c) = client {
                let body = json!({ "filePath": resolved });
                match c
                    .post_json("/api/compose/diagnose", &[], &body, Duration::from_secs(30))
                    .await
                {
                    HttpOutcome::Ok(v) => v,
                    HttpOutcome::ApiErr(e) => return Err(e),
                    HttpOutcome::Unavailable(_) => {
                        let diag = compose_diagnose::compose_diagnose(resolved)
                            .await
                            .map_err(|e| e.to_string())?;
                        serde_json::to_value(diag).map_err(|e| e.to_string())?
                    }
                }
            } else {
                let diag = compose_diagnose::compose_diagnose(resolved)
                    .await
                    .map_err(|e| e.to_string())?;
                serde_json::to_value(diag).map_err(|e| e.to_string())?
            };
            if use_json {
                print_json_ok(&val);
            } else {
                print_human_value(&val);
            }
            Ok(())
        }
        ComposeCommand::AutofixPropose { file_or_dir } => {
            ensure_kb_init();
            let resolved = resolve_compose_file_path(&file_or_dir);
            let val = if let Some(c) = client {
                let body = json!({ "filePath": resolved });
                match c
                    .post_json(
                        "/api/compose/autofix/propose",
                        &[],
                        &body,
                        Duration::from_secs(60),
                    )
                    .await
                {
                    HttpOutcome::Ok(v) => v,
                    HttpOutcome::ApiErr(e) => return Err(e),
                    HttpOutcome::Unavailable(_) => {
                        let prop = compose_autofix::compose_autofix_propose(resolved)
                            .await
                            .map_err(|e| e.to_string())?;
                        serde_json::to_value(prop).map_err(|e| e.to_string())?
                    }
                }
            } else {
                let prop = compose_autofix::compose_autofix_propose(resolved)
                    .await
                    .map_err(|e| e.to_string())?;
                serde_json::to_value(prop).map_err(|e| e.to_string())?
            };
            if use_json {
                print_json_ok(&val);
            } else {
                print_human_value(&val);
            }
            Ok(())
        }
        ComposeCommand::AutofixApply { patch_json_or_file } => {
            ensure_kb_init();
            let raw_patch = read_inline_or_file(&patch_json_or_file)?;
            let patch: Value = serde_json::from_str(&raw_patch)
                .map_err(|e| format!("Invalid patch JSON: {e}"))?;
            let file_path = patch
                .get("filePath")
                .or_else(|| patch.get("file_path"))
                .and_then(Value::as_str)
                .ok_or_else(|| "Patch JSON missing 'filePath'".to_string())?
                .to_string();
            let new_content = patch
                .get("newContent")
                .or_else(|| patch.get("new_content"))
                .and_then(Value::as_str)
                .ok_or_else(|| "Patch JSON missing 'newContent'".to_string())?
                .to_string();
            let explanation = patch
                .get("explanation")
                .and_then(Value::as_str)
                .unwrap_or("Applied via CLI")
                .to_string();
            let declared_removals: Vec<String> = patch
                .get("declaredRemovals")
                .or_else(|| patch.get("declared_removals"))
                .and_then(Value::as_array)
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default();

            let val = if let Some(c) = client {
                let body = json!({
                    "filePath": file_path,
                    "newContent": new_content,
                    "explanation": explanation,
                    "declaredRemovals": declared_removals,
                });
                match c
                    .post_json(
                        "/api/compose/autofix/apply",
                        &[],
                        &body,
                        Duration::from_secs(30),
                    )
                    .await
                {
                    HttpOutcome::Ok(v) => v,
                    HttpOutcome::ApiErr(e) => return Err(e),
                    HttpOutcome::Unavailable(_) => {
                        let rec = compose_autofix_apply::compose_autofix_apply(
                            file_path,
                            new_content,
                            explanation,
                            declared_removals,
                        )
                        .await
                        .map_err(|e| e.to_string())?;
                        serde_json::to_value(rec).map_err(|e| e.to_string())?
                    }
                }
            } else {
                let rec = compose_autofix_apply::compose_autofix_apply(
                    file_path,
                    new_content,
                    explanation,
                    declared_removals,
                )
                .await
                .map_err(|e| e.to_string())?;
                serde_json::to_value(rec).map_err(|e| e.to_string())?
            };
            if use_json {
                print_json_ok(&val);
            } else {
                print_human_value(&val);
            }
            Ok(())
        }
        ComposeCommand::AutofixUndo { backup_id } => {
            ensure_kb_init();
            let val = if let Some(c) = client {
                let body = json!({ "fixId": backup_id });
                match c
                    .post_json("/api/compose/autofix/undo", &[], &body, Duration::from_secs(20))
                    .await
                {
                    HttpOutcome::Ok(v) => v,
                    HttpOutcome::ApiErr(e) => return Err(e),
                    HttpOutcome::Unavailable(_) => {
                        let rec = compose_autofix_apply::compose_autofix_undo(backup_id)
                            .await
                            .map_err(|e| e.to_string())?;
                        serde_json::to_value(rec).map_err(|e| e.to_string())?
                    }
                }
            } else {
                let rec = compose_autofix_apply::compose_autofix_undo(backup_id)
                    .await
                    .map_err(|e| e.to_string())?;
                serde_json::to_value(rec).map_err(|e| e.to_string())?
            };
            if use_json {
                print_json_ok(&val);
            } else {
                print_human_value(&val);
            }
            Ok(())
        }
        ComposeCommand::AutofixHistory => {
            ensure_kb_init();
            let val = if let Some(c) = client {
                match c
                    .get("/api/compose/autofix/history", &[], Duration::from_secs(15))
                    .await
                {
                    HttpOutcome::Ok(v) => v,
                    HttpOutcome::ApiErr(e) => return Err(e),
                    HttpOutcome::Unavailable(_) => {
                        let hist = compose_autofix_apply::compose_autofix_history()
                            .await
                            .map_err(|e| e.to_string())?;
                        serde_json::to_value(hist).map_err(|e| e.to_string())?
                    }
                }
            } else {
                let hist = compose_autofix_apply::compose_autofix_history()
                    .await
                    .map_err(|e| e.to_string())?;
                serde_json::to_value(hist).map_err(|e| e.to_string())?
            };
            if use_json {
                print_json_ok(&val);
            } else {
                print_human_value(&val);
            }
            Ok(())
        }
        ComposeCommand::Passthrough(args) => crate::services::compose::ComposeService::auto_detect()
            .passthrough(&args)
            .await
            .map_err(|e| e.to_string()),
    }
}

async fn run_compose_project_action(
    verb: &str,
    project: String,
    client: Option<&DesktopClient>,
    use_json: bool,
) -> Result<(), String> {
    if let Some(c) = client {
        let path = format!("/api/compose/{verb}");
        let body = json!({ "projectName": project });
        match c.post_json(&path, &[], &body, Duration::from_secs(120)).await {
            HttpOutcome::Ok(v) => {
                emit_message(v.as_str().unwrap_or("OK"), use_json);
                return Ok(());
            }
            HttpOutcome::ApiErr(e) => return Err(e),
            HttpOutcome::Unavailable(_) => {}
        }
    }

    let msg = match verb {
        "down" => compose::compose_down(project).await.map_err(|e| e.to_string())?,
        "start" => compose::compose_start(project).await.map_err(|e| e.to_string())?,
        "stop" => compose::compose_stop(project).await.map_err(|e| e.to_string())?,
        "restart" => compose::compose_restart(project).await.map_err(|e| e.to_string())?,
        _ => return Err(format!("Unsupported compose action: {verb}")),
    };
    emit_message(&msg, use_json);
    Ok(())
}

// ============================================================================
// 9. Kubernetes Handler
// ============================================================================

pub async fn handle_k8s(
    action: K8sCommand,
    client: Option<&DesktopClient>,
    use_json: bool,
) -> Result<(), String> {
    match action {
        K8sCommand::Status => {
            let val = if let Some(c) = client {
                match c
                    .get("/api/k8s/cluster-health", &[], Duration::from_secs(15))
                    .await
                {
                    HttpOutcome::Ok(v) => try_parse_json_str(v),
                    HttpOutcome::ApiErr(e) => return Err(e),
                    HttpOutcome::Unavailable(_) => {
                        let s = k8s_cluster::k8s_cluster_health()
                            .await
                            .map_err(|e| e.to_string())?;
                        try_parse_json_str(Value::String(s))
                    }
                }
            } else {
                let s = k8s_cluster::k8s_cluster_health()
                    .await
                    .map_err(|e| e.to_string())?;
                try_parse_json_str(Value::String(s))
            };
            if use_json {
                print_json_ok(&val);
            } else {
                print_human_value(&val);
            }
            Ok(())
        }
        K8sCommand::Contexts => {
            let raw_list = k8s_cluster::k8s_contexts().await.map_err(|e| e.to_string())?;
            let current = k8s_cluster::k8s_current_context()
                .await
                .unwrap_or_default()
                .trim()
                .to_string();
            let contexts: Vec<String> = raw_list
                .lines()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .collect();
            if use_json {
                print_json_ok(&json!({ "current": current, "contexts": contexts }));
            } else {
                for ctx in &contexts {
                    let marker = if *ctx == current { "*" } else { " " };
                    println!("{marker} {ctx}");
                }
            }
            Ok(())
        }
        K8sCommand::UseContext { ctx } => {
            let msg = if let Some(c) = client {
                let body = json!({ "context": ctx });
                match c
                    .post_json("/api/k8s/contexts/set", &[], &body, Duration::from_secs(10))
                    .await
                {
                    HttpOutcome::Ok(v) => v.as_str().unwrap_or("Context updated").to_string(),
                    HttpOutcome::ApiErr(e) => return Err(e),
                    HttpOutcome::Unavailable(_) => k8s_cluster::k8s_set_context(ctx)
                        .await
                        .map_err(|e| e.to_string())?,
                }
            } else {
                k8s_cluster::k8s_set_context(ctx)
                    .await
                    .map_err(|e| e.to_string())?
            };
            emit_message(msg.trim(), use_json);
            Ok(())
        }
        K8sCommand::Namespaces => {
            let list = kubernetes::k8s_namespaces()
                .await
                .map_err(|e| e.to_string())?;
            if use_json {
                print_json_ok(&list);
            } else {
                println!("{:<28} {:<14} AGE", "NAME", "STATUS");
                for ns in &list {
                    println!("{:<28} {:<14} {}", truncate(&ns.name, 27), ns.status, ns.age);
                }
            }
            Ok(())
        }
        K8sCommand::Pods { namespace } => {
            let pods = kubernetes::k8s_pods(namespace)
                .await
                .map_err(|e| e.to_string())?;
            if use_json {
                print_json_ok(&pods);
            } else {
                println!(
                    "{:<18} {:<32} {:<8} {:<14} {:<10} AGE",
                    "NAMESPACE", "NAME", "READY", "STATUS", "RESTARTS"
                );
                for p in &pods {
                    println!(
                        "{:<18} {:<32} {:<8} {:<14} {:<10} {}",
                        truncate(&p.namespace, 17),
                        truncate(&p.name, 31),
                        p.ready,
                        truncate(&p.status, 13),
                        p.restarts,
                        p.age
                    );
                }
            }
            Ok(())
        }
        K8sCommand::Deployments { namespace } => {
            let deps = kubernetes::k8s_deployments(namespace)
                .await
                .map_err(|e| e.to_string())?;
            if use_json {
                print_json_ok(&deps);
            } else {
                println!(
                    "{:<18} {:<32} {:<10} {:<10} AGE",
                    "NAMESPACE", "NAME", "READY", "AVAILABLE"
                );
                for d in &deps {
                    println!(
                        "{:<18} {:<32} {:<10} {:<10} {}",
                        truncate(&d.namespace, 17),
                        truncate(&d.name, 31),
                        d.ready,
                        d.available,
                        d.age
                    );
                }
            }
            Ok(())
        }
        K8sCommand::Services { namespace } => {
            let svcs = kubernetes::k8s_services(namespace)
                .await
                .map_err(|e| e.to_string())?;
            if use_json {
                print_json_ok(&svcs);
            } else {
                println!(
                    "{:<18} {:<26} {:<14} {:<18} PORTS",
                    "NAMESPACE", "NAME", "TYPE", "CLUSTER-IP"
                );
                for s in &svcs {
                    println!(
                        "{:<18} {:<26} {:<14} {:<18} {}",
                        truncate(&s.namespace, 17),
                        truncate(&s.name, 25),
                        s.svc_type,
                        s.cluster_ip,
                        s.ports
                    );
                }
            }
            Ok(())
        }
        K8sCommand::Nodes => {
            if use_json {
                let raw = k8s_cluster::k8s_nodes_json()
                    .await
                    .map_err(|e| e.to_string())?;
                print_json_ok(&try_parse_json_str(Value::String(raw)));
            } else {
                let out = kubernetes::k8s_nodes().await.map_err(|e| e.to_string())?;
                print!("{out}");
            }
            Ok(())
        }
        K8sCommand::Events { namespace } => {
            if use_json {
                let raw = k8s_cluster::k8s_events_json(namespace)
                    .await
                    .map_err(|e| e.to_string())?;
                print_json_ok(&try_parse_json_str(Value::String(raw)));
            } else {
                let out = kubernetes::k8s_events(namespace)
                    .await
                    .map_err(|e| e.to_string())?;
                print!("{out}");
            }
            Ok(())
        }
        K8sCommand::Resources { kind, namespace } => {
            let raw = kubernetes::k8s_resources(kind, namespace)
                .await
                .map_err(|e| e.to_string())?;
            let parsed = try_parse_json_str(Value::String(raw));
            if use_json {
                print_json_ok(&parsed);
            } else {
                print_human_value(&parsed);
            }
            Ok(())
        }
        K8sCommand::Logs {
            pod,
            namespace,
            tail,
        } => {
            let logs = kubernetes::k8s_pod_logs(namespace, pod, tail)
                .await
                .map_err(|e| e.to_string())?;
            emit_raw_or_json(&logs, "logs", use_json);
            Ok(())
        }
        K8sCommand::Describe {
            kind,
            name,
            namespace,
        } => {
            let desc = kubernetes::k8s_describe(namespace, kind, name)
                .await
                .map_err(|e| e.to_string())?;
            emit_raw_or_json(&desc, "description", use_json);
            Ok(())
        }
        K8sCommand::Yaml {
            kind,
            name,
            namespace,
        } => {
            let yaml = k8s_resources::k8s_yaml(kind, namespace, name)
                .await
                .map_err(|e| e.to_string())?;
            emit_raw_or_json(&yaml, "yaml", use_json);
            Ok(())
        }
        K8sCommand::Apply {
            yaml_file,
            namespace,
        } => {
            let path = yaml_file.strip_prefix('@').unwrap_or(&yaml_file);
            let yaml_content = fs::read_to_string(path)
                .map_err(|e| format!("Failed to read manifest '{path}': {e}"))?;
            let msg = if let Some(c) = client {
                let body = json!({ "yaml": yaml_content, "namespace": namespace });
                match c
                    .post_json("/api/k8s/apply", &[], &body, Duration::from_secs(30))
                    .await
                {
                    HttpOutcome::Ok(v) => v.as_str().unwrap_or("Applied").to_string(),
                    HttpOutcome::ApiErr(e) => return Err(e),
                    HttpOutcome::Unavailable(_) => {
                        k8s_resources::k8s_apply(yaml_content, namespace)
                            .await
                            .map_err(|e| e.to_string())?
                    }
                }
            } else {
                k8s_resources::k8s_apply(yaml_content, namespace)
                    .await
                    .map_err(|e| e.to_string())?
            };
            emit_message(msg.trim(), use_json);
            Ok(())
        }
        K8sCommand::Scale {
            deployment,
            replicas,
            namespace,
        } => {
            let msg = if let Some(c) = client {
                let body = json!({
                    "namespace": namespace,
                    "deployment": deployment,
                    "replicas": replicas,
                });
                match c
                    .post_json("/api/k8s/scale", &[], &body, Duration::from_secs(20))
                    .await
                {
                    HttpOutcome::Ok(v) => v.as_str().unwrap_or("Scaled").to_string(),
                    HttpOutcome::ApiErr(e) => return Err(e),
                    HttpOutcome::Unavailable(_) => {
                        kubernetes::k8s_scale(namespace, deployment, replicas)
                            .await
                            .map_err(|e| e.to_string())?
                    }
                }
            } else {
                kubernetes::k8s_scale(namespace, deployment, replicas)
                    .await
                    .map_err(|e| e.to_string())?
            };
            emit_message(msg.trim(), use_json);
            Ok(())
        }
        K8sCommand::Restart {
            kind,
            name,
            namespace,
        } => {
            let msg = k8s_resources::k8s_restart(kind, namespace, name)
                .await
                .map_err(|e| e.to_string())?;
            emit_message(msg.trim(), use_json);
            Ok(())
        }
        K8sCommand::Delete {
            kind,
            name,
            namespace,
        } => {
            let plural = kubernetes::normalize_listable_resource(&kind);
            let msg = k8s_resources::k8s_delete_resource(plural, namespace, name)
                .await
                .map_err(|e| e.to_string())?;
            emit_message(msg.trim(), use_json);
            Ok(())
        }
        K8sCommand::PortForward { action } => match action.unwrap_or(PortForwardAction::List) {
            PortForwardAction::List => {
                let raw = if let Some(c) = client {
                    match c
                        .get("/api/k8s/port-forward/list", &[], Duration::from_secs(10))
                        .await
                    {
                        HttpOutcome::Ok(v) => v.as_str().unwrap_or_default().to_string(),
                        HttpOutcome::ApiErr(e) => return Err(e),
                        HttpOutcome::Unavailable(_) => {
                            k8s_cluster::k8s_pf_list().await.map_err(|e| e.to_string())?
                        }
                    }
                } else {
                    k8s_cluster::k8s_pf_list().await.map_err(|e| e.to_string())?
                };
                let items: Vec<Value> = raw
                    .lines()
                    .filter(|l| !l.trim().is_empty())
                    .map(|line| {
                        let (port, pid) = line.split_once(':').unwrap_or((line, ""));
                        json!({ "local_port": port, "pid": pid })
                    })
                    .collect();
                if use_json {
                    print_json_ok(&items);
                } else if items.is_empty() {
                    println!("No active port-forwards.");
                } else {
                    println!("{:<14} PID", "LOCAL PORT");
                    for it in &items {
                        println!(
                            "{:<14} {}",
                            it["local_port"].as_str().unwrap_or("-"),
                            it["pid"].as_str().unwrap_or("-")
                        );
                    }
                }
                Ok(())
            }
            PortForwardAction::Start {
                target,
                local_port,
                remote_port,
                namespace,
            } => {
                let (resource_type, name) = match target.split_once('/') {
                    Some((rt, n)) => (rt.to_string(), n.to_string()),
                    None => ("pod".to_string(), target),
                };
                let msg = if let Some(c) = client {
                    let body = json!({
                        "namespace": namespace,
                        "resourceType": resource_type,
                        "name": name,
                        "localPort": local_port,
                        "remotePort": remote_port,
                    });
                    match c
                        .post_json(
                            "/api/k8s/port-forward/start",
                            &[],
                            &body,
                            Duration::from_secs(15),
                        )
                        .await
                    {
                        HttpOutcome::Ok(v) => v.as_str().unwrap_or("Started").to_string(),
                        HttpOutcome::ApiErr(e) => return Err(e),
                        HttpOutcome::Unavailable(_) => k8s_cluster::k8s_pf_start(
                            namespace,
                            name,
                            u32::from(local_port),
                            u32::from(remote_port),
                            resource_type,
                        )
                        .await
                        .map_err(|e| e.to_string())?,
                    }
                } else {
                    k8s_cluster::k8s_pf_start(
                        namespace,
                        name,
                        u32::from(local_port),
                        u32::from(remote_port),
                        resource_type,
                    )
                    .await
                    .map_err(|e| e.to_string())?
                };
                emit_message(&msg, use_json);
                Ok(())
            }
            PortForwardAction::Stop { local_port } => {
                let msg = if let Some(c) = client {
                    let body = json!({ "localPort": local_port });
                    match c
                        .post_json(
                            "/api/k8s/port-forward/stop",
                            &[],
                            &body,
                            Duration::from_secs(10),
                        )
                        .await
                    {
                        HttpOutcome::Ok(v) => v.as_str().unwrap_or("Stopped").to_string(),
                        HttpOutcome::ApiErr(e) => return Err(e),
                        HttpOutcome::Unavailable(_) => k8s_cluster::k8s_pf_stop(u32::from(local_port))
                            .await
                            .map_err(|e| e.to_string())?,
                    }
                } else {
                    k8s_cluster::k8s_pf_stop(u32::from(local_port))
                        .await
                        .map_err(|e| e.to_string())?
                };
                emit_message(&msg, use_json);
                Ok(())
            }
        },
        K8sCommand::Exec {
            pod,
            namespace,
            container,
            cmd,
        } => {
            if !crate::validation::is_valid_k8s_name(&namespace) {
                return Err(format!("Invalid namespace: {namespace}"));
            }
            if !crate::validation::is_valid_k8s_name(&pod) {
                return Err(format!("Invalid pod name: {pod}"));
            }
            if !container.is_empty() && !crate::validation::is_valid_k8s_name(&container) {
                return Err(format!("Invalid container name: {container}"));
            }
            let out = crate::helpers::run_blocking(move || {
                let mut argv = vec!["exec".to_string(), "-n".to_string(), namespace, pod];
                if !container.is_empty() {
                    argv.push("-c".to_string());
                    argv.push(container);
                }
                argv.push("--".to_string());
                argv.extend(cmd);
                let refs: Vec<&str> = argv.iter().map(String::as_str).collect();
                crate::helpers::run_cmd("kubectl", &refs)
            })
            .await?;
            emit_raw_or_json(&out, "output", use_json);
            Ok(())
        }
        K8sCommand::Crds => {
            let raw = k8s_resources::k8s_crds().await.map_err(|e| e.to_string())?;
            let parsed = try_parse_json_str(Value::String(raw));
            if use_json {
                print_json_ok(&parsed);
            } else if let Some(items) = parsed.get("items").and_then(Value::as_array) {
                println!("{:<48} GROUP", "CRD NAME");
                for item in items {
                    let name = item
                        .pointer("/metadata/name")
                        .and_then(Value::as_str)
                        .unwrap_or("-");
                    let grp = item
                        .pointer("/spec/group")
                        .and_then(Value::as_str)
                        .unwrap_or("-");
                    println!("{:<48} {}", truncate(name, 47), grp);
                }
            } else {
                print_human_value(&parsed);
            }
            Ok(())
        }
        K8sCommand::Passthrough(args) => {
            crate::services::orchestration::OrchestrationService::auto_detect()
                .passthrough(&args)
                .await
                .map_err(|e| e.to_string())
        }
    }
}

// ============================================================================
// 10. Kind Handler
// ============================================================================

pub async fn handle_kind(
    action: KindCommand,
    client: Option<&DesktopClient>,
    use_json: bool,
) -> Result<(), String> {
    match action {
        KindCommand::List => {
            let raw = if let Some(c) = client {
                match c.get("/api/kind", &[], Duration::from_secs(15)).await {
                    HttpOutcome::Ok(v) => v.as_str().unwrap_or_default().to_string(),
                    HttpOutcome::ApiErr(e) => return Err(e),
                    HttpOutcome::Unavailable(_) => {
                        kind::kind_list().await.map_err(|e| e.to_string())?
                    }
                }
            } else {
                kind::kind_list().await.map_err(|e| e.to_string())?
            };
            let clusters: Vec<String> = raw
                .lines()
                .map(str::trim)
                .filter(|l| !l.is_empty() && *l != "No kind clusters found.")
                .map(str::to_string)
                .collect();
            if use_json {
                print_json_ok(&clusters);
            } else if clusters.is_empty() {
                println!("No kind clusters found.");
            } else {
                for cl in &clusters {
                    println!("{cl}");
                }
            }
            Ok(())
        }
        KindCommand::Create { name, image } => {
            let msg = if let Some(c) = client {
                let body = json!({ "name": name, "image": image });
                match c
                    .post_json("/api/kind/create", &[], &body, Duration::from_secs(300))
                    .await
                {
                    HttpOutcome::Ok(v) => v.as_str().unwrap_or("Cluster created").to_string(),
                    HttpOutcome::ApiErr(e) => return Err(e),
                    HttpOutcome::Unavailable(_) => kind::kind_create(name, image)
                        .await
                        .map_err(|e| e.to_string())?,
                }
            } else {
                kind::kind_create(name, image)
                    .await
                    .map_err(|e| e.to_string())?
            };
            emit_message(&msg, use_json);
            Ok(())
        }
        KindCommand::Delete { name } => {
            let msg = if let Some(c) = client {
                let body = json!({ "name": name });
                match c
                    .post_json("/api/kind/delete", &[], &body, Duration::from_secs(90))
                    .await
                {
                    HttpOutcome::Ok(v) => v.as_str().unwrap_or("Cluster deleted").to_string(),
                    HttpOutcome::ApiErr(e) => return Err(e),
                    HttpOutcome::Unavailable(_) => {
                        kind::kind_delete(name).await.map_err(|e| e.to_string())?
                    }
                }
            } else {
                kind::kind_delete(name).await.map_err(|e| e.to_string())?
            };
            emit_message(&msg, use_json);
            Ok(())
        }
    }
}

// ============================================================================
// 11. Lima Handler
// ============================================================================

pub async fn handle_lima(
    action: LimaCommand,
    client: Option<&DesktopClient>,
    use_json: bool,
) -> Result<(), String> {
    match action {
        LimaCommand::List => {
            let val = if let Some(c) = client {
                match c.get("/api/lima", &[], Duration::from_secs(15)).await {
                    HttpOutcome::Ok(v) => v,
                    HttpOutcome::ApiErr(e) => return Err(e),
                    HttpOutcome::Unavailable(_) => {
                        let vms = lima::lima_list().await.map_err(|e| e.to_string())?;
                        serde_json::to_value(vms).map_err(|e| e.to_string())?
                    }
                }
            } else {
                let vms = lima::lima_list().await.map_err(|e| e.to_string())?;
                serde_json::to_value(vms).map_err(|e| e.to_string())?
            };

            if use_json {
                print_json_ok(&val);
            } else if let Some(arr) = val.as_array() {
                if arr.is_empty() {
                    println!("No Lima instances found.");
                } else {
                    println!("{:<22} {:<14} {:<10} {:<6} DIR", "NAME", "STATUS", "ARCH", "CPUS");
                    for item in arr {
                        let name = item.get("name").and_then(Value::as_str).unwrap_or("-");
                        let status = item.get("status").and_then(Value::as_str).unwrap_or("-");
                        let arch = item.get("arch").and_then(Value::as_str).unwrap_or("-");
                        let cpus = item.get("cpus").and_then(Value::as_u64).unwrap_or(0);
                        let dir = item.get("dir").and_then(Value::as_str).unwrap_or("");
                        println!(
                            "{:<22} {:<14} {:<10} {:<6} {}",
                            truncate(name, 21),
                            status,
                            arch,
                            cpus,
                            dir
                        );
                    }
                }
            } else {
                print_human_value(&val);
            }
            Ok(())
        }
        LimaCommand::Info { name } => {
            let raw = lima::lima_info(name).await.map_err(|e| e.to_string())?;
            let parsed = try_parse_json_str(Value::String(raw));
            if use_json {
                print_json_ok(&parsed);
            } else {
                print_human_value(&parsed);
            }
            Ok(())
        }
        LimaCommand::Templates => {
            let raw = if let Some(c) = client {
                match c
                    .get("/api/lima/templates", &[], Duration::from_secs(15))
                    .await
                {
                    HttpOutcome::Ok(v) => v.as_str().unwrap_or_default().to_string(),
                    HttpOutcome::ApiErr(e) => return Err(e),
                    HttpOutcome::Unavailable(_) => {
                        lima::lima_templates().await.map_err(|e| e.to_string())?
                    }
                }
            } else {
                lima::lima_templates().await.map_err(|e| e.to_string())?
            };
            let list: Vec<String> = raw
                .lines()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .collect();
            if use_json {
                print_json_ok(&list);
            } else {
                for tpl in &list {
                    println!("{tpl}");
                }
            }
            Ok(())
        }
        LimaCommand::Create {
            name,
            template,
            cpus,
            memory,
            disk,
        } => {
            let msg = if let Some(c) = client {
                let body = json!({
                    "name": name,
                    "template": template,
                    "cpus": cpus,
                    "memory": memory,
                    "disk": disk,
                });
                match c
                    .post_json("/api/lima/create", &[], &body, Duration::from_secs(300))
                    .await
                {
                    HttpOutcome::Ok(v) => v.as_str().unwrap_or("Created").to_string(),
                    HttpOutcome::ApiErr(e) => return Err(e),
                    HttpOutcome::Unavailable(_) => {
                        lima::lima_create(name, template, cpus, memory, disk)
                            .await
                            .map_err(|e| e.to_string())?
                    }
                }
            } else {
                lima::lima_create(name, template, cpus, memory, disk)
                    .await
                    .map_err(|e| e.to_string())?
            };
            emit_message(&msg, use_json);
            Ok(())
        }
        LimaCommand::Start { name } => {
            let msg = if let Some(c) = client {
                let body = json!({ "name": name });
                match c
                    .post_json("/api/lima/start", &[], &body, Duration::from_secs(240))
                    .await
                {
                    HttpOutcome::Ok(v) => v.as_str().unwrap_or("Started").to_string(),
                    HttpOutcome::ApiErr(e) => return Err(e),
                    HttpOutcome::Unavailable(_) => {
                        lima::lima_start(name).await.map_err(|e| e.to_string())?
                    }
                }
            } else {
                lima::lima_start(name).await.map_err(|e| e.to_string())?
            };
            emit_message(&msg, use_json);
            Ok(())
        }
        LimaCommand::Stop { name } => {
            let msg = if let Some(c) = client {
                let body = json!({ "name": name });
                match c
                    .post_json("/api/lima/stop", &[], &body, Duration::from_secs(90))
                    .await
                {
                    HttpOutcome::Ok(v) => v.as_str().unwrap_or("Stopped").to_string(),
                    HttpOutcome::ApiErr(e) => return Err(e),
                    HttpOutcome::Unavailable(_) => {
                        lima::lima_stop(name).await.map_err(|e| e.to_string())?
                    }
                }
            } else {
                lima::lima_stop(name).await.map_err(|e| e.to_string())?
            };
            emit_message(&msg, use_json);
            Ok(())
        }
        LimaCommand::Delete { name, force } => {
            let msg = if let Some(c) = client {
                let body = json!({ "name": name, "force": force });
                match c
                    .post_json("/api/lima/delete", &[], &body, Duration::from_secs(90))
                    .await
                {
                    HttpOutcome::Ok(v) => v.as_str().unwrap_or("Deleted").to_string(),
                    HttpOutcome::ApiErr(e) => return Err(e),
                    HttpOutcome::Unavailable(_) => lima::lima_delete(name, force)
                        .await
                        .map_err(|e| e.to_string())?,
                }
            } else {
                lima::lima_delete(name, force)
                    .await
                    .map_err(|e| e.to_string())?
            };
            emit_message(&msg, use_json);
            Ok(())
        }
        LimaCommand::Shell { name, cmd } => {
            let command_str = cmd.join(" ");
            let out = if let Some(c) = client {
                let body = json!({ "name": name, "command": command_str });
                match c
                    .post_json("/api/lima/shell", &[], &body, Duration::from_secs(60))
                    .await
                {
                    HttpOutcome::Ok(v) => v.as_str().unwrap_or_default().to_string(),
                    HttpOutcome::ApiErr(e) => return Err(e),
                    HttpOutcome::Unavailable(_) => lima::lima_shell(name, command_str)
                        .await
                        .map_err(|e| e.to_string())?,
                }
            } else {
                lima::lima_shell(name, command_str)
                    .await
                    .map_err(|e| e.to_string())?
            };
            emit_raw_or_json(&out, "output", use_json);
            Ok(())
        }
        LimaCommand::Passthrough(args) => {
            use crate::adapters::traits::VMManager;
            crate::adapters::lima::LimaAdapter::new()
                .passthrough(&args)
                .await
                .map_err(|e| e.to_string())
        }
    }
}

// ============================================================================
// Legacy System & Status Handlers
// ============================================================================

pub async fn handle_system(
    action: SystemCommand,
    client: Option<&DesktopClient>,
    use_json: bool,
) -> Result<(), String> {
    match action {
        SystemCommand::Check => {
            let val = if let Some(c) = client {
                match c.get("/api/system/check", &[], Duration::from_secs(15)).await {
                    HttpOutcome::Ok(v) => v,
                    HttpOutcome::ApiErr(e) => return Err(e),
                    HttpOutcome::Unavailable(_) => {
                        let info = system::check_system().await.map_err(|e| e.to_string())?;
                        serde_json::to_value(info).map_err(|e| e.to_string())?
                    }
                }
            } else {
                let info = system::check_system().await.map_err(|e| e.to_string())?;
                serde_json::to_value(info).map_err(|e| e.to_string())?
            };
            if use_json {
                print_json_ok(&val);
            } else {
                print_human_value(&val);
            }
            Ok(())
        }
        SystemCommand::Df => {
            let out = if let Some(c) = client {
                match c.get("/api/system/df", &[], Duration::from_secs(15)).await {
                    HttpOutcome::Ok(v) => v.as_str().unwrap_or_default().to_string(),
                    HttpOutcome::ApiErr(e) => return Err(e),
                    HttpOutcome::Unavailable(_) => {
                        containers::system_df().await.map_err(|e| e.to_string())?
                    }
                }
            } else {
                containers::system_df().await.map_err(|e| e.to_string())?
            };
            emit_raw_or_json(&out, "df", use_json);
            Ok(())
        }
        SystemCommand::Prune { all } => {
            let all_str = if all { "true" } else { "false" };
            let msg = if let Some(c) = client {
                match c
                    .post_empty(
                        "/api/system/prune",
                        &[("all", all_str)],
                        Duration::from_secs(120),
                    )
                    .await
                {
                    HttpOutcome::Ok(v) => v.as_str().unwrap_or("Pruned").to_string(),
                    HttpOutcome::ApiErr(e) => return Err(e),
                    HttpOutcome::Unavailable(_) => containers::system_prune(all)
                        .await
                        .map_err(|e| e.to_string())?,
                }
            } else {
                containers::system_prune(all)
                    .await
                    .map_err(|e| e.to_string())?
            };
            emit_message(&msg, use_json);
            Ok(())
        }
        SystemCommand::Specs => {
            let val = if let Some(c) = client {
                match c
                    .get("/api/system/host-specs", &[], Duration::from_secs(10))
                    .await
                {
                    HttpOutcome::Ok(v) => v,
                    HttpOutcome::ApiErr(e) => return Err(e),
                    HttpOutcome::Unavailable(_) => {
                        let specs = tokio::task::spawn_blocking(system::detect_host_specs)
                            .await
                            .map_err(|e| format!("Task join error: {e}"))?;
                        serde_json::to_value(specs).map_err(|e| e.to_string())?
                    }
                }
            } else {
                let specs = tokio::task::spawn_blocking(system::detect_host_specs)
                    .await
                    .map_err(|e| format!("Task join error: {e}"))?;
                serde_json::to_value(specs).map_err(|e| e.to_string())?
            };
            if use_json {
                print_json_ok(&val);
            } else {
                print_human_value(&val);
            }
            Ok(())
        }
        SystemCommand::Info => {
            let svc = ContainerService::auto_detect();
            let rt_name = svc.runtime_name();
            if use_json {
                print_json_ok(&json!({
                    "version": env!("CARGO_PKG_VERSION"),
                    "runtime": rt_name,
                }));
            } else {
                println!("ColimaUI v{}", env!("CARGO_PKG_VERSION"));
                println!("Runtime: {rt_name}");
            }
            Ok(())
        }
    }
}

pub async fn handle_status(client: Option<&DesktopClient>, use_json: bool) -> Result<(), String> {
    let vms = if let Some(c) = client {
        match c.get("/api/instances", &[], Duration::from_secs(10)).await {
            HttpOutcome::Ok(v) => serde_json::from_value::<Vec<colima::ColimaInstance>>(v)
                .unwrap_or_else(|_| crate::instance_reader::list_instances_fast()),
            _ => crate::instance_reader::list_instances_fast(),
        }
    } else {
        crate::instance_reader::list_instances_fast()
    };

    let any_vm_running = vms
        .iter()
        .any(|vm| vm.status.eq_ignore_ascii_case("running"));
    let containers_res = ContainerService::auto_detect().list_containers(true).await;

    if use_json {
        let mut status = serde_json::Map::new();
        status.insert("vms".into(), serde_json::to_value(&vms).unwrap_or_default());
        match containers_res {
            Ok(list) => {
                status.insert(
                    "containers".into(),
                    serde_json::to_value(&list).unwrap_or_default(),
                );
            }
            Err(e) if any_vm_running => return Err(e.to_string()),
            Err(_) => {
                status.insert("containers".into(), Value::Array(Vec::new()));
            }
        }
        print_json_ok(&status);
    } else {
        println!("=== ColimaUI Status ===\n");
        println!("--- VM Instances ---");
        if vms.is_empty() {
            println!("  No instances found");
        } else {
            for vm in &vms {
                println!(
                    "  {} [{}] — {} cpu, {} mem, {}",
                    vm.name,
                    vm.status,
                    vm.cpus,
                    format_bytes(vm.memory),
                    vm.runtime
                );
            }
        }

        println!("\n--- Containers ---");
        match containers_res {
            Ok(list) if !list.is_empty() => {
                let running = list.iter().filter(|c| c.state == "running").count();
                let stopped = list.len() - running;
                println!("  {running} running, {stopped} stopped");
            }
            Ok(_) => println!("  No containers"),
            Err(e) if any_vm_running => return Err(e.to_string()),
            Err(_) => println!("  Runtime not running"),
        }
    }
    Ok(())
}

fn emit_message(msg: &str, use_json: bool) {
    if use_json {
        print_json_ok(&json!({ "message": msg }));
    } else {
        println!("{msg}");
    }
}

fn emit_raw_or_json(raw: &str, field_name: &str, use_json: bool) {
    if use_json {
        let mut map = serde_json::Map::new();
        map.insert(field_name.to_string(), Value::String(raw.to_string()));
        print_json_ok(&map);
    } else {
        print!("{raw}");
        if !raw.is_empty() && !raw.ends_with('\n') {
            println!();
        }
    }
}
