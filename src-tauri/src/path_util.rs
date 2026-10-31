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

/// `~/.colima-ui` — where this app keeps everything it writes.
///
/// Extracted when a third caller appeared (crash reports). Two ad-hoc copies of
/// a path is a coincidence; three is a convention that should live in one place,
/// so a future move does not leave one directory behind.
///
/// An empty `HOME` counts as missing, not as a valid prefix. `env::var` returns
/// `Ok("")` for a variable that is set but blank, and
/// `PathBuf::from("").join(".colima-ui")` is the *relative* path `.colima-ui` —
/// which would put the knowledge bank and the crash reports in whatever
/// directory the app happened to be launched from, silently, a different one
/// each time.
///
/// The `/tmp` fallback matches what the two original callers did. A process with
/// no `HOME` is not a normal desktop session, and losing the knowledge bank
/// there is better than refusing to start.
pub fn app_data_dir() -> PathBuf {
    let home = env::var("HOME")
        .ok()
        .filter(|h| !h.is_empty())
        .unwrap_or_else(|| "/tmp".to_string());
    let dir = PathBuf::from(home).join(".colima-ui");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if dir.exists() {
            let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700));
        }
    }
    dir
}

/// Ensure common binary paths are in the PATH environment variable.
///
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
        if !paths.contains(&extra_str) && PathBuf::from(extra).exists() {
            paths.push(extra_str);
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

/// Marks Colima profiles created to detonate an untrusted image.
///
/// Defined here rather than in `commands::detonation` because this module is
/// what decides which instance the app's docker calls reach, and it must be
/// able to exclude them without depending on a command module. `detonation`
/// re-uses this constant so the name the sweep deletes and the name daemon
/// detection skips can never drift apart.
pub const DETONATION_PROFILE_PREFIX: &str = "colimaui-detonate-";

/// Cache TTL for socket health probes to prevent hot-path I/O overhead.
pub const PROBE_CACHE_TTL: std::time::Duration = std::time::Duration::from_secs(2);
/// Timeout for Unix socket HTTP probing.
pub const PROBE_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(300);

static PROBE_CACHE: std::sync::LazyLock<
    std::sync::Mutex<std::collections::HashMap<PathBuf, (bool, std::time::Instant)>>,
> = std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashMap::new()));

/// Clears the 2-second socket probe cache.
pub fn invalidate_probe_cache() {
    let mut cache = PROBE_CACHE.lock().unwrap_or_else(|e| e.into_inner());
    cache.clear();
}

#[cfg(test)]
pub(crate) static TEST_PROBE_GUARD: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(test)]
pub(crate) fn test_probe_exclusive() -> std::sync::MutexGuard<'static, ()> {
    TEST_PROBE_GUARD.lock().unwrap_or_else(|e| e.into_inner())
}

/// Probes a Docker Unix domain socket by performing an HTTP `GET /_ping` handshake.
///
/// Returns `true` if the socket responds with HTTP 200 OK.
/// Returns `false` on connection error, immediate EOF (handling Lima SSH port forwarder
/// when the daemon inside the VM is dead or booting), read timeout, or non-200 status.
#[cfg(unix)]
pub fn probe_docker_socket(sock: &std::path::Path, timeout: std::time::Duration) -> bool {
    use std::io::{Read, Write};
    use std::os::unix::net::UnixStream;

    let Ok(mut stream) = UnixStream::connect(sock) else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(timeout));
    let _ = stream.set_write_timeout(Some(timeout));
    if stream
        .write_all(b"GET /_ping HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
        .is_err()
    {
        return false;
    }
    let mut buf = [0u8; 256];
    let Ok(n) = stream.read(&mut buf) else {
        return false;
    };
    if n == 0 {
        return false;
    }
    let head = std::str::from_utf8(&buf[..n]).unwrap_or("");
    let mut parts = head.lines().next().unwrap_or("").split_whitespace();
    let version_ok = parts.next().is_some_and(|v| v.starts_with("HTTP/1."));
    version_ok && parts.next() == Some("200")
}

#[cfg(not(unix))]
pub fn probe_docker_socket(_sock: &std::path::Path, _timeout: std::time::Duration) -> bool {
    false
}

/// Probes a Docker Unix domain socket with a 2-second TTL cache to prevent hot-path I/O overhead.
pub fn probe_docker_socket_cached(sock: &std::path::Path) -> bool {
    let now = std::time::Instant::now();
    {
        let cache = PROBE_CACHE.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(&(healthy, timestamp)) = cache.get(sock) {
            if now.duration_since(timestamp) < PROBE_CACHE_TTL {
                return healthy;
            }
        }
    }

    let healthy = probe_docker_socket(sock, PROBE_TIMEOUT);

    {
        let mut cache = PROBE_CACHE.lock().unwrap_or_else(|e| e.into_inner());
        cache.insert(sock.to_path_buf(), (healthy, std::time::Instant::now()));
    }

    healthy
}

#[cfg(unix)]
fn is_pid_alive(pid: libc::pid_t) -> bool {
    if pid <= 0 {
        return false;
    }
    let res = unsafe { libc::kill(pid, 0) };
    if res == 0 {
        return true;
    }
    std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

/// Check if a Lima instance is alive by testing `ha.sock` connection and inspecting `ha.pid`.
///
/// On Unix, tests `ha.sock` via `UnixStream::connect`. If `ha.sock` exists and accepts connections,
/// the hostagent is alive. If `ha.sock` exists but connection fails (e.g. ECONNREFUSED), the
/// hostagent is dead. If `ha.sock` does not exist yet (during early boot), checks `ha.pid`
/// via `libc::kill(pid, 0)`.
#[cfg(unix)]
pub fn is_lima_instance_alive(lima_dir: &std::path::Path) -> bool {
    let ha_sock = lima_dir.join("ha.sock");
    if ha_sock.exists() {
        return std::os::unix::net::UnixStream::connect(&ha_sock).is_ok();
    }

    let ha_pid = lima_dir.join("ha.pid");
    if let Ok(content) = std::fs::read_to_string(&ha_pid) {
        if let Ok(pid) = content.trim().parse::<libc::pid_t>() {
            return is_pid_alive(pid);
        }
    }

    false
}

#[cfg(not(unix))]
pub fn is_lima_instance_alive(_lima_dir: &std::path::Path) -> bool {
    false
}

fn get_docker_current_context() -> Option<String> {
    let config_path = if let Ok(docker_config) = std::env::var("DOCKER_CONFIG") {
        PathBuf::from(docker_config).join("config.json")
    } else if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(".docker").join("config.json")
    } else {
        return None;
    };

    if !config_path.exists() {
        return None;
    }

    let content = std::fs::read_to_string(config_path).ok()?;
    let parsed: serde_json::Value = serde_json::from_str(&content).ok()?;
    parsed
        .get("currentContext")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

fn profile_from_docker_context(context: &str) -> Option<String> {
    if context == "colima" {
        Some("default".to_string())
    } else {
        context
            .strip_prefix("colima-")
            .filter(|p| !p.is_empty())
            .map(|p| p.to_string())
    }
}

/// Lima instance store (VM dirs incl. ha.pid/ha.sock).
///
/// Mirrors colima's own `limaDir()`: `LIMA_HOME` is honored unconditionally
/// when non-empty (it is provisioned into this process at startup — see
/// `shell_env::adopt`), else instances live under the colima config home's
/// `_lima/`.
pub fn lima_home() -> PathBuf {
    if let Ok(home) = std::env::var("LIMA_HOME") {
        if !home.trim().is_empty() {
            return PathBuf::from(home);
        }
    }
    crate::instance_reader::colima_home().join("_lima")
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
        let trimmed = host.trim();
        if !trimmed.is_empty() {
            if trimmed.starts_with("unix://") {
                let sock_path = std::path::Path::new(trimmed.trim_start_matches("unix://"));
                if probe_docker_socket_cached(sock_path) {
                    return Some((trimmed.to_string(), "default".to_string()));
                }
            } else {
                return Some((trimmed.to_string(), "default".to_string()));
            }
        }
    }

    let colima_home = crate::instance_reader::colima_home();
    let colima_path = std::path::Path::new(&colima_home);

    if !colima_path.exists() {
        return None;
    }

    let active_profile =
        get_docker_current_context().and_then(|ctx| profile_from_docker_context(&ctx));

    // Scan profiles for a running instance
    if let Ok(entries) = std::fs::read_dir(colima_path) {
        let mut profiles = Vec::new();
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('_') || name.starts_with('.') || !entry.path().is_dir() {
                continue;
            }
            // A detonation instance is running an untrusted image and is deleted
            // minutes later. `read_dir` order is arbitrary, so without this the
            // app could bind its whole docker surface — container list, metrics,
            // bollard — to the VM holding the sample, and then to nothing when
            // that VM is destroyed.
            if name.starts_with(DETONATION_PROFILE_PREFIX) {
                continue;
            }
            profiles.push(name);
        }

        // Deterministic sort: active context profile first, then "default", then alphabetical
        profiles.sort_by(|a, b| {
            let a_is_active = active_profile.as_deref() == Some(a.as_str());
            let b_is_active = active_profile.as_deref() == Some(b.as_str());
            if a_is_active != b_is_active {
                return b_is_active.cmp(&a_is_active);
            }
            let a_is_default = a == "default";
            let b_is_default = b == "default";
            if a_is_default != b_is_default {
                return b_is_default.cmp(&a_is_default);
            }
            a.cmp(b)
        });

        for name in profiles {
            // Map profile to lima instance name
            let lima_name = if name == "default" {
                "colima".to_string()
            } else {
                format!("colima-{}", name)
            };
            let lima_dir = lima_home().join(&lima_name);
            let sock = colima_path.join(&name).join("docker.sock");

            // Both gates required: the aliveness files prove the VM/forwarder
            // pair is up (a stale forwarder socket may still answer PING for a
            // dead instance — the adversarial suite pins that), and the probe
            // proves the daemon actually answers.
            if is_lima_instance_alive(&lima_dir) && probe_docker_socket_cached(&sock) {
                return Some((format!("unix://{}", sock.display()), name));
            }
        }
    }

    // Fallback: check the colima-level docker.sock symlink.
    //
    // Colima repoints this symlink at whichever profile started last, so during
    // a detonation it can lead to the VM running the untrusted image. Reachable
    // when the user's own instance is stopped: the scan above finds nothing
    // running, and this would otherwise hand back the sample's daemon. Resolve
    // it and check where it actually goes.
    let fallback = colima_path.join("docker.sock");
    if fallback.exists() {
        let points_at_detonation = std::fs::canonicalize(&fallback)
            .ok()
            .is_some_and(|target| {
                target
                    .components()
                    .any(|c| c.as_os_str().to_string_lossy().starts_with(DETONATION_PROFILE_PREFIX))
            });
        if !points_at_detonation && probe_docker_socket_cached(&fallback) {
            return Some((format!("unix://{}", fallback.display()), "default".to_string()));
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    fn temp_socket_path(test_name: &str) -> PathBuf {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = PathBuf::from(format!("/tmp/cui-{}-{}-{}.sock", test_name, std::process::id(), id));
        let _ = std::fs::remove_file(&path);
        path
    }

    struct SocketCleanup(PathBuf);
    impl Drop for SocketCleanup {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }

    #[test]
    #[cfg(unix)]
    fn test_probe_docker_socket_immediate_eof() {
        let sock_path = temp_socket_path("eof");
        let _guard = SocketCleanup(sock_path.clone());
        let listener = std::os::unix::net::UnixListener::bind(&sock_path).unwrap();

        let handle = std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                use std::io::Read;
                let mut buf = [0u8; 128];
                let _ = stream.read(&mut buf);
                // Immediately close without writing response (EOF)
            }
        });

        let healthy = probe_docker_socket(&sock_path, std::time::Duration::from_millis(300));
        assert!(!healthy, "Immediate EOF must be detected as unhealthy");
        let _ = handle.join();
    }

    #[test]
    #[cfg(unix)]
    fn test_probe_docker_socket_timeout() {
        let sock_path = temp_socket_path("timeout");
        let _guard = SocketCleanup(sock_path.clone());
        let listener = std::os::unix::net::UnixListener::bind(&sock_path).unwrap();

        let handle = std::thread::spawn(move || {
            if let Ok((_stream, _)) = listener.accept() {
                // Sleep longer than probe timeout
                std::thread::sleep(std::time::Duration::from_millis(200));
            }
        });

        let healthy = probe_docker_socket(&sock_path, std::time::Duration::from_millis(50));
        assert!(!healthy, "Read timeout must be detected as unhealthy");
        let _ = handle.join();
    }

    #[test]
    #[cfg(unix)]
    fn test_probe_docker_socket_200_ok() {
        let sock_path = temp_socket_path("ok");
        let _guard = SocketCleanup(sock_path.clone());
        let listener = std::os::unix::net::UnixListener::bind(&sock_path).unwrap();

        let handle = std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                use std::io::{Read, Write};
                let mut buf = [0u8; 128];
                let _ = stream.read(&mut buf);
                let resp = b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 2\r\n\r\nOK";
                let _ = stream.write_all(resp);
            }
        });

        let healthy = probe_docker_socket(&sock_path, std::time::Duration::from_millis(300));
        assert!(healthy, "HTTP 200 OK must report healthy");
        let _ = handle.join();
    }

    #[test]
    #[cfg(unix)]
    fn test_probe_docker_socket_orphaned_file() {
        let sock_path = temp_socket_path("orphan");
        let _guard = SocketCleanup(sock_path.clone());
        let listener = std::os::unix::net::UnixListener::bind(&sock_path).unwrap();
        // Drop listener immediately: socket file remains in filesystem, but nobody is listening
        drop(listener);

        assert!(sock_path.exists());
        let healthy = probe_docker_socket(&sock_path, std::time::Duration::from_millis(300));
        assert!(!healthy, "Orphaned socket without listener must report unhealthy");
    }

    #[test]
    #[cfg(unix)]
    fn test_probe_docker_socket_nonexistent() {
        let sock_path = PathBuf::from("/tmp/cui-does-not-exist-123456789.sock");
        let healthy = probe_docker_socket(&sock_path, std::time::Duration::from_millis(300));
        assert!(!healthy, "Nonexistent socket must report unhealthy");
    }

    #[test]
    #[cfg(unix)]
    fn test_probe_docker_socket_cached_ttl_and_invalidation() {
        let _guard = test_probe_exclusive();
        invalidate_probe_cache();

        let sock_path = temp_socket_path("cache");
        let _clean = SocketCleanup(sock_path.clone());
        let listener = std::os::unix::net::UnixListener::bind(&sock_path).unwrap();

        let handle = std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                use std::io::{Read, Write};
                let mut buf = [0u8; 128];
                let _ = stream.read(&mut buf);
                let resp = b"HTTP/1.1 200 OK\r\n\r\nOK";
                let _ = stream.write_all(resp);
            }
        });

        // 1. Initial probe succeeds and caches (true, Instant::now())
        let healthy = probe_docker_socket_cached(&sock_path);
        assert!(healthy, "Initial probe should be true");
        let _ = handle.join();

        // 2. Remove socket file now. Since cache TTL is 2s, cached probe should still return true!
        let _ = std::fs::remove_file(&sock_path);
        assert!(!sock_path.exists());
        let cached_healthy = probe_docker_socket_cached(&sock_path);
        assert!(cached_healthy, "Cached probe must return true within TTL");

        // 3. Invalidate cache: subsequent probe hits filesystem, sees socket is gone -> returns false
        invalidate_probe_cache();
        let after_invalidation = probe_docker_socket_cached(&sock_path);
        assert!(!after_invalidation, "Probe must return false after cache invalidation");
    }

    #[test]
    #[cfg(unix)]
    fn test_probe_docker_socket_http_500_with_200_in_headers() {
        let sock_path = temp_socket_path("adv-500");
        let _clean = SocketCleanup(sock_path.clone());
        let listener = std::os::unix::net::UnixListener::bind(&sock_path).unwrap();

        let handle = std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                use std::io::{Read, Write};
                let mut buf = [0u8; 128];
                let _ = stream.read(&mut buf);
                let resp = b"HTTP/1.1 500 Internal Server Error\r\nContent-Length: 200 \r\n\r\n";
                let _ = stream.write_all(resp);
            }
        });

        let healthy = probe_docker_socket(&sock_path, std::time::Duration::from_millis(300));
        assert!(!healthy, "HTTP 500 with Content-Length 200 must return false");
        let _ = handle.join();
    }

    #[test]
    #[cfg(unix)]
    fn test_is_lima_instance_alive_pid_and_sock() {
        static DIR_COUNTER: AtomicU64 = AtomicU64::new(0);
        let id = DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
        let base_dir = PathBuf::from(format!("/tmp/cui-lima-test-{}-{}", std::process::id(), id));
        let _ = std::fs::create_dir_all(&base_dir);

        struct DirCleanup(PathBuf);
        impl Drop for DirCleanup {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let _guard = DirCleanup(base_dir.clone());

        // Case 1: Empty dir
        assert!(!is_lima_instance_alive(&base_dir));

        // Case 2: Dead PID in ha.pid, no ha.sock
        let pid_file = base_dir.join("ha.pid");
        std::fs::write(&pid_file, "99999999\n").unwrap();
        assert!(!is_lima_instance_alive(&base_dir));

        // Case 3: Live PID (current process ID) in ha.pid, no ha.sock
        let current_pid = std::process::id();
        std::fs::write(&pid_file, format!("{}\n", current_pid)).unwrap();
        assert!(is_lima_instance_alive(&base_dir));

        // Case 4: Stale ha.sock exists (file exists, no listener).
        let sock_path = base_dir.join("ha.sock");
        let listener = std::os::unix::net::UnixListener::bind(&sock_path).unwrap();
        drop(listener);
        std::thread::sleep(std::time::Duration::from_millis(15));
        assert!(sock_path.exists());
        assert!(!is_lima_instance_alive(&base_dir));

        // Case 5: Active ha.sock (listener listening)
        let _ = std::fs::remove_file(&sock_path);
        let active_listener = std::os::unix::net::UnixListener::bind(&sock_path).unwrap();
        assert!(is_lima_instance_alive(&base_dir));
        drop(active_listener);
    }

    #[test]
    fn test_profile_from_docker_context() {
        assert_eq!(profile_from_docker_context("colima"), Some("default".to_string()));
        assert_eq!(profile_from_docker_context("colima-dev"), Some("dev".to_string()));
        assert_eq!(profile_from_docker_context("colima-"), None);
        assert_eq!(profile_from_docker_context("desktop-linux"), None);
    }
}
