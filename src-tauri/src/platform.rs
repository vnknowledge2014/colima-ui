//! Platform detection utilities.
//!
//! Detects OS, architecture, WSL, and available package managers.

use serde::Serialize;
use std::process::Command;

#[derive(Serialize)]
pub struct PackageManagerInfo {
    pub name: String,
    pub available: bool,
    pub version: String,
}

#[derive(Serialize)]
pub struct PlatformInfo {
    pub os: String,
    pub arch: String,
    pub wsl: bool,
    pub wsl_available: bool,
    pub package_managers: Vec<PackageManagerInfo>,
}

fn check_cmd_version(cmd: &str, args: &[&str]) -> (bool, String) {
    match Command::new(cmd).args(args).output() {
        Ok(o) if o.status.success() => {
            let ver = String::from_utf8_lossy(&o.stdout)
                .lines()
                .next()
                .unwrap_or("")
                .to_string();
            (true, ver)
        }
        _ => (false, String::new()),
    }
}

pub fn detect_platform() -> PlatformInfo {
    let os = std::env::consts::OS.to_string(); // "macos", "linux", "windows"
    let arch = std::env::consts::ARCH.to_string(); // "x86_64", "aarch64"

    // WSL detection: check /proc/version for microsoft/WSL or env var
    let wsl = if cfg!(target_os = "linux") {
        std::env::var("WSL_DISTRO_NAME").is_ok()
            || std::fs::read_to_string("/proc/version")
                .unwrap_or_default()
                .to_lowercase()
                .contains("microsoft")
    } else {
        false
    };

    // On Windows, check if WSL is available
    let wsl_available = if cfg!(target_os = "windows") {
        Command::new("wsl")
            .arg("--list")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    } else {
        false
    };

    // Detect package managers
    let mut pms = Vec::new();

    // Homebrew (macOS + Linux)
    let (brew_ok, brew_ver) = check_cmd_version("brew", &["--version"]);
    pms.push(PackageManagerInfo {
        name: "brew".to_string(),
        available: brew_ok,
        version: brew_ver,
    });

    // apt (Linux / WSL)
    if cfg!(target_os = "linux") || wsl {
        let (apt_ok, apt_ver) = check_cmd_version("apt", &["--version"]);
        pms.push(PackageManagerInfo {
            name: "apt".to_string(),
            available: apt_ok,
            version: apt_ver,
        });
    }

    // nix (any platform)
    let (nix_ok, nix_ver) = check_cmd_version("nix", &["--version"]);
    pms.push(PackageManagerInfo {
        name: "nix".to_string(),
        available: nix_ok,
        version: nix_ver,
    });

    // Always offer manual
    pms.push(PackageManagerInfo {
        name: "manual".to_string(),
        available: true,
        version: String::new(),
    });

    PlatformInfo {
        os,
        arch,
        wsl,
        wsl_available,
        package_managers: pms,
    }
}

#[tauri::command]
pub async fn get_platform() -> Result<PlatformInfo, String> {
    Ok(detect_platform())
}
