//! Start Colima when the user logs in.
//!
//! The frontend has always offered this in the setup wizard, but nothing
//! implemented it on either transport — the call failed over IPC *and* 404'd
//! over HTTP. This is the missing half.
//!
//! There is no cross-platform login-item API worth wrapping here, so each OS
//! gets its native mechanism: a launchd user agent on macOS, a systemd user
//! unit on Linux. Windows has neither and is reported as unsupported rather
//! than silently doing nothing.

use serde::Serialize;
use std::path::PathBuf;
use std::process::Command;

use crate::path_util;

/// Reverse-DNS label for the launchd agent, and the stem of the systemd unit.
const AGENT_LABEL: &str = "com.colima-ui.autostart";

#[derive(Debug, Clone, Serialize)]
pub struct AutostartStatus {
    pub enabled: bool,
}

fn home_dir() -> Result<PathBuf, String> {
    std::env::var("HOME")
        .map(PathBuf::from)
        .map_err(|_| "HOME is not set; cannot locate the user's login items".to_string())
}

/// Where the unit file lives for this platform.
fn unit_path() -> Result<PathBuf, String> {
    let home = home_dir()?;
    if cfg!(target_os = "macos") {
        Ok(home
            .join("Library/LaunchAgents")
            .join(format!("{}.plist", AGENT_LABEL)))
    } else if cfg!(target_os = "linux") {
        Ok(home
            .join(".config/systemd/user")
            .join(format!("{}.service", AGENT_LABEL)))
    } else {
        Err("Auto-start is only supported on macOS and Linux".to_string())
    }
}

/// launchd and systemd both start with a near-empty environment, so the unit
/// carries the same PATH the app computed for itself. Without it `colima start`
/// cannot find `limactl`.
fn resolved_path_env() -> String {
    std::env::var("PATH").unwrap_or_else(|_| "/usr/local/bin:/usr/bin:/bin".to_string())
}

fn launchd_plist(colima: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{label}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{colima}</string>
        <string>start</string>
    </array>
    <key>EnvironmentVariables</key>
    <dict>
        <key>PATH</key>
        <string>{path}</string>
    </dict>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <false/>
</dict>
</plist>
"#,
        label = AGENT_LABEL,
        colima = colima,
        path = resolved_path_env(),
    )
}

fn systemd_unit(colima: &str) -> String {
    format!(
        r#"[Unit]
Description=Start Colima at login (colima-ui)
After=default.target

[Service]
Type=oneshot
RemainAfterExit=yes
Environment=PATH={path}
ExecStart={colima} start

[Install]
WantedBy=default.target
"#,
        colima = colima,
        path = resolved_path_env(),
    )
}

/// Run a command and fold a non-zero exit into `Err`.
fn run(program: &str, args: &[&str]) -> Result<(), String> {
    let mut cmd = Command::new(program);
    cmd.args(args);
    path_util::apply_path_to_cmd(&mut cmd);
    let output = cmd
        .output()
        .map_err(|e| format!("Failed to run {}: {}", program, e))?;
    if output.status.success() {
        return Ok(());
    }
    Err(format!(
        "{} {} failed: {}",
        program,
        args.join(" "),
        String::from_utf8_lossy(&output.stderr).trim()
    ))
}

fn enable_blocking() -> Result<String, String> {
    let path = unit_path()?;
    let dir = path
        .parent()
        .ok_or_else(|| "Auto-start directory has no parent".to_string())?;
    std::fs::create_dir_all(dir)
        .map_err(|e| format!("Failed to create {}: {}", dir.display(), e))?;

    let colima = path_util::resolve_binary("colima");
    let body = if cfg!(target_os = "macos") {
        launchd_plist(&colima)
    } else {
        systemd_unit(&colima)
    };
    std::fs::write(&path, body)
        .map_err(|e| format!("Failed to write {}: {}", path.display(), e))?;

    let path_str = path.to_string_lossy().to_string();
    if cfg!(target_os = "macos") {
        // Unload first so re-enabling picks up a rewritten plist. A agent that
        // was never loaded makes this fail, which is not an error here.
        let _ = run("launchctl", &["unload", "-w", &path_str]);
        run("launchctl", &["load", "-w", &path_str])?;
    } else {
        run("systemctl", &["--user", "daemon-reload"])?;
        run(
            "systemctl",
            &["--user", "enable", &format!("{}.service", AGENT_LABEL)],
        )?;
    }

    Ok(format!("Auto-start enabled ({})", path.display()))
}

fn disable_blocking() -> Result<String, String> {
    let path = unit_path()?;
    let path_str = path.to_string_lossy().to_string();

    if cfg!(target_os = "macos") {
        // Already-unloaded is the normal case when the user toggles twice.
        let _ = run("launchctl", &["unload", "-w", &path_str]);
    } else {
        let _ = run(
            "systemctl",
            &["--user", "disable", &format!("{}.service", AGENT_LABEL)],
        );
    }

    if path.exists() {
        std::fs::remove_file(&path)
            .map_err(|e| format!("Failed to remove {}: {}", path.display(), e))?;
    }
    if cfg!(target_os = "linux") {
        let _ = run("systemctl", &["--user", "daemon-reload"]);
    }

    Ok("Auto-start disabled".to_string())
}

pub fn configure_autostart_blocking(enable: bool) -> Result<String, String> {
    if enable {
        enable_blocking()
    } else {
        disable_blocking()
    }
}

/// Presence of the unit file is the source of truth: both `enable_blocking` and
/// `disable_blocking` write it last and remove it first, so it cannot claim to
/// be on while the agent is not registered.
pub fn autostart_status_blocking() -> AutostartStatus {
    AutostartStatus {
        enabled: unit_path().map(|p| p.exists()).unwrap_or(false),
    }
}

/// Turn "start Colima at login" on or off
#[tauri::command]
pub async fn configure_autostart(enable: bool) -> Result<String, crate::error::ColimaError> {
    tokio::task::spawn_blocking(move || configure_autostart_blocking(enable))
        .await
        .map_err(|e| format!("Task join error: {}", e))
        .and_then(|r| r)
        .map_err(crate::error::ColimaError::from)
}

/// Whether "start Colima at login" is currently configured
#[tauri::command]
pub async fn get_autostart_status() -> Result<AutostartStatus, crate::error::ColimaError> {
    tokio::task::spawn_blocking(autostart_status_blocking)
        .await
        .map_err(|e| crate::error::ColimaError::from(format!("Task join error: {}", e)))
}
