use std::env;
use std::path::PathBuf;
use std::process::Command;

/// Common binary search paths on macOS, Linux, and Windows
const EXTRA_PATHS: &[&str] = &[
    "/usr/local/bin",
    "/opt/homebrew/bin",
    "/opt/homebrew/sbin",
    "/usr/local/sbin",
    "/usr/bin",
    "/bin",
    "/usr/sbin",
    "/sbin",
    // Nix paths
    "/nix/var/nix/profiles/default/bin",
    "/run/current-system/sw/bin",
    // Linuxbrew
    "/home/linuxbrew/.linuxbrew/bin",
];

/// Nix user-level paths that need $HOME expansion
const NIX_USER_PATHS: &[&str] = &[
    ".nix-profile/bin",
    ".local/state/nix/profiles/profile/bin",
    ".nix-defexpr/channels/bin",
];

/// Ensure common binary paths are in the PATH environment variable.
/// Call this once at app startup to fix the PATH for all subsequent Command calls.
pub fn fix_path_env() {
    let current_path = env::var("PATH").unwrap_or_default();
    let mut paths: Vec<String> = current_path
        .split(':')
        .map(|s| s.to_string())
        .collect();

    for extra in EXTRA_PATHS {
        let extra_str = extra.to_string();
        if !paths.contains(&extra_str) {
            if PathBuf::from(extra).exists() {
                paths.push(extra_str);
            }
        }
    }

    // Add Nix user-level paths (need $HOME expansion)
    if let Ok(home) = env::var("HOME") {
        for rel in NIX_USER_PATHS {
            let full = format!("{}/{}", home, rel);
            if !paths.contains(&full) && PathBuf::from(&full).exists() {
                paths.push(full);
            }
        }
    }

    // Also try to load user's shell PATH via login shell
    if let Ok(output) = Command::new("/bin/sh")
        .args(["-l", "-c", "echo $PATH"])
        .output()
    {
        if output.status.success() {
            let shell_path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            for p in shell_path.split(':') {
                let ps = p.to_string();
                if !ps.is_empty() && !paths.contains(&ps) {
                    paths.push(ps);
                }
            }
        }
    }

    let new_path = paths.join(":");
    env::set_var("PATH", &new_path);
}

/// Resolve the full path for a binary by checking PATH
#[allow(dead_code)]
pub fn resolve_binary(name: &str) -> String {
    // First check if the binary can be found via `which`
    if let Ok(output) = Command::new("/usr/bin/which")
        .arg(name)
        .output()
    {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() {
                return path;
            }
        }
    }

    // Fall back to common locations
    for dir in EXTRA_PATHS {
        let candidate = PathBuf::from(dir).join(name);
        if candidate.exists() {
            return candidate.to_string_lossy().to_string();
        }
    }

    // Last resort: just use the name and hope PATH has it
    name.to_string()
}
