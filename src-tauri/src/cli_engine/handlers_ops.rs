//! Dual-transport (`HttpOrDirect`) command handlers for Security, AI Models,
//! Alerts, Self-Healing, Knowledge Bank (`memories`), Chat `sessions`, Desktop `ui`,
//! SSE `events`, `doctor`, Headless AI `chat`, and `mcp`.

use std::collections::HashMap;
use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, OpenFlags};
use serde_json::{json, Value};

use crate::commands::{
    agent_loop::{self, HeadlessChatRequest},
    ai_chat, alerts, detonation, falco_bridge, kb_articles, knowledge_bank, models, resolution,
    security_catalog, security_history, security_rules, security_scan, security_triage, self_heal,
};

use super::args::{
    AlertsCommand, ChatArgs, EventsArgs, MemoriesCommand, ModelsCommand, SecurityCommand,
    SelfHealCommand, SessionsCommand, UiCommand,
};
use super::client::{
    inspect_credentials, probe_active_port, DesktopClient, HttpOutcome,
};
use super::ensure_kb_init;
use super::output::{
    extract_suggestions, print_human_value, print_json_ok, truncate, I18n,
};

// ============================================================================
// Shared Security Helpers (reused by `mcp.rs`)
// ============================================================================

pub(crate) fn fresh_scan_id(prefix: &str) -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis());
    format!("{prefix}-{}-{millis}", std::process::id())
}

pub(crate) fn parse_sbom_format(raw: &str) -> Result<security_scan::SbomFormat, String> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "cyclonedx" | "cyclonedx-json" | "cyclone_dx" => Ok(security_scan::SbomFormat::CycloneDx),
        "spdx" | "spdx-json" => Ok(security_scan::SbomFormat::Spdx),
        other => Err(format!(
            "Unsupported SBOM format '{other}'. Expected 'cyclonedx-json' or 'spdx-json'."
        )),
    }
}

/// Export an SBOM either to a caller-supplied `output_path` or to a temporary file
/// whose JSON contents are parsed, returned, and cleaned up automatically.
pub(crate) async fn run_sbom_export(
    image: &str,
    format: security_scan::SbomFormat,
    output_path: Option<&str>,
    client: Option<&DesktopClient>,
) -> Result<Value, String> {
    let (dest_dir, file_name, is_temp) = if let Some(out_str) = output_path {
        let out_path = Path::new(out_str);
        let parent = out_path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let canonical_parent = if parent.is_relative() {
            std::env::current_dir()
                .map_err(|e| e.to_string())?
                .join(parent)
        } else {
            parent.to_path_buf()
        };
        let fname = out_path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| format!("Invalid output file path: {out_str}"))?
            .to_string();
        (canonical_parent.to_string_lossy().to_string(), fname, false)
    } else {
        let tmp_dir = std::env::temp_dir().to_string_lossy().to_string();
        let fname = format!("{}.sbom.json", fresh_scan_id("cui-sbom"));
        (tmp_dir, fname, true)
    };

    let written_path = if let Some(c) = client {
        let body = json!({
            "imageRef": image,
            "destDir": dest_dir,
            "fileName": file_name,
            "format": format,
            "overwrite": true,
        });
        match c
            .post_json("/api/security/sbom", &[], &body, Duration::from_secs(300))
            .await
        {
            HttpOutcome::Ok(v) => v.as_str().map_or_else(
                || Path::new(&dest_dir).join(&file_name).to_string_lossy().to_string(),
                str::to_string,
            ),
            HttpOutcome::ApiErr(e) => return Err(e),
            HttpOutcome::Unavailable(_) => {
                let img = image.to_string();
                let dd = dest_dir.clone();
                let fnm = file_name.clone();
                crate::helpers::run_blocking(move || {
                    security_scan::export_sbom_blocking(&img, &dd, &fnm, format, true)
                })
                .await?
            }
        }
    } else {
        let img = image.to_string();
        let dd = dest_dir.clone();
        let fnm = file_name.clone();
        crate::helpers::run_blocking(move || {
            security_scan::export_sbom_blocking(&img, &dd, &fnm, format, true)
        })
        .await?
    };

    if is_temp {
        let raw_content = fs::read_to_string(&written_path)
            .map_err(|e| format!("Failed reading generated SBOM: {e}"));
        let _ = fs::remove_file(&written_path);
        let text = raw_content?;
        Ok(serde_json::from_str::<Value>(&text).unwrap_or(Value::String(text)))
    } else {
        Ok(json!({
            "image": image,
            "path": written_path,
        }))
    }
}

// ============================================================================
// 12. Security Handler
// ============================================================================

pub async fn handle_security(
    action: SecurityCommand,
    client: Option<&DesktopClient>,
    use_json: bool,
) -> Result<(), String> {
    match action {
        SecurityCommand::Scan { image, refresh } => {
            let scan_id = fresh_scan_id("cli-scan");
            let val = if let Some(c) = client {
                let body = json!({
                    "scanId": scan_id,
                    "imageRef": image,
                    "refresh": refresh,
                });
                match c
                    .post_json("/api/security/scan", &[], &body, Duration::from_secs(300))
                    .await
                {
                    HttpOutcome::Ok(v) => v,
                    HttpOutcome::ApiErr(e) => return Err(e),
                    HttpOutcome::Unavailable(_) => {
                        let res = security_scan::security_scan_image(scan_id, image, Some(refresh))
                            .await
                            .map_err(|e| e.to_string())?;
                        serde_json::to_value(res).map_err(|e| e.to_string())?
                    }
                }
            } else {
                let res = security_scan::security_scan_image(scan_id, image, Some(refresh))
                    .await
                    .map_err(|e| e.to_string())?;
                serde_json::to_value(res).map_err(|e| e.to_string())?
            };

            if use_json {
                print_json_ok(&val);
            } else {
                let img_ref = val.get("imageRef").and_then(Value::as_str).unwrap_or("-");
                let findings = val
                    .get("findings")
                    .and_then(Value::as_array)
                    .map_or(0, Vec::len);
                println!("Image:    {img_ref}");
                println!("Findings: {findings} vulnerabilities");
                if let Some(arr) = val
                    .get("findings")
                    .and_then(Value::as_array)
                    .filter(|a| !a.is_empty())
                {
                    println!("\n{:<18} {:<10} {:<24} {:<16} FIXED", "ID", "SEVERITY", "PACKAGE", "INSTALLED");
                    for f in arr.iter().take(30) {
                        println!(
                            "{:<18} {:<10} {:<24} {:<16} {}",
                            truncate(f.get("id").and_then(Value::as_str).unwrap_or("-"), 17),
                            f.get("severity").and_then(Value::as_str).unwrap_or("-"),
                            truncate(f.get("package").and_then(Value::as_str).unwrap_or("-"), 23),
                            truncate(
                                f.get("installedVersion")
                                    .and_then(Value::as_str)
                                    .unwrap_or("-"),
                                15
                            ),
                            f.get("fixedVersion").and_then(Value::as_str).unwrap_or("-")
                        );
                    }
                }
            }
            Ok(())
        }
        SecurityCommand::Sbom {
            image,
            format,
            output,
        } => {
            let fmt = parse_sbom_format(&format)?;
            let val = run_sbom_export(&image, fmt, output.as_deref(), client).await?;
            if use_json {
                print_json_ok(&val);
            } else {
                print_human_value(&val);
            }
            Ok(())
        }
        SecurityCommand::Audit { image, refresh } => {
            let audit_val = run_security_audit(&image, refresh, client).await?;
            if use_json {
                print_json_ok(&audit_val);
            } else {
                let score = audit_val
                    .pointer("/score/total")
                    .and_then(Value::as_u64)
                    .unwrap_or(0);
                let findings = audit_val
                    .pointer("/scan/findings")
                    .and_then(Value::as_array)
                    .map_or(0, Vec::len);
                println!("Image:    {image}");
                println!("Score:    {score}/100");
                println!("Findings: {findings}");
            }
            Ok(())
        }
        SecurityCommand::Triage { image } => {
            let audit_val = run_security_audit(&image, false, client).await?;
            let audit: security_scan::SecurityAudit = serde_json::from_value(audit_val)
                .map_err(|e| format!("Failed parsing audit payload for triage: {e}"))?;
            let triaged = security_triage::triage(&audit, security_rules::Level::default());
            if use_json {
                print_json_ok(&triaged);
            } else {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&triaged).unwrap_or_default()
                );
            }
            Ok(())
        }
        SecurityCommand::History { digest, limit } => {
            let runs = match digest {
                Some(d) => crate::helpers::run_blocking(move || security_history::history(&d)).await?,
                None => crate::helpers::run_blocking(move || security_history::recent(limit)).await?,
            };
            if use_json {
                print_json_ok(&runs);
            } else if runs.is_empty() {
                println!("No recorded security scans.");
            } else {
                println!(
                    "{:<30} {:<7} {:<6} {:<6} {:<6} {:<6}",
                    "IMAGE", "SCORE", "CRIT", "HIGH", "MED", "LOW"
                );
                for r in &runs {
                    println!(
                        "{:<30} {:<7} {:<6} {:<6} {:<6} {:<6}",
                        truncate(&r.image_ref, 29),
                        r.score,
                        r.critical,
                        r.high,
                        r.medium,
                        r.low
                    );
                }
            }
            Ok(())
        }
        SecurityCommand::Rules => {
            let pack = security_rules::rule_pack();
            if use_json {
                print_json_ok(&pack);
            } else {
                println!("Rule Pack v{} ({} rules)", pack.pack_version, pack.rules.len());
                for r in &pack.rules {
                    println!("  - {:<12} {}", r.id, r.title);
                }
            }
            Ok(())
        }
        SecurityCommand::Alternatives { image } => {
            let val = if let Some(c) = client {
                match c
                    .get(
                        "/api/security/alternatives",
                        &[("image", image.as_str())],
                        Duration::from_secs(10),
                    )
                    .await
                {
                    HttpOutcome::Ok(v) => v,
                    HttpOutcome::ApiErr(e) => return Err(e),
                    HttpOutcome::Unavailable(_) => {
                        let sug = security_catalog::suggestions_for(&image);
                        serde_json::to_value(sug).map_err(|e| e.to_string())?
                    }
                }
            } else {
                let sug = security_catalog::suggestions_for(&image);
                serde_json::to_value(sug).map_err(|e| e.to_string())?
            };
            if use_json {
                print_json_ok(&val);
            } else {
                print_human_value(&val);
            }
            Ok(())
        }
        SecurityCommand::FalcoState => {
            let val = if let Some(c) = client {
                match c
                    .get("/api/security/falco/state", &[], Duration::from_secs(15))
                    .await
                {
                    HttpOutcome::Ok(v) => v,
                    HttpOutcome::ApiErr(e) => return Err(e),
                    HttpOutcome::Unavailable(_) => {
                        let st = crate::helpers::run_blocking(|| Ok(falco_bridge::detect())).await?;
                        serde_json::to_value(st).map_err(|e| e.to_string())?
                    }
                }
            } else {
                let st = crate::helpers::run_blocking(|| Ok(falco_bridge::detect())).await?;
                serde_json::to_value(st).map_err(|e| e.to_string())?
            };
            if use_json {
                print_json_ok(&val);
            } else {
                print_human_value(&val);
            }
            Ok(())
        }
        SecurityCommand::FalcoEvents { limit } => {
            let limit_str = limit.to_string();
            let val = if let Some(c) = client {
                match c
                    .get(
                        "/api/security/falco/events",
                        &[("limit", limit_str.as_str())],
                        Duration::from_secs(15),
                    )
                    .await
                {
                    HttpOutcome::Ok(v) => v,
                    HttpOutcome::ApiErr(e) => return Err(e),
                    HttpOutcome::Unavailable(_) => {
                        let evs =
                            crate::helpers::run_blocking(move || security_history::falco_events(limit))
                                .await?;
                        serde_json::to_value(evs).map_err(|e| e.to_string())?
                    }
                }
            } else {
                let evs =
                    crate::helpers::run_blocking(move || security_history::falco_events(limit)).await?;
                serde_json::to_value(evs).map_err(|e| e.to_string())?
            };
            if use_json {
                print_json_ok(&val);
            } else {
                print_human_value(&val);
            }
            Ok(())
        }
        SecurityCommand::DetonationList => {
            let val = if let Some(c) = client {
                match c
                    .get("/api/detonation/list", &[], Duration::from_secs(10))
                    .await
                {
                    HttpOutcome::Ok(v) => v,
                    HttpOutcome::ApiErr(e) => return Err(e),
                    HttpOutcome::Unavailable(_) => {
                        serde_json::to_value(detonation::list_sessions())
                            .map_err(|e| e.to_string())?
                    }
                }
            } else {
                serde_json::to_value(detonation::list_sessions()).map_err(|e| e.to_string())?
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

pub(crate) async fn run_security_audit(
    image: &str,
    refresh: bool,
    client: Option<&DesktopClient>,
) -> Result<Value, String> {
    let scan_id = fresh_scan_id("cli-audit");
    if let Some(c) = client {
        let body = json!({
            "scanId": scan_id,
            "imageRef": image,
            "refresh": refresh,
        });
        match c
            .post_json("/api/security/audit", &[], &body, Duration::from_secs(300))
            .await
        {
            HttpOutcome::Ok(v) => return Ok(v),
            HttpOutcome::ApiErr(e) => return Err(e),
            HttpOutcome::Unavailable(_) => {}
        }
    }
    let audit = security_scan::security_audit_image(
        scan_id,
        image.to_string(),
        Some(security_rules::Level::default()),
        Some(refresh),
    )
    .await
    .map_err(|e| e.to_string())?;
    serde_json::to_value(audit).map_err(|e| e.to_string())
}

// ============================================================================
// 13. Models Handler
// ============================================================================

pub async fn handle_models(
    action: ModelsCommand,
    client: Option<&DesktopClient>,
    use_json: bool,
) -> Result<(), String> {
    match action {
        ModelsCommand::List { profile } => {
            let val = if let Some(c) = client {
                match c
                    .get(
                        "/api/models",
                        &[("profile", profile.as_str())],
                        Duration::from_secs(20),
                    )
                    .await
                {
                    HttpOutcome::Ok(v) => v,
                    HttpOutcome::ApiErr(e) => return Err(e),
                    HttpOutcome::Unavailable(_) => {
                        let list = models::list_models(profile, None)
                            .await
                            .map_err(|e| e.to_string())?;
                        serde_json::to_value(list).map_err(|e| e.to_string())?
                    }
                }
            } else {
                let list = models::list_models(profile, None)
                    .await
                    .map_err(|e| e.to_string())?;
                serde_json::to_value(list).map_err(|e| e.to_string())?
            };

            if use_json {
                print_json_ok(&val);
            } else if let Some(arr) = val.as_array() {
                if arr.is_empty() {
                    println!("No AI models found.");
                } else {
                    println!("{:<36} {:<14} MODIFIED", "MODEL NAME", "SIZE");
                    for m in arr {
                        let name = m.get("name").and_then(Value::as_str).unwrap_or("-");
                        let size = m.get("size").and_then(Value::as_str).unwrap_or("-");
                        let mod_at = m.get("modified").and_then(Value::as_str).unwrap_or("");
                        println!("{:<36} {:<14} {}", truncate(name, 35), size, mod_at);
                    }
                }
            } else {
                print_human_value(&val);
            }
            Ok(())
        }
        ModelsCommand::Pull { model, profile } => {
            let msg = if let Some(c) = client {
                match c
                    .post_empty(
                        "/api/models/pull",
                        &[("profile", profile.as_str()), ("modelName", model.as_str())],
                        Duration::from_secs(600),
                    )
                    .await
                {
                    HttpOutcome::Ok(v) => v.as_str().unwrap_or("Model pulled").to_string(),
                    HttpOutcome::ApiErr(e) => return Err(e),
                    HttpOutcome::Unavailable(_) => models::pull_model(profile, model, None)
                        .await
                        .map_err(|e| e.to_string())?,
                }
            } else {
                models::pull_model(profile, model, None)
                    .await
                    .map_err(|e| e.to_string())?
            };
            if use_json {
                print_json_ok(&json!({ "message": msg }));
            } else {
                println!("{msg}");
            }
            Ok(())
        }
        ModelsCommand::Serve {
            model,
            profile,
            port,
        } => {
            let port_str = port.to_string();
            let msg = if let Some(c) = client {
                match c
                    .post_empty(
                        "/api/models/serve",
                        &[
                            ("profile", profile.as_str()),
                            ("modelName", model.as_str()),
                            ("port", port_str.as_str()),
                        ],
                        Duration::from_secs(60),
                    )
                    .await
                {
                    HttpOutcome::Ok(v) => v.as_str().unwrap_or("Serving model").to_string(),
                    HttpOutcome::ApiErr(e) => return Err(e),
                    HttpOutcome::Unavailable(_) => models::serve_model(profile, model, port, None)
                        .await
                        .map_err(|e| e.to_string())?,
                }
            } else {
                models::serve_model(profile, model, port, None)
                    .await
                    .map_err(|e| e.to_string())?
            };
            if use_json {
                print_json_ok(&json!({ "message": msg }));
            } else {
                println!("{msg}");
            }
            Ok(())
        }
        ModelsCommand::Delete { model, profile } => {
            let msg = if let Some(c) = client {
                match c
                    .post_empty(
                        "/api/models/delete",
                        &[("profile", profile.as_str()), ("modelName", model.as_str())],
                        Duration::from_secs(30),
                    )
                    .await
                {
                    HttpOutcome::Ok(v) => v.as_str().unwrap_or("Model deleted").to_string(),
                    HttpOutcome::ApiErr(e) => return Err(e),
                    HttpOutcome::Unavailable(_) => models::delete_model(profile, model, None)
                        .await
                        .map_err(|e| e.to_string())?,
                }
            } else {
                models::delete_model(profile, model, None)
                    .await
                    .map_err(|e| e.to_string())?
            };
            if use_json {
                print_json_ok(&json!({ "message": msg }));
            } else {
                println!("{msg}");
            }
            Ok(())
        }
    }
}

// ============================================================================
// 14. Alerts Handler
// ============================================================================

pub async fn handle_alerts(
    action: AlertsCommand,
    client: Option<&DesktopClient>,
    use_json: bool,
) -> Result<(), String> {
    match action {
        AlertsCommand::Rules => {
            let val = if let Some(c) = client {
                match c
                    .get("/api/alerts/rules", &[], Duration::from_secs(10))
                    .await
                {
                    HttpOutcome::Ok(v) => v,
                    HttpOutcome::ApiErr(e) => return Err(e),
                    HttpOutcome::Unavailable(_) => {
                        let rules = crate::helpers::run_blocking(alerts::list_rules).await?;
                        serde_json::to_value(rules).map_err(|e| e.to_string())?
                    }
                }
            } else {
                let rules = crate::helpers::run_blocking(alerts::list_rules).await?;
                serde_json::to_value(rules).map_err(|e| e.to_string())?
            };
            if use_json {
                print_json_ok(&val);
            } else {
                print_human_value(&val);
            }
            Ok(())
        }
        AlertsCommand::Events { limit } => {
            let limit_str = limit.to_string();
            let val = if let Some(c) = client {
                match c
                    .get(
                        "/api/alerts/events",
                        &[("limit", limit_str.as_str())],
                        Duration::from_secs(10),
                    )
                    .await
                {
                    HttpOutcome::Ok(v) => v,
                    HttpOutcome::ApiErr(e) => return Err(e),
                    HttpOutcome::Unavailable(_) => {
                        let evs =
                            crate::helpers::run_blocking(move || alerts::recent_events(limit)).await?;
                        serde_json::to_value(evs).map_err(|e| e.to_string())?
                    }
                }
            } else {
                let evs = crate::helpers::run_blocking(move || alerts::recent_events(limit)).await?;
                serde_json::to_value(evs).map_err(|e| e.to_string())?
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
// 15. Self-Heal Handler
// ============================================================================

pub async fn handle_self_heal(
    action: SelfHealCommand,
    client: Option<&DesktopClient>,
    use_json: bool,
) -> Result<(), String> {
    match action {
        SelfHealCommand::Status => {
            let enabled = if let Some(c) = client {
                match c
                    .get("/api/self-heal/enabled", &[], Duration::from_secs(10))
                    .await
                {
                    HttpOutcome::Ok(v) => v.as_bool().unwrap_or_else(self_heal::is_enabled),
                    HttpOutcome::ApiErr(e) => return Err(e),
                    HttpOutcome::Unavailable(_) => self_heal::is_enabled(),
                }
            } else {
                self_heal::is_enabled()
            };
            if use_json {
                print_json_ok(&json!({ "enabled": enabled }));
            } else {
                println!(
                    "Self-healing: {}",
                    if enabled { "enabled" } else { "disabled" }
                );
            }
            Ok(())
        }
        SelfHealCommand::Rules => {
            let val = if let Some(c) = client {
                match c
                    .get("/api/self-heal/rules", &[], Duration::from_secs(10))
                    .await
                {
                    HttpOutcome::Ok(v) => v,
                    HttpOutcome::ApiErr(e) => return Err(e),
                    HttpOutcome::Unavailable(_) => {
                        let rules = crate::helpers::run_blocking(self_heal::list_rules).await?;
                        serde_json::to_value(rules).map_err(|e| e.to_string())?
                    }
                }
            } else {
                let rules = crate::helpers::run_blocking(self_heal::list_rules).await?;
                serde_json::to_value(rules).map_err(|e| e.to_string())?
            };
            if use_json {
                print_json_ok(&val);
            } else {
                print_human_value(&val);
            }
            Ok(())
        }
        SelfHealCommand::Log { limit } => {
            let limit_str = limit.to_string();
            let val = if let Some(c) = client {
                match c
                    .get(
                        "/api/self-heal/log",
                        &[("limit", limit_str.as_str())],
                        Duration::from_secs(10),
                    )
                    .await
                {
                    HttpOutcome::Ok(v) => v,
                    HttpOutcome::ApiErr(e) => return Err(e),
                    HttpOutcome::Unavailable(_) => {
                        let entries =
                            crate::helpers::run_blocking(move || self_heal::recent_log(limit)).await?;
                        serde_json::to_value(entries).map_err(|e| e.to_string())?
                    }
                }
            } else {
                let entries =
                    crate::helpers::run_blocking(move || self_heal::recent_log(limit)).await?;
                serde_json::to_value(entries).map_err(|e| e.to_string())?
            };
            if use_json {
                print_json_ok(&val);
            } else {
                print_human_value(&val);
            }
            Ok(())
        }
        SelfHealCommand::Enable => set_self_heal_toggle(true, client, use_json).await,
        SelfHealCommand::Disable => set_self_heal_toggle(false, client, use_json).await,
    }
}

async fn set_self_heal_toggle(
    on: bool,
    client: Option<&DesktopClient>,
    use_json: bool,
) -> Result<(), String> {
    if let Some(c) = client {
        let body = json!({ "on": on });
        match c
            .post_json("/api/self-heal/enabled", &[], &body, Duration::from_secs(10))
            .await
        {
            HttpOutcome::Ok(_) => {
                if use_json {
                    print_json_ok(&json!({ "enabled": on }));
                } else {
                    println!(
                        "Self-healing {}",
                        if on { "enabled" } else { "disabled" }
                    );
                }
                return Ok(());
            }
            HttpOutcome::ApiErr(e) => return Err(e),
            HttpOutcome::Unavailable(_) => {}
        }
    }
    crate::helpers::run_blocking(move || self_heal::set_enabled(on)).await?;
    if use_json {
        print_json_ok(&json!({ "enabled": on }));
    } else {
        println!(
            "Self-healing {}",
            if on { "enabled" } else { "disabled" }
        );
    }
    Ok(())
}

// ============================================================================
// 16. Memories (Knowledge Bank) Handler
// ============================================================================

pub async fn handle_memories(
    action: MemoriesCommand,
    client: Option<&DesktopClient>,
    use_json: bool,
) -> Result<(), String> {
    ensure_kb_init();
    match action {
        MemoriesCommand::Query { text } => {
            let val = if let Some(c) = client {
                let body = json!({ "error_text": text });
                match c
                    .post_json("/api/kb/query", &[], &body, Duration::from_secs(15))
                    .await
                {
                    HttpOutcome::Ok(v) => v,
                    HttpOutcome::ApiErr(e) => return Err(e),
                    HttpOutcome::Unavailable(_) => {
                        let res = knowledge_bank::kb_query(text)
                            .await
                            .map_err(|e| e.to_string())?;
                        serde_json::to_value(res).map_err(|e| e.to_string())?
                    }
                }
            } else {
                let res = knowledge_bank::kb_query(text)
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
        MemoriesCommand::Search { text, limit } => {
            let val = if let Some(c) = client {
                let body = json!({ "query": text, "limit": limit });
                match c
                    .post_json("/api/kb/search", &[], &body, Duration::from_secs(10))
                    .await
                {
                    HttpOutcome::Ok(v) => v,
                    HttpOutcome::ApiErr(e) => return Err(e),
                    HttpOutcome::Unavailable(_) => {
                        let hits = knowledge_bank::search_memory(text, limit)
                            .await
                            .map_err(|e| e.to_string())?;
                        serde_json::to_value(hits).map_err(|e| e.to_string())?
                    }
                }
            } else {
                let hits = knowledge_bank::search_memory(text, limit)
                    .await
                    .map_err(|e| e.to_string())?;
                serde_json::to_value(hits).map_err(|e| e.to_string())?
            };
            if use_json {
                print_json_ok(&val);
            } else if let Some(arr) = val.as_array() {
                if arr.is_empty() {
                    println!("No matching memories.");
                } else {
                    for (idx, item) in arr.iter().enumerate() {
                        println!("{}. {}", idx + 1, item.as_str().unwrap_or(""));
                    }
                }
            } else {
                print_human_value(&val);
            }
            Ok(())
        }
        MemoriesCommand::List => {
            let val = if let Some(c) = client {
                match c
                    .get("/api/kb/memories", &[], Duration::from_secs(10))
                    .await
                {
                    HttpOutcome::Ok(v) => v,
                    HttpOutcome::ApiErr(e) => return Err(e),
                    HttpOutcome::Unavailable(_) => {
                        let items = knowledge_bank::get_all_memories()
                            .await
                            .map_err(|e| e.to_string())?;
                        serde_json::to_value(items).map_err(|e| e.to_string())?
                    }
                }
            } else {
                let items = knowledge_bank::get_all_memories()
                    .await
                    .map_err(|e| e.to_string())?;
                serde_json::to_value(items).map_err(|e| e.to_string())?
            };
            if use_json {
                print_json_ok(&val);
            } else if let Some(arr) = val.as_array() {
                if arr.is_empty() {
                    println!("No agent memories stored.");
                } else {
                    println!("{:<18} {:<14} CONTENT", "ID", "CATEGORY");
                    for m in arr {
                        println!(
                            "{:<18} {:<14} {}",
                            m.get("id").and_then(Value::as_str).unwrap_or("-"),
                            m.get("memory_type").and_then(Value::as_str).unwrap_or("-"),
                            m.get("content").and_then(Value::as_str).unwrap_or("")
                        );
                    }
                }
            } else {
                print_human_value(&val);
            }
            Ok(())
        }
        MemoriesCommand::Add { content, category } => {
            let msg = if let Some(c) = client {
                let body = json!({ "memory_type": category, "content": content });
                match c
                    .post_json("/api/kb/memories/add", &[], &body, Duration::from_secs(10))
                    .await
                {
                    HttpOutcome::Ok(v) => v.as_str().unwrap_or("Memory added").to_string(),
                    HttpOutcome::ApiErr(e) => return Err(e),
                    HttpOutcome::Unavailable(_) => knowledge_bank::add_memory(category, content)
                        .await
                        .map_err(|e| e.to_string())?,
                }
            } else {
                knowledge_bank::add_memory(category, content)
                    .await
                    .map_err(|e| e.to_string())?
            };
            if use_json {
                print_json_ok(&json!({ "message": msg }));
            } else {
                println!("{msg}");
            }
            Ok(())
        }
        MemoriesCommand::Delete { id } => {
            let msg = if let Some(c) = client {
                let body = json!({ "id": id });
                match c
                    .post_json(
                        "/api/kb/memories/delete",
                        &[],
                        &body,
                        Duration::from_secs(10),
                    )
                    .await
                {
                    HttpOutcome::Ok(v) => v.as_str().unwrap_or("Memory deleted").to_string(),
                    HttpOutcome::ApiErr(e) => return Err(e),
                    HttpOutcome::Unavailable(_) => knowledge_bank::delete_memory(id)
                        .await
                        .map_err(|e| e.to_string())?,
                }
            } else {
                knowledge_bank::delete_memory(id)
                    .await
                    .map_err(|e| e.to_string())?
            };
            if use_json {
                print_json_ok(&json!({ "message": msg }));
            } else {
                println!("{msg}");
            }
            Ok(())
        }
        MemoriesCommand::Learn { pattern, solution } => {
            let solution_id = if let Some(c) = client {
                let body = json!({
                    "error_pattern": pattern,
                    "solution_text": solution,
                });
                match c
                    .post_json("/api/kb/learn", &[], &body, Duration::from_secs(10))
                    .await
                {
                    HttpOutcome::Ok(v) => v.as_i64().unwrap_or(0),
                    HttpOutcome::ApiErr(e) => return Err(e),
                    HttpOutcome::Unavailable(_) => knowledge_bank::kb_learn(pattern, solution)
                        .await
                        .map_err(|e| e.to_string())?,
                }
            } else {
                knowledge_bank::kb_learn(pattern, solution)
                    .await
                    .map_err(|e| e.to_string())?
            };
            if use_json {
                print_json_ok(&json!({ "id": solution_id }));
            } else {
                println!("Learned solution #{solution_id}");
            }
            Ok(())
        }
        MemoriesCommand::Articles {
            search_query,
            locale,
        } => {
            let val = if let Some(q) = search_query {
                if let Some(c) = client {
                    match c
                        .get(
                            "/api/kb/articles/search",
                            &[("q", q.as_str()), ("locale", locale.as_str())],
                            Duration::from_secs(10),
                        )
                        .await
                    {
                        HttpOutcome::Ok(v) => v,
                        HttpOutcome::ApiErr(e) => return Err(e),
                        HttpOutcome::Unavailable(_) => {
                            let arts = kb_articles::kb_search_articles(q, locale)
                                .await
                                .map_err(|e| e.to_string())?;
                            serde_json::to_value(arts).map_err(|e| e.to_string())?
                        }
                    }
                } else {
                    let arts = kb_articles::kb_search_articles(q, locale)
                        .await
                        .map_err(|e| e.to_string())?;
                    serde_json::to_value(arts).map_err(|e| e.to_string())?
                }
            } else if let Some(c) = client {
                match c
                    .get(
                        "/api/kb/articles",
                        &[("locale", locale.as_str())],
                        Duration::from_secs(10),
                    )
                    .await
                {
                    HttpOutcome::Ok(v) => v,
                    HttpOutcome::ApiErr(e) => return Err(e),
                    HttpOutcome::Unavailable(_) => {
                        let arts = kb_articles::kb_list_articles(locale)
                            .await
                            .map_err(|e| e.to_string())?;
                        serde_json::to_value(arts).map_err(|e| e.to_string())?
                    }
                }
            } else {
                let arts = kb_articles::kb_list_articles(locale)
                    .await
                    .map_err(|e| e.to_string())?;
                serde_json::to_value(arts).map_err(|e| e.to_string())?
            };

            if use_json {
                print_json_ok(&val);
            } else if let Some(arr) = val.as_array() {
                println!("{:<32} TITLE", "SLUG");
                for a in arr {
                    println!(
                        "{:<32} {}",
                        truncate(a.get("slug").and_then(Value::as_str).unwrap_or("-"), 31),
                        a.get("title").and_then(Value::as_str).unwrap_or("-")
                    );
                }
            } else {
                print_human_value(&val);
            }
            Ok(())
        }
    }
}

// ============================================================================
// 17. Sessions Handler
// ============================================================================

pub async fn handle_sessions(
    action: SessionsCommand,
    client: Option<&DesktopClient>,
    use_json: bool,
) -> Result<(), String> {
    ensure_kb_init();
    match action {
        SessionsCommand::List { include_a2a } => {
            let include_str = if include_a2a { "true" } else { "false" };
            let filtered = if let Some(c) = client {
                match c
                    .get(
                        "/api/ai/conversations",
                        &[("include_a2a", include_str)],
                        Duration::from_secs(10),
                    )
                    .await
                {
                    HttpOutcome::Ok(v) => {
                        serde_json::from_value::<Vec<ai_chat::AiConversation>>(v)
                            .map_err(|e| e.to_string())?
                            .into_iter()
                            .filter(|c| include_a2a || c.id != ai_chat::A2A_DEFAULT_CONVERSATION_ID)
                            .collect::<Vec<_>>()
                    }
                    HttpOutcome::ApiErr(e) => return Err(e),
                    HttpOutcome::Unavailable(_) => {
                        ai_chat::ai_chat_list_conversations_filtered(include_a2a)
                            .await
                            .map_err(|e| e.to_string())?
                    }
                }
            } else {
                ai_chat::ai_chat_list_conversations_filtered(include_a2a)
                    .await
                    .map_err(|e| e.to_string())?
            };

            if use_json {
                print_json_ok(&filtered);
            } else if filtered.is_empty() {
                println!("No chat sessions.");
            } else {
                println!("{:<30} {:<8} {:<24} PREVIEW", "SESSION ID", "MSGS", "TITLE");
                for s in &filtered {
                    println!(
                        "{:<30} {:<8} {:<24} {}",
                        truncate(&s.id, 29),
                        s.message_count,
                        truncate(if s.title.is_empty() { "-" } else { &s.title }, 23),
                        truncate(&s.preview, 40)
                    );
                }
            }
            Ok(())
        }
        SessionsCommand::History { session_id } => {
            let sid = session_id.unwrap_or_else(|| ai_chat::DEFAULT_CONVERSATION_ID.to_string());
            let msgs = if let Some(c) = client {
                match c
                    .get(
                        "/api/ai/history",
                        &[("conversationId", sid.as_str())],
                        Duration::from_secs(10),
                    )
                    .await
                {
                    HttpOutcome::Ok(v) => {
                        serde_json::from_value::<Vec<ai_chat::AiChatHistoryMessage>>(v)
                            .map_err(|e| e.to_string())?
                    }
                    HttpOutcome::ApiErr(e) => return Err(e),
                    HttpOutcome::Unavailable(_) => ai_chat::ai_chat_load_history(Some(sid.clone()))
                        .await
                        .map_err(|e| e.to_string())?,
                }
            } else {
                ai_chat::ai_chat_load_history(Some(sid.clone()))
                    .await
                    .map_err(|e| e.to_string())?
            };

            if use_json {
                print_json_ok(&json!({ "session_id": sid, "messages": msgs }));
            } else if msgs.is_empty() {
                println!("Session '{sid}' has no messages.");
            } else {
                for m in &msgs {
                    println!("[{}] {}", m.role, m.content);
                }
            }
            Ok(())
        }
        SessionsCommand::Clear { session_id } => {
            let sid = session_id.unwrap_or_else(|| ai_chat::DEFAULT_CONVERSATION_ID.to_string());
            if let Some(c) = client {
                let body = json!({ "conversationId": sid });
                match c
                    .post_json("/api/ai/history/clear", &[], &body, Duration::from_secs(10))
                    .await
                {
                    HttpOutcome::Ok(_) => {
                        if use_json {
                            print_json_ok(&json!({ "cleared": true, "session_id": sid }));
                        } else {
                            println!("Cleared history for session '{sid}'.");
                        }
                        return Ok(());
                    }
                    HttpOutcome::ApiErr(e) => return Err(e),
                    HttpOutcome::Unavailable(_) => {}
                }
            }
            ai_chat::ai_chat_clear_history(Some(sid.clone()))
                .await
                .map_err(|e| e.to_string())?;
            if use_json {
                print_json_ok(&json!({ "cleared": true, "session_id": sid }));
            } else {
                println!("Cleared history for session '{sid}'.");
            }
            Ok(())
        }
        SessionsCommand::Delete { session_id } => {
            if let Some(c) = client {
                let body = json!({ "id": session_id });
                match c
                    .post_json(
                        "/api/ai/conversations/delete",
                        &[],
                        &body,
                        Duration::from_secs(10),
                    )
                    .await
                {
                    HttpOutcome::Ok(_) => {
                        if use_json {
                            print_json_ok(&json!({ "deleted": true, "session_id": session_id }));
                        } else {
                            println!("Deleted session '{session_id}'.");
                        }
                        return Ok(());
                    }
                    HttpOutcome::ApiErr(e) => return Err(e),
                    HttpOutcome::Unavailable(_) => {}
                }
            }
            ai_chat::ai_chat_delete_conversation(session_id.clone())
                .await
                .map_err(|e| e.to_string())?;
            if use_json {
                print_json_ok(&json!({ "deleted": true, "session_id": session_id }));
            } else {
                println!("Deleted session '{session_id}'.");
            }
            Ok(())
        }
    }
}

// ============================================================================
// 18. UI Handler
// ============================================================================

pub async fn handle_ui(
    action: UiCommand,
    client: Option<&DesktopClient>,
    use_json: bool,
) -> Result<(), String> {
    match action {
        UiCommand::Navigate {
            page,
            section,
            toast,
        } => {
            let Some(c) = client else {
                return Err(
                    "ColimaUI Desktop is not running. Launch ColimaUI Desktop to use `ui navigate`."
                        .to_string(),
                );
            };
            let body = json!({
                "page": page,
                "section": section,
                "toast": toast,
            });
            match c
                .post_json("/api/ui/navigate", &[], &body, Duration::from_secs(5))
                .await
            {
                HttpOutcome::Ok(val) => {
                    let out = if val.is_null() {
                        json!({ "navigated": true, "page": page, "section": section, "toast": toast })
                    } else {
                        val
                    };
                    if use_json {
                        print_json_ok(&out);
                    } else {
                        println!("Navigated ColimaUI Desktop to '{page}'.");
                    }
                    Ok(())
                }
                HttpOutcome::ApiErr(e) => Err(e),
                HttpOutcome::Unavailable(reason) => Err(format!(
                    "ColimaUI Desktop UI navigation endpoint unavailable ({reason}). Ensure the latest ColimaUI Desktop is running."
                )),
            }
        }
    }
}

// ============================================================================
// 19. Events SSE Stream Handler
// ============================================================================

pub async fn handle_events(
    args: EventsArgs,
    client: Option<&DesktopClient>,
    use_json: bool,
) -> Result<(), String> {
    let Some(c) = client else {
        return Err(
            "ColimaUI Desktop HTTP API is not running. Start ColimaUI Desktop to stream `/api/events`."
                .to_string(),
        );
    };

    let effective_limit = match args.limit {
        Some(n) => Some(n),
        None if use_json => Some(10),
        None => None,
    };

    let timeout = if use_json {
        Some(Duration::from_secs(args.timeout_secs.max(1)))
    } else {
        None
    };

    let events = c
        .stream_sse(
            args.topics.as_deref(),
            args.filter.as_deref(),
            effective_limit,
            timeout,
            |ev| {
                if !use_json {
                    let name = ev.get("event").and_then(Value::as_str).unwrap_or("message");
                    let data = ev.get("data").unwrap_or(&Value::Null);
                    let compact = serde_json::to_string(data).unwrap_or_default();
                    println!("[{name}] {compact}");
                }
            },
        )
        .await?;

    if use_json {
        print_json_ok(&events);
    }
    Ok(())
}

// ============================================================================
// 20. Doctor Handler
// ============================================================================

pub async fn handle_doctor(
    client: Option<&DesktopClient>,
    use_json: bool,
) -> Result<(), String> {
    let creds = inspect_credentials();
    let live_port = match client {
        Some(c) => Some(c.port),
        None => probe_active_port().await,
    };
    let desktop_api_reachable = client.is_some();

    let db_path = creds.data_dir.join("knowledge.db");
    let knowledge_db_exists = db_path.exists();
    let settings = read_settings_readonly(&db_path);
    let resolved_ai = agent_loop::resolve_ai_config(&HeadlessChatRequest::default(), &settings);

    let tools = tokio::task::spawn_blocking(|| {
        let mut map = serde_json::Map::new();
        for tool in ["colima", "docker", "kubectl", "limactl", "trivy"] {
            map.insert(tool.to_string(), probe_binary_info(tool));
        }
        Value::Object(map)
    })
    .await
    .map_err(|e| format!("Task join error: {e}"))?;

    let env_resolutions = tokio::task::spawn_blocking(resolution::get_environment_resolutions)
        .await
        .map_err(|e| format!("Task join error: {e}"))?;

    let report = json!({
        "version": env!("CARGO_PKG_VERSION"),
        "desktop_api_reachable": desktop_api_reachable,
        "bound_port": live_port.or(creds.recorded_port),
        "recorded_port": creds.recorded_port,
        "token_file_present": creds.token_file_present && creds.token_non_empty,
        "token_permissions_0600": creds.token_permissions_0600,
        "ai_provider": resolved_ai.provider,
        "ai_model": resolved_ai.model,
        "knowledge_db_exists": knowledge_db_exists,
        "data_dir": creds.data_dir.to_string_lossy(),
        "tools": tools,
        "environment_resolutions": env_resolutions,
    });

    if use_json {
        print_json_ok(&report);
    } else {
        println!("ColimaUI Doctor (v{})", env!("CARGO_PKG_VERSION"));
        println!("────────────────────────────────────────────────────");
        println!(
            "Desktop API Reachable : {}",
            if desktop_api_reachable { "yes" } else { "no (standalone fallback ready)" }
        );
        println!(
            "Bound Port            : {}",
            live_port
                .or(creds.recorded_port)
                .map_or_else(|| "-".to_string(), |p| p.to_string())
        );
        println!(
            "Token File Present    : {} (0600 perms: {})",
            if creds.token_file_present && creds.token_non_empty {
                "yes"
            } else {
                "no"
            },
            if creds.token_permissions_0600 {
                "yes"
            } else {
                "no"
            }
        );
        println!(
            "AI Provider / Model   : {} / {}",
            resolved_ai.provider, resolved_ai.model
        );
        println!(
            "Knowledge DB Exists   : {}",
            if knowledge_db_exists { "yes" } else { "no" }
        );
        println!("\nHost Binaries:");
        if let Some(obj) = report.get("tools").and_then(Value::as_object) {
            for (name, info) in obj {
                let installed = info
                    .get("installed")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                let ver = info.get("version").and_then(Value::as_str).unwrap_or("-");
                let path = info.get("path").and_then(Value::as_str).unwrap_or("-");
                let mark = if installed { "✓" } else { "✗" };
                println!("  {mark} {:<10} {:<36} ({path})", name, truncate(ver, 35));
            }
        }
    }

    Ok(())
}

fn read_settings_readonly(db_path: &Path) -> HashMap<String, String> {
    let mut map = HashMap::new();
    if !db_path.exists() {
        return map;
    }
    let Ok(conn) = Connection::open_with_flags(
        db_path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    ) else {
        return map;
    };
    let Ok(mut stmt) = conn.prepare("SELECT setting_key, setting_value FROM app_settings") else {
        return map;
    };
    if let Ok(rows) = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    }) {
        for (k, v) in rows.filter_map(Result::ok) {
            map.insert(k, v);
        }
    }
    map
}

fn probe_binary_info(tool: &str) -> Value {
    let resolved = crate::path_util::resolve_binary(tool);
    let mut cmd = std::process::Command::new(&resolved);
    crate::path_util::apply_path_to_cmd(&mut cmd);
    match tool {
        "kubectl" => {
            cmd.args(["version", "--client", "--output=yaml"]);
        }
        "trivy" => {
            cmd.arg("--version");
        }
        _ => {
            cmd.arg("--version");
        }
    }

    match cmd.output() {
        Ok(out) => {
            let text = if out.stdout.is_empty() {
                String::from_utf8_lossy(&out.stderr).to_string()
            } else {
                String::from_utf8_lossy(&out.stdout).to_string()
            };
            let first_line = text.lines().next().unwrap_or("").trim().to_string();
            json!({
                "installed": true,
                "path": resolved,
                "version": first_line,
            })
        }
        Err(_) => json!({
            "installed": false,
            "path": resolved,
            "version": "",
        }),
    }
}

// ============================================================================
// 21. Headless AI Chat Handler
// ============================================================================

pub async fn handle_chat(
    args: ChatArgs,
    client: Option<&DesktopClient>,
    use_json: bool,
) -> Result<(), String> {
    let prompt = args.message.join(" ").trim().to_string();
    if prompt.is_empty() {
        return Err("Prompt must not be empty".to_string());
    }

    let i18n = I18n::new();
    if !use_json {
        print!("{}", i18n.t("thinking"));
        let _ = io::stdout().flush();
    }

    let req_payload = HeadlessChatRequest {
        prompt: prompt.clone(),
        provider: args.provider.clone(),
        model: args.model.clone(),
        allow_actions: args.yes,
        session_id: Some(args.session.clone()),
        mirror_ui: args.mirror_ui,
    };

    let outcome_res: Result<(String, Value, Option<String>), String> = if let Some(c) = client {
        let body = serde_json::to_value(&req_payload).map_err(|e| e.to_string())?;
        match c
            .post_json("/api/cli/chat", &[], &body, Duration::from_secs(180))
            .await
        {
            HttpOutcome::Ok(v) => {
                let reply = v
                    .get("reply")
                    .and_then(Value::as_str)
                    .map_or_else(|| v.to_string(), str::to_string);
                let tools_executed = v
                    .get("tools_executed")
                    .cloned()
                    .unwrap_or_else(|| json!([]));
                let navigated_to = v
                    .get("navigated_to")
                    .and_then(Value::as_str)
                    .map(str::to_string);
                Ok((reply, tools_executed, navigated_to))
            }
            HttpOutcome::ApiErr(e) => Err(e),
            HttpOutcome::Unavailable(_) => {
                ensure_kb_init();
                agent_loop::run_headless_agent_detailed(req_payload)
                    .await
                    .map(|o| {
                        (
                            o.reply,
                            serde_json::to_value(&o.tools_executed).unwrap_or_else(|_| json!([])),
                            o.navigated_to,
                        )
                    })
            }
        }
    } else {
        ensure_kb_init();
        agent_loop::run_headless_agent_detailed(req_payload)
            .await
            .map(|o| {
                (
                    o.reply,
                    serde_json::to_value(&o.tools_executed).unwrap_or_else(|_| json!([])),
                    o.navigated_to,
                )
            })
    };

    if !use_json {
        print!("\r\x1b[2K");
        let _ = io::stdout().flush();
    }

    let (raw_reply, tools_executed, navigated_to) = outcome_res?;
    let (clean_reply, suggestions) = extract_suggestions(&raw_reply);

    if use_json {
        print_json_ok(&json!({
            "reply": clean_reply,
            "session_id": args.session,
            "allow_actions": args.yes,
            "mirror_ui": args.mirror_ui,
            "tools_executed": tools_executed,
            "navigated_to": navigated_to,
            "suggestions": suggestions,
        }));
    } else {
        println!("{clean_reply}");
        if !suggestions.is_empty() {
            println!("\nSuggested next prompts:");
            for s in &suggestions {
                println!("  • {s}");
            }
        }
    }

    Ok(())
}

// ============================================================================
// 22. MCP Server Handler
// ============================================================================

pub async fn handle_mcp() -> Result<(), String> {
    crate::mcp::run_stdio_server().await
}
