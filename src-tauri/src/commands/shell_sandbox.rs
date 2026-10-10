use serde::{Deserialize, Serialize};
use std::process::Command;

// ===== 3-Tier Safety Classification =====

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CommandSafety {
    Safe,           // Auto-run: read-only diagnostics
    NeedsApproval,  // Show approval dialog to user
    Banned,         // NEVER execute — reject immediately
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassifyResult {
    pub safety: CommandSafety,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub truncated: bool,
}

// ===== Safety Classification =====

const MAX_OUTPUT_BYTES: usize = 10240; // 10KB max output
const EXEC_TIMEOUT_SECS: u64 = 30;

/// Banned patterns — NEVER allow these regardless of context
const BANNED_PATTERNS: &[&str] = &[
    "rm -rf", "rm -r", "rm -fr",
    "sudo ",
    "chmod ", "chown ",
    "mkfs", "fdisk", "dd ",
    "eval ", "exec ",
    "> ", ">> ",     // file write redirects
    "tee ",
    "sed -i",        // in-place edit
    "mv ", "cp ",    // file manipulation
    "pip install", "npm install", "yarn add",
    "bash -c", "sh -c", "zsh -c",
    "python -c", "python3 -c",
    "curl -X POST", "curl -X PUT", "curl -X DELETE",
    "curl -d ", "curl --data",
    "wget -O",
];

/// Banned shell metacharacters — prevent command injection
const BANNED_CHARS: &[char] = &[';', '`', '$'];
const BANNED_SEQUENCES: &[&str] = &["&&", "||", "$(", "${"];

/// Safe command prefixes — auto-run without approval
const SAFE_PREFIXES: &[&str] = &[
    // Process inspection
    "ps ", "pgrep ", "top -l",
    // File reading (read-only)
    "cat ", "tail ", "head ", "wc ", "ls ", "find ", "file ",
    "grep ", "awk ", "sort ", "uniq ", "cut ",
    // System info
    "uname", "sw_vers", "sysctl ", "df ", "du ", "uptime", "whoami", "id ",
    "hostnamectl", "arch",
    // Colima read-only
    "colima status", "colima list", "colima version", "colima nerdctl ls",
    // Docker read-only
    "docker ps", "docker images", "docker info", "docker logs ",
    "docker inspect ", "docker stats", "docker version",
    "docker compose ps", "docker compose ls",
    "docker network ls", "docker network inspect",
    "docker volume ls", "docker volume inspect",
    // Lima read-only
    "limactl list", "limactl info",
    // Kubernetes read-only
    "kubectl get ", "kubectl describe ", "kubectl logs ",
    "kubectl version", "kubectl cluster-info", "kubectl config ",
    "kubectl top ",
    // Network diagnostics
    "ping -c", "netstat ", "lsof -i", "lsof -P",
    "curl -s", "curl --silent", "curl -I",
    // Brew info
    "brew list", "brew info", "brew --version", "brew --prefix",
    // Misc
    "which ", "type ", "command -v",
    "date", "echo ",
];

/// Approve-required command prefixes — need user confirmation
const APPROVE_PREFIXES: &[&str] = &[
    // Colima state changes
    "colima start", "colima stop", "colima restart", "colima delete",
    "docker start ", "docker stop ", "docker restart ", "docker pull ",
    "docker rm ", "docker rmi ", "docker system prune",
    "docker volume rm", "docker network rm",
    "docker compose up", "docker compose down", "docker compose restart",
    // Process management
    "pkill ", "kill ", "killall ",
    // Kubernetes state changes
    "kubectl delete ", "kubectl apply ", "kubectl scale ",
    "kubectl rollout ",
    // Brew package management
    "brew install ", "brew upgrade ", "brew uninstall ",
    // Special: only docker.sock removal is allowed
    "rm /var/run/docker.sock",
    "rm -f /var/run/docker.sock",
];

/// Classify a command into Safe/NeedsApproval/Banned
fn classify(command: &str) -> ClassifyResult {
    let cmd = command.trim();

    // Check banned chars/sequences first (injection prevention)
    for &ch in BANNED_CHARS {
        if cmd.contains(ch) {
            return ClassifyResult {
                safety: CommandSafety::Banned,
                reason: format!("Command contains forbidden character '{}'", ch),
            };
        }
    }
    for &seq in BANNED_SEQUENCES {
        if cmd.contains(seq) {
            return ClassifyResult {
                safety: CommandSafety::Banned,
                reason: format!("Command chaining '{}' is not allowed", seq),
            };
        }
    }

    // Check banned patterns
    let cmd_lower = cmd.to_lowercase();
    for &pattern in BANNED_PATTERNS {
        if cmd_lower.contains(&pattern.to_lowercase()) {
            return ClassifyResult {
                safety: CommandSafety::Banned,
                reason: format!("'{}' is a banned command pattern", pattern),
            };
        }
    }

    // Check if rm is used on anything other than docker.sock
    if cmd_lower.starts_with("rm ") && !cmd.contains("/var/run/docker.sock") {
        return ClassifyResult {
            safety: CommandSafety::Banned,
            reason: "rm is only allowed for /var/run/docker.sock".to_string(),
        };
    }

    // Check safe prefixes
    for &prefix in SAFE_PREFIXES {
        if cmd.starts_with(prefix) || cmd == prefix.trim() {
            return ClassifyResult {
                safety: CommandSafety::Safe,
                reason: format!("Read-only diagnostic command (matches '{}')", prefix.trim()),
            };
        }
    }

    // Check approve-required prefixes
    for &prefix in APPROVE_PREFIXES {
        if cmd.starts_with(prefix) || cmd == prefix.trim() {
            return ClassifyResult {
                safety: CommandSafety::NeedsApproval,
                reason: format!("State-changing command requires approval (matches '{}')", prefix.trim()),
            };
        }
    }

    // Default: anything not explicitly whitelisted is banned
    ClassifyResult {
        safety: CommandSafety::Banned,
        reason: "Command not in whitelist — only explicitly allowed commands can run".to_string(),
    }
}

/// Execute a command with safety checks, timeout, and output truncation
fn execute_command(command: &str) -> Result<ExecResult, String> {
    let parts: Vec<&str> = command.split_whitespace().collect();
    if parts.is_empty() {
        return Err("Empty command".to_string());
    }

    let mut child = Command::new(parts[0])
        .args(&parts[1..])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to spawn '{}': {}", parts[0], e))?;

    // Fix #9: Actually enforce the timeout (was declared but never used)
    let timeout = std::time::Duration::from_secs(EXEC_TIMEOUT_SECS);
    let start = std::time::Instant::now();

    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,  // Process exited
            Ok(None) => {
                if start.elapsed() > timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(format!(
                        "Command timed out after {}s and was killed",
                        EXEC_TIMEOUT_SECS
                    ));
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            Err(e) => return Err(format!("Error waiting for process: {}", e)),
        }
    }

    let output = child
        .wait_with_output()
        .map_err(|e| format!("Command execution error: {}", e))?;

    let mut stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let mut stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let mut truncated = false;

    if stdout.len() > MAX_OUTPUT_BYTES {
        stdout = stdout[..MAX_OUTPUT_BYTES].to_string();
        stdout.push_str("\n...[output truncated]");
        truncated = true;
    }
    if stderr.len() > MAX_OUTPUT_BYTES {
        stderr = stderr[..MAX_OUTPUT_BYTES].to_string();
        stderr.push_str("\n...[output truncated]");
        truncated = true;
    }

    Ok(ExecResult {
        stdout,
        stderr,
        exit_code: output.status.code().unwrap_or(-1),
        truncated,
    })
}

// ===== Tauri Commands =====

/// Classify a command's safety level (for UI display)
#[tauri::command]
pub async fn sandbox_classify(command: String) -> Result<ClassifyResult, crate::error::ColimaError> {
    async move {
    Ok(classify(&command))
    }
    .await.map_err(|e: String| crate::error::ColimaError::from(e))
}

/// Execute a safe command (auto-approved)
#[tauri::command]
pub async fn sandbox_execute(command: String) -> Result<ExecResult, crate::error::ColimaError> {
    async move {
    let result = classify(&command);
    match result.safety {
        CommandSafety::Safe => {
            tokio::task::spawn_blocking(move || execute_command(&command))
                .await
                .map_err(|e| format!("Task join error: {}", e))?
        }
        CommandSafety::NeedsApproval => {
            Err(format!("needs_approval:{}", result.reason))
        }
        CommandSafety::Banned => {
            Err(format!("banned:{}", result.reason))
        }
    }
    }
    .await.map_err(|e: String| crate::error::ColimaError::from(e))
}

/// Execute a command that was explicitly approved by the user
#[tauri::command]
pub async fn sandbox_execute_approved(command: String) -> Result<ExecResult, crate::error::ColimaError> {
    async move {
    let result = classify(&command);
    match result.safety {
        CommandSafety::Banned => {
            Err(format!("banned:{}", result.reason))
        }
        _ => {
            // Safe or NeedsApproval — both OK when user approved
            tokio::task::spawn_blocking(move || execute_command(&command))
                .await
                .map_err(|e| format!("Task join error: {}", e))?
        }
    }
    }
    .await.map_err(|e: String| crate::error::ColimaError::from(e))
}
