use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use std::process::Command;
use crate::path_util;

#[derive(Debug, Clone)]
pub struct ResourceSaverState {
    pub enabled: bool,
    pub idle_minutes_threshold: u64,
    pub idle_minutes_count: u64,
}

impl Default for ResourceSaverState {
    fn default() -> Self {
        Self { enabled: false, idle_minutes_threshold: 15, idle_minutes_count: 0 }
    }
}


#[derive(Debug, Clone, Deserialize)]
pub struct PluginsConfig {
    pub plugins: HashMap<String, PluginDef>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PluginDef {
    pub description: String,
    pub command: Option<String>,
    pub prompt: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SystemInfo {
    pub colima_installed: bool,
    pub colima_version: String,
    pub docker_installed: bool,
    pub docker_version: String,
    pub lima_installed: bool,
    pub lima_version: String,
}

/// Probe a tool's version, resolving it through the fixed PATH so it also works
/// when the app was launched from Finder.
///
/// `pub(crate)` so `system_capabilities` can reuse it rather than growing a
/// second, subtly different probe.
pub(crate) fn get_version(cmd: &str, args: &[&str]) -> Option<String> {
    let resolved = path_util::resolve_binary(cmd);
    let mut command = Command::new(&resolved);
    command.args(args);
    path_util::apply_path_to_cmd(&mut command);
    command
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

/// Check system prerequisites
/// Runs on a blocking thread with a 5s timeout to prevent app startup hangs
/// when Docker/Colima daemons are unresponsive.
#[tauri::command]
pub async fn check_system() -> Result<SystemInfo, crate::error::ColimaError> {
    async move {
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        tokio::task::spawn_blocking(|| {
            let colima_version = get_version("colima", &["version"]);
            // Use --version (not context-dependent) to avoid Docker daemon connection
            let docker_version = get_version("docker", &["--version"]);
            let lima_version = get_version("limactl", &["--version"]);

            SystemInfo {
                colima_installed: colima_version.is_some(),
                colima_version: colima_version.unwrap_or_default(),
                docker_installed: docker_version.is_some(),
                docker_version: docker_version.unwrap_or_default(),
                lima_installed: lima_version.is_some(),
                lima_version: lima_version.unwrap_or_default(),
            }
        }),
    )
    .await;

    match result {
        Ok(Ok(info)) => Ok(info),
        Ok(Err(e)) => Err(format!("Task join error: {}", e)),
        Err(_) => {
            // Timeout — return partial info so the app doesn't freeze
            Ok(SystemInfo {
                colima_installed: false,
                colima_version: String::new(),
                docker_installed: false,
                docker_version: String::new(),
                lima_installed: false,
                lima_version: String::new(),
            })
        }
    }
    }
    .await.map_err(|e: String| crate::error::ColimaError::from(e))
}

/// Get Colima version string
#[tauri::command]
pub async fn get_colima_version() -> Result<String, crate::error::ColimaError> {
    async move {
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        tokio::task::spawn_blocking(|| {
            Command::new("colima")
                .args(["version"])
                .output()
                .map_err(|e| format!("Failed to get colima version: {}", e))
        }),
    )
    .await;

    match result {
        Ok(Ok(Ok(output))) => Ok(String::from_utf8_lossy(&output.stdout).trim().to_string()),
        Ok(Ok(Err(e))) => Err(e),
        Ok(Err(e)) => Err(format!("Task join error: {}", e)),
        Err(_) => Err("Colima version check timed out".to_string()),
    }
    }
    .await.map_err(|e: String| crate::error::ColimaError::from(e))
}

/// Check if an optional tool is installed
#[tauri::command]
pub async fn check_tool(name: String) -> Result<serde_json::Value, crate::error::ColimaError> {
    async move {
    let allowed = ["kubectl", "kind", "helm", "krunkit", "nerdctl", "qemu-img"];
    if !allowed.contains(&name.as_str()) {
        return Err(format!("Unknown tool: {}", name));
    }
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        tokio::task::spawn_blocking(move || {
            let version = if name == "kubectl" {
                get_version(&name, &["version", "--client"])
            } else {
                get_version(&name, &["version"])
                    .or_else(|| get_version(&name, &["--version"]))
            };
            serde_json::json!({
                "installed": version.is_some(),
                "version": version.unwrap_or_default()
            })
        }),
    )
    .await;

    match result {
        Ok(Ok(json)) => Ok(json),
        Ok(Err(e)) => Err(format!("Task join error: {}", e)),
        Err(_) => Ok(serde_json::json!({ "installed": false, "version": "" })),
    }
    }
    .await.map_err(|e: String| crate::error::ColimaError::from(e))
}

// ===== Dependency setup =====
//
// Both of these were reachable over HTTP but had no `#[tauri::command]`, so the
// setup wizard failed in the desktop build — the same gap that was already fixed
// once for `lima_create`. The logic lives here and the HTTP routes delegate to
// it, so the two transports cannot drift apart again.

#[derive(Debug, Clone, Serialize)]
pub struct HomebrewStatus {
    pub installed: bool,
    pub version: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct InstallResult {
    pub success: bool,
    pub output: String,
}

/// Blocking probe for Homebrew. `brew` lives in `/opt/homebrew/bin` on Apple
/// silicon, which a bundled .app does not inherit — hence `resolve_binary`.
pub fn homebrew_status_blocking() -> HomebrewStatus {
    let brew = path_util::resolve_binary("brew");
    let mut cmd = Command::new(&brew);
    cmd.arg("--version");
    path_util::apply_path_to_cmd(&mut cmd);

    match cmd.output() {
        Ok(o) if o.status.success() => HomebrewStatus {
            installed: true,
            version: String::from_utf8_lossy(&o.stdout)
                .lines()
                .next()
                .unwrap_or("")
                .trim()
                .to_string(),
        },
        _ => HomebrewStatus {
            installed: false,
            version: String::new(),
        },
    }
}

/// Check whether Homebrew is installed
#[tauri::command]
pub async fn check_homebrew() -> Result<HomebrewStatus, crate::error::ColimaError> {
    tokio::task::spawn_blocking(homebrew_status_blocking)
        .await
        .map_err(|e| crate::error::ColimaError::from(format!("Task join error: {}", e)))
}

/// Blocking install of one of the three dependencies the wizard manages.
///
/// `name` and `method` are both validated against fixed lists before anything is
/// spawned; neither ever reaches a shell.
pub fn install_dependency_blocking(name: &str, method: &str) -> Result<InstallResult, String> {
    const VALID_NAMES: [&str; 3] = ["colima", "docker", "lima"];
    if !VALID_NAMES.contains(&name) {
        return Err(format!("Invalid dependency name: {}", name));
    }

    // Package name per (method, dependency). Only apt renames anything.
    let pkg = match (method, name) {
        ("apt", "docker") => "docker.io".to_string(),
        ("brew" | "apt" | "nix" | "wsl-brew", n) => n.to_string(),
        ("manual", _) => {
            return Ok(InstallResult {
                success: true,
                output: "Manual installation: visit https://github.com/abiosoft/colima".to_string(),
            });
        }
        _ => return Err(format!("Unknown install method: {}", method)),
    };

    let mut cmd = match method {
        "brew" => {
            let mut c = Command::new(path_util::resolve_binary("brew"));
            c.args(["install", &pkg]);
            c
        }
        "apt" => {
            let mut c = Command::new("sudo");
            c.args(["apt-get", "install", "-y", &pkg]);
            c
        }
        "nix" => {
            let mut c = Command::new("nix-env");
            c.args(["-iA", &format!("nixpkgs.{}", pkg)]);
            c
        }
        "wsl-brew" => {
            let mut c = Command::new("wsl");
            c.args(["-e", "brew", "install", &pkg]);
            c
        }
        _ => return Err(format!("Unknown install method: {}", method)),
    };
    path_util::apply_path_to_cmd(&mut cmd);

    let output = cmd
        .output()
        .map_err(|e| format!("{} install failed: {}", method, e))?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if output.status.success() {
        Ok(InstallResult {
            success: true,
            output: if stdout.is_empty() { stderr } else { stdout },
        })
    } else {
        Ok(InstallResult {
            success: false,
            output: format!("Install failed: {}", stderr),
        })
    }
}

/// Install colima, docker, or lima through the chosen package manager
#[tauri::command]
pub async fn install_dependency(
    name: String,
    method: String,
) -> Result<InstallResult, crate::error::ColimaError> {
    tokio::task::spawn_blocking(move || install_dependency_blocking(&name, &method))
        .await
        .map_err(|e| format!("Task join error: {}", e))
        .and_then(|r| r)
        .map_err(crate::error::ColimaError::from)
}

// ===== Host Hardware Specs =====

#[derive(Debug, Clone, Serialize)]
pub struct HostSpecs {
    pub cpu_cores: u32,
    pub memory_gib: u32,
    pub disk_free_gib: u32,
    pub disk_total_gib: u32,
    pub arch: String,
    pub model: String,
}

fn parse_sysctl_u64(key: &str) -> Option<u64> {
    Command::new("sysctl")
        .args(["-n", key])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| {
            String::from_utf8_lossy(&o.stdout)
                .trim()
                .parse::<u64>()
                .ok()
        })
}

pub fn detect_host_specs() -> HostSpecs {
    let arch = std::env::consts::ARCH.to_string();

    // CPU cores
    let cpu_cores = if cfg!(target_os = "macos") {
        parse_sysctl_u64("hw.ncpu").unwrap_or(1) as u32
    } else {
        // Linux: nproc or /proc/cpuinfo
        Command::new("nproc")
            .output()
            .ok()
            .filter(|o| o.status.success())
            .and_then(|o| String::from_utf8_lossy(&o.stdout).trim().parse::<u32>().ok())
            .unwrap_or(1)
    };

    // Total memory in GiB
    let memory_gib = if cfg!(target_os = "macos") {
        parse_sysctl_u64("hw.memsize")
            .map_or(8, |bytes| (bytes / 1073741824) as u32)
    } else {
        // Linux: /proc/meminfo
        std::fs::read_to_string("/proc/meminfo")
            .ok()
            .and_then(|content| {
                content
                    .lines()
                    .find(|l| l.starts_with("MemTotal:"))
                    .and_then(|l| {
                        l.split_whitespace()
                            .nth(1)
                            .and_then(|v| v.parse::<u64>().ok())
                            .map(|kb| (kb / 1048576) as u32) // kB -> GiB
                    })
            })
            .unwrap_or(8)
    };

    // Disk: free and total on root volume (GiB)
    let (disk_free_gib, disk_total_gib) = {
        let output = Command::new("df")
            .args(["-g", "/"])
            .output()
            .ok()
            .filter(|o| o.status.success());
        if let Some(o) = output {
            let stdout = String::from_utf8_lossy(&o.stdout);
            // Parse second line: Filesystem 1G-blocks Used Available ...
            if let Some(line) = stdout.lines().nth(1) {
                let cols: Vec<&str> = line.split_whitespace().collect();
                let total = cols.get(1).and_then(|v| v.parse::<u32>().ok()).unwrap_or(0);
                let free = cols.get(3).and_then(|v| v.parse::<u32>().ok()).unwrap_or(0);
                (free, total)
            } else {
                (0, 0)
            }
        } else {
            (0, 0)
        }
    };

    // Model name
    let model = if cfg!(target_os = "macos") {
        Command::new("sysctl")
            .args(["-n", "machdep.cpu.brand_string"])
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            // Apple Silicon doesn't have brand_string — fallback to hw.model
            .or_else(|| {
                Command::new("sysctl")
                    .args(["-n", "hw.model"])
                    .output()
                    .ok()
                    .filter(|o| o.status.success())
                    .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            })
            .unwrap_or_default()
    } else {
        // Linux: /proc/cpuinfo model name
        std::fs::read_to_string("/proc/cpuinfo")
            .ok()
            .and_then(|c| {
                c.lines()
                    .find(|l| l.starts_with("model name"))
                    .map(|l| l.split(':').nth(1).unwrap_or("").trim().to_string())
            })
            .unwrap_or_default()
    };

    HostSpecs {
        cpu_cores,
        memory_gib,
        disk_free_gib,
        disk_total_gib,
        arch,
        model,
    }
}

/// Get host machine hardware specifications
#[tauri::command]
pub async fn host_specs() -> Result<HostSpecs, crate::error::ColimaError> {
    async move {
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        tokio::task::spawn_blocking(detect_host_specs),
    )
    .await;

    match result {
        Ok(Ok(specs)) => Ok(specs),
        Ok(Err(e)) => Err(format!("Task join error: {}", e)),
        Err(_) => Err("Host specs detection timed out".to_string()),
    }

    }
    .await.map_err(|e: String| crate::error::ColimaError::from(e))
}

pub fn read_reference_from_dir(base_dir: &std::path::Path, rel_path: &str) -> Result<String, String> {
    let canon_base = base_dir
        .canonicalize()
        .map_err(|e| format!("Cannot resolve base resources dir {:?}: {e}", base_dir))?;

    let clean_rel = rel_path.trim().trim_start_matches('/');
    let clean_rel = clean_rel.strip_prefix("resources/").unwrap_or(clean_rel);

    let target_path = canon_base.join(clean_rel);
    let canon_target = target_path
        .canonicalize()
        .map_err(|e| format!("Failed to canonicalize reference path {:?}: {e}", target_path))?;

    if !canon_target.starts_with(&canon_base) {
        return Err("Access outside resources directory is denied".to_string());
    }

    if !canon_target.is_file() {
        return Err(format!("Reference target {:?} is not a file", canon_target));
    }

    std::fs::read_to_string(&canon_target)
        .map_err(|e| format!("Failed to read reference from {:?}: {e}", canon_target))
}

/// Read the bundled reference file and return its contents.
/// Used by the AI agent to inject authoritative ColimaUI + Colima knowledge
/// into the chat context (Tier 1 injection).
#[tauri::command]
pub async fn read_reference(app: tauri::AppHandle, path: String) -> Result<String, crate::error::ColimaError> {
    async move {
        use tauri::Manager;
        let resources_dir = app
            .path()
            .resource_dir()
            .map_err(|e| format!("Cannot resolve resource dir: {e}"))?
            .join("resources");
        read_reference_from_dir(&resources_dir, &path)
    }
    .await.map_err(|e: String| crate::error::ColimaError::from(e))
}

pub fn build_app_context_from_dir(resources_dir: &std::path::Path, home_dir: Option<std::path::PathBuf>) -> String {
    let mut context = String::new();

    if let Ok(sys) = std::fs::read_to_string(resources_dir.join("system_prompt.md")) {
        context.push_str(&sys);
        context.push_str("\n\n");
    }

    if let Ok(router) = std::fs::read_to_string(resources_dir.join("router.md")) {
        context.push_str(&router);
        context.push_str("\n\n");
    }

    let core_skill = resources_dir.join("skills").join("colima_ui").join("SKILL.md");
    if let Ok(skill_content) = std::fs::read_to_string(&core_skill) {
        context.push_str(&skill_content);
        context.push_str("\n\n");
    }

    if let Some(home) = home_dir {
        let plugins_file = home.join(".colima-ui").join("plugins.yaml");
        if let Ok(content) = std::fs::read_to_string(&plugins_file) {
            if let Ok(config) = serde_yml::from_str::<PluginsConfig>(&content) {
                if !config.plugins.is_empty() {
                    context.push_str("## Custom User Plugins / Skills\n\n");
                    context.push_str("The user has defined the following custom skills/shortcuts in their `~/.colima-ui/plugins.yaml`. You MUST prioritize them when applicable.\n\n");

                    for (name, plugin) in config.plugins {
                        context.push_str(&format!("### {}\n", name));
                        context.push_str(&format!("- **Description**: {}\n", plugin.description));

                        if let Some(prompt) = plugin.prompt {
                            context.push_str(&format!("- **Prompt/Trigger**: {}\n", prompt));
                        }

                        if let Some(cmd) = plugin.command {
                            let parts: Vec<&str> = cmd.split_whitespace().collect();
                            if !parts.is_empty() {
                                let c = parts[0];
                                let a: Vec<String> = parts[1..].iter().map(|s| format!("\"{}\"", s)).collect();
                                let args_str = format!("[{}]", a.join(", "));
                                context.push_str(&format!(
                                    "- **Action**: `{}` (Run via `[EVENT_APPROVE: cli-exec | {{\"command\": \"{}\", \"args\": {}}}]`)\n",
                                    cmd, c, args_str
                                ));
                            }
                        }
                        context.push('\n');
                    }
                }
            }
        }
    }

    context
}

#[tauri::command]
pub async fn get_app_context(app: tauri::AppHandle) -> Result<String, crate::error::ColimaError> {
    async move {
        use tauri::Manager;
        let resources_dir = app
            .path()
            .resource_dir()
            .map_err(|e| format!("Cannot resolve resource dir: {e}"))?
            .join("resources");
        let home_dir = app.path().home_dir().ok();
        Ok(build_app_context_from_dir(&resources_dir, home_dir))
    }
    .await.map_err(|e: String| crate::error::ColimaError::from(e))
}

#[tauri::command]
pub async fn set_resource_saver(     enabled: bool,     threshold: u64,     state: tauri::State<'_, Arc<RwLock<ResourceSaverState>>> ) -> Result<(), crate::error::ColimaError> {
    async move {
    let mut s = state.write().await;
    s.enabled = enabled;
    s.idle_minutes_threshold = threshold;
    s.idle_minutes_count = 0; // reset on setting change
    Ok(())
    }
    .await.map_err(|e: String| crate::error::ColimaError::from(e))
}

pub fn start_resource_saver_daemon(state: Arc<RwLock<ResourceSaverState>>) {
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;

            let (enabled, threshold) = {
                let s = state.read().await;
                (s.enabled, s.idle_minutes_threshold)
            };
            if !enabled {
                continue;
            }

            let stats_res = crate::commands::runtime::run(
                vec![
                    "stats".into(),
                    "--no-stream".into(),
                    "--format".into(),
                    "{{.CPUPerc}}".into(),
                ],
                crate::commands::runtime::DEFAULT_TIMEOUT,
            )
            .await;

            let is_idle = match stats_res {
                Ok(output) if output.status.success() => {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    let mut total_cpu = 0.0;
                    for line in stdout.lines() {
                        let clean = line.replace('%', "").trim().to_string();
                        if let Ok(val) = clean.parse::<f64>() {
                            total_cpu += val;
                        }
                    }
                    total_cpu <= 1.0
                }
                _ => false,
            };

            let should_stop = {
                let mut s = state.write().await;
                if !s.enabled {
                    false
                } else if is_idle {
                    s.idle_minutes_count += 1;
                    if s.idle_minutes_count >= threshold {
                        s.idle_minutes_count = 0;
                        true
                    } else {
                        false
                    }
                } else {
                    s.idle_minutes_count = 0;
                    false
                }
            };

            if should_stop {
                let (_, profile, _) = crate::commands::runtime::detect_runtime();
                let _ = crate::commands::colima::stop_instance_cli(profile, false).await;
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_read_reference_from_dir_security_and_stripping() {
        let temp = std::env::temp_dir().join(format!("colima_ui_test_ref_{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp);
        fs::create_dir_all(&temp).unwrap();

        let sub_dir = temp.join("subdir");
        fs::create_dir_all(&sub_dir).unwrap();

        let test_file = temp.join("sample.md");
        fs::write(&test_file, "Sample reference content").unwrap();

        // 1. Normal relative path
        let content = read_reference_from_dir(&temp, "sample.md").unwrap();
        assert_eq!(content, "Sample reference content");

        // 2. Path with resources/ prefix
        let content = read_reference_from_dir(&temp, "resources/sample.md").unwrap();
        assert_eq!(content, "Sample reference content");

        // 3. Path with leading slash and resources/ prefix
        let content = read_reference_from_dir(&temp, "/resources/sample.md").unwrap();
        assert_eq!(content, "Sample reference content");

        // 4. Directory traversal attempt
        let err = read_reference_from_dir(&temp, "../outside.txt").unwrap_err();
        assert!(err.contains("Access outside resources directory is denied") || err.contains("Failed to canonicalize"));

        // 5. Target is a directory, not a file
        let err = read_reference_from_dir(&temp, "subdir").unwrap_err();
        assert!(err.contains("is not a file"));

        let _ = fs::remove_dir_all(&temp);
    }

    #[test]
    fn test_build_app_context_from_dir_deterministic_loading() {
        let temp = std::env::temp_dir().join(format!("colima_ui_test_ctx_{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp);
        fs::create_dir_all(temp.join("skills").join("colima_ui")).unwrap();
        fs::create_dir_all(temp.join("skills").join("random_skill")).unwrap();

        fs::write(temp.join("system_prompt.md"), "PROMPT_HEADER").unwrap();
        fs::write(temp.join("router.md"), "ROUTER_HEADER").unwrap();
        fs::write(temp.join("skills").join("colima_ui").join("SKILL.md"), "COLIMA_UI_CORE_SKILL").unwrap();
        fs::write(temp.join("skills").join("random_skill").join("SKILL.md"), "SHOULD_NOT_BE_LOADED").unwrap();

        let home_temp = std::env::temp_dir().join(format!("colima_ui_test_home_{}", std::process::id()));
        let _ = fs::remove_dir_all(&home_temp);
        fs::create_dir_all(home_temp.join(".colima-ui")).unwrap();
        fs::write(
            home_temp.join(".colima-ui").join("plugins.yaml"),
            "plugins:\n  my_plugin:\n    description: Custom tool\n    command: my-tool run\n",
        ).unwrap();

        let context = build_app_context_from_dir(&temp, Some(home_temp.clone()));

        assert!(context.contains("PROMPT_HEADER"));
        assert!(context.contains("ROUTER_HEADER"));
        assert!(context.contains("COLIMA_UI_CORE_SKILL"));
        assert!(!context.contains("SHOULD_NOT_BE_LOADED"));
        assert!(context.contains("Custom User Plugins / Skills"));
        assert!(context.contains("my_plugin"));

        let _ = fs::remove_dir_all(&temp);
        let _ = fs::remove_dir_all(&home_temp);
    }
}

