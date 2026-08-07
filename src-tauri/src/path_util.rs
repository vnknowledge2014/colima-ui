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

/// Version manager / user-local paths that need $HOME expansion
const USER_PATHS: &[&str] = &[
    // asdf version manager (manages node, kubectl, helm, etc.)
    ".asdf/shims",
    ".asdf/bin",
    // Cargo (Rust)
    ".cargo/bin",
    // Go
    "go/bin",
    // Local bin
    ".local/bin",
    // Rancher Desktop
    ".rd/bin",
    // Krew (kubectl plugin manager)
    ".krew/bin",
];

/// The computed PATH, stored for use by `apply_path_to_cmd()` in late-spawned contexts.
static COMPUTED_PATH: std::sync::OnceLock<String> = std::sync::OnceLock::new();

/// Ensure common binary paths are in the PATH environment variable.
/// Call this once at app startup **before any threads are spawned** to fix
/// the PATH for all subsequent Command calls.
///
/// # Safety
/// This function calls `env::set_var` which is unsound in multi-threaded contexts.
/// It MUST be called from `main()` before `tauri::Builder::default()` (which spawns
/// the tokio runtime and other threads).
pub fn fix_path_env() {
    let current_path = env::var("PATH").unwrap_or_default();
    let mut paths: Vec<String> = current_path.split(':').map(|s| s.to_string()).collect();

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

        // Add version-manager and user-local paths (asdf, cargo, go, etc.)
        for rel in USER_PATHS {
            let full = format!("{}/{}", home, rel);
            if !paths.contains(&full) && PathBuf::from(&full).exists() {
                paths.push(full);
            }
        }
    }

    // Try to load user's shell PATH via login shell
    // Use zsh (macOS default) first, then fall back to sh
    let shell_cmds = [
        ("/bin/zsh", &["-l", "-c", "echo $PATH"][..]),
        ("/bin/sh", &["-l", "-c", "echo $PATH"][..]),
    ];
    for (shell, args) in &shell_cmds {
        if let Ok(output) = Command::new(shell).args(*args).output() {
            if output.status.success() {
                let shell_path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                for p in shell_path.split(':') {
                    let ps = p.to_string();
                    if !ps.is_empty() && !paths.contains(&ps) {
                        paths.push(ps);
                    }
                }
                break; // Use the first successful shell
            }
        }
    }

    let new_path = paths.join(":");

    // Store for per-Command application in late-spawned threads
    let _ = COMPUTED_PATH.set(new_path.clone());

    // Fix #17: Guard against accidental use from multi-threaded context.
    // This function MUST be called exactly once from main(), before any threads exist.
    #[cfg(debug_assertions)]
    {
        use std::sync::atomic::{AtomicBool, Ordering};
        static CALLED: AtomicBool = AtomicBool::new(false);
        assert!(
            !CALLED.swap(true, Ordering::SeqCst),
            "fix_path_env() must only be called once, from main() before the async runtime starts"
        );
    }

    // SAFETY: Called from main() before any threads are spawned (before tauri::Builder).
    // No concurrent readers/writers of the environment exist at this point.
    unsafe { env::set_var("PATH", &new_path) };
}

/// Apply the computed PATH to a Command. Use this in contexts where the global
/// env may not have been inherited (e.g., spawned after fix_path_env).
#[allow(dead_code)]
pub fn apply_path_to_cmd(cmd: &mut Command) {
    if let Some(path) = COMPUTED_PATH.get() {
        cmd.env("PATH", path);
    }
}

/// Resolve the full path for a binary by checking PATH
#[allow(dead_code)]
pub fn resolve_binary(name: &str) -> String {
    // First check if the binary can be found via `which`
    if let Ok(output) = Command::new("/usr/bin/which").arg(name).output() {
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

/// Detect the Docker socket from a running Colima instance.
/// Returns something like "unix:///Users/mike/.colima/default/docker.sock".
///
/// This is critical for macOS .app bundles because:
/// 1. DOCKER_HOST env var is NOT inherited from the user's shell
/// 2. Colima doesn't create /var/run/docker.sock (no root access)
/// 3. Bollard's connect_with_defaults() only checks /var/run/docker.sock
pub fn detect_docker_host() -> Option<(String, String)> {
    if let Ok(host) = std::env::var("DOCKER_HOST") {
        if !host.is_empty() {
            return Some((host, "default".to_string()));
        }
    }

    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    let colima_home = std::env::var("COLIMA_HOME").unwrap_or_else(|_| format!("{}/.colima", home));
    let colima_path = std::path::Path::new(&colima_home);

    if !colima_path.exists() {
        return None;
    }

    // Scan profiles for a running instance (has ha.sock in _lima dir)
    if let Ok(entries) = std::fs::read_dir(colima_path) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('_') || name.starts_with('.') || !entry.path().is_dir() {
                continue;
            }

            // Map profile to lima instance name
            let lima_name = if name == "default" {
                "colima".to_string()
            } else {
                format!("colima-{}", name)
            };
            let lima_dir = colima_path.join("_lima").join(&lima_name);

            if lima_dir.join("ha.sock").exists() || lima_dir.join("ha.pid").exists() {
                // Found running instance — return its docker socket
                let sock = colima_path.join(&name).join("docker.sock");
                if sock.exists() {
                    return Some((format!("unix://{}", sock.display()), name.clone()));
                }
            }
        }
    }

    // Fallback: check the colima-level docker.sock symlink
    let fallback = colima_path.join("docker.sock");
    if fallback.exists() {
        return Some((format!("unix://{}", fallback.display()), "default".to_string()));
    }

    None
}
