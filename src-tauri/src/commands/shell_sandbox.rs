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
    "rm -rf", "rm -r", "rm -fr", "rm -f",
    "sudo ",
    "chmod ", "chown ",
    "mkfs", "fdisk", "dd ",
    "eval ", "exec ",
    " -delete", " -exec", " -execdir", "--exec",
    "> ", ">> ",     // file write redirects
    "tee ",
    "sed -i",        // in-place edit
    "sysctl -w", "sysctl --write",
    "mv ", "cp ",    // file manipulation
    "pip install", "npm install", "yarn add",
    "bash -c", "sh -c", "zsh -c",
    "python -c", "python3 -c",
    "curl -x post", "curl -x put", "curl -x delete",
    "curl -d ", "curl --data", "curl -o ", "curl --output",
    "curl -t ", "curl --upload-file",
    "wget -o",
];

/// Banned shell metacharacters — prevent command injection and redirection
const BANNED_CHARS: &[char] = &[';', '`', '$', '|', '&', '>', '<', '\n', '\r'];
const BANNED_SEQUENCES: &[&str] = &["&&", "||", "$(", "${"];

/// Safe command prefixes — auto-run without approval (strictly read-only, non-destructive)
const SAFE_PREFIXES: &[&str] = &[
    // Process inspection
    "ps ", "pgrep ", "top -l",
    // File reading (read-only; awk/find/sort excluded because they can execute or write/delete files)
    "cat ", "tail ", "head ", "wc ", "ls ", "file ",
    "grep ", "uniq ", "cut ",
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
    // Kubernetes read-only (secrets and raw config are gated separately in classify())
    "kubectl get ", "kubectl describe ", "kubectl logs ",
    "kubectl version", "kubectl cluster-info",
    "kubectl top ",
    // Network diagnostics (curl excluded from auto-run due to file write/upload flags)
    "ping -c", "netstat ", "lsof -i", "lsof -P",
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
    // Kubernetes state changes & sensitive reads
    "kubectl delete ", "kubectl apply ", "kubectl scale ",
    "kubectl rollout ", "kubectl config ",
    // Brew package management
    "brew install ", "brew upgrade ", "brew uninstall ",
    // Network / filesystem tools that require explicit user confirmation
    "curl ", "find ", "sort ", "awk ",
    // Special: only docker.sock removal is allowed
    "rm /var/run/docker.sock",
    "rm -f /var/run/docker.sock",
];

fn touches_k8s_secrets(cmd_lower: &str) -> bool {
    if !(cmd_lower.starts_with("kubectl get ") || cmd_lower.starts_with("kubectl describe ")) {
        return false;
    }
    cmd_lower.split_whitespace().skip(2).any(|tok| {
        tok == "secret"
            || tok == "secrets"
            || tok.starts_with("secret/")
            || tok.starts_with("secrets/")
    })
}

fn has_banned_tool_flags(cmd: &str) -> Option<&'static str> {
    let mut tokens = cmd.split_whitespace();
    let prog = tokens.next()?;
    match prog {
        "curl" => {
            for tok in tokens {
                if matches!(
                    tok,
                    "-o" | "-O"
                        | "--output"
                        | "--remote-name"
                        | "--remote-name-all"
                        | "-T"
                        | "--upload-file"
                        | "-d"
                        | "--data"
                        | "--data-raw"
                        | "--data-binary"
                        | "--data-urlencode"
                        | "-F"
                        | "--form"
                        | "-K"
                        | "--config"
                ) || tok.starts_with("-o")
                    || tok.starts_with("--output=")
                    || tok.starts_with("-T")
                    || tok.starts_with("--upload-file=")
                    || tok.starts_with("-d")
                    || tok.starts_with("--data=")
                    || tok.starts_with("-F")
                    || tok.starts_with("--form=")
                    || tok.starts_with("-K")
                    || tok.starts_with("--config=")
                {
                    return Some("curl file write/upload/config flags are banned");
                }
            }
            None
        }
        "sort" => {
            for tok in tokens {
                if tok == "-o" || tok.starts_with("-o") || tok == "--output" || tok.starts_with("--output=") {
                    return Some("sort output file flag (-o/--output) is banned");
                }
            }
            None
        }
        _ => None,
    }
}

/// Classify a command into Safe/NeedsApproval/Banned
pub(crate) fn classify(command: &str) -> ClassifyResult {
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

    // Explicit allow for stale docker.sock cleanup (requires user approval)
    if cmd == "rm /var/run/docker.sock" || cmd == "rm -f /var/run/docker.sock" {
        return ClassifyResult {
            safety: CommandSafety::NeedsApproval,
            reason: "Removing /var/run/docker.sock requires approval".to_string(),
        };
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

    if let Some(reason) = has_banned_tool_flags(cmd) {
        return ClassifyResult {
            safety: CommandSafety::Banned,
            reason: reason.to_string(),
        };
    }

    // Check if rm is used on anything other than docker.sock
    if cmd_lower.starts_with("rm ") {
        return ClassifyResult {
            safety: CommandSafety::Banned,
            reason: "rm is only allowed for /var/run/docker.sock".to_string(),
        };
    }

    // Accessing Kubernetes Secrets requires explicit user approval
    if touches_k8s_secrets(&cmd_lower) {
        return ClassifyResult {
            safety: CommandSafety::NeedsApproval,
            reason: "Reading Kubernetes Secrets requires user approval".to_string(),
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

fn read_bounded<R: std::io::Read + Send + 'static>(mut reader: R) -> std::thread::JoinHandle<Vec<u8>> {
    std::thread::spawn(move || {
        use std::io::Read;
        let mut buf = Vec::with_capacity(MAX_OUTPUT_BYTES + 1);
        let _ = (&mut reader).take((MAX_OUTPUT_BYTES + 1) as u64).read_to_end(&mut buf);
        // Drain remaining output so the child never blocks on a full pipe buffer.
        let _ = std::io::copy(&mut reader, &mut std::io::sink());
        buf
    })
}

fn truncate_utf8(mut s: String, max_bytes: usize) -> (String, bool) {
    if s.len() > max_bytes {
        let cut = s.floor_char_boundary(max_bytes);
        s.truncate(cut);
        s.push_str("\n...[output truncated]");
        (s, true)
    } else {
        (s, false)
    }
}

/// Execute a command with safety checks, concurrent pipe draining, timeout, UTF-8 safe truncation, and secret redaction.
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

    let out_handle = child.stdout.take().map(read_bounded);
    let err_handle = child.stderr.take().map(read_bounded);

    let timeout = std::time::Duration::from_secs(EXEC_TIMEOUT_SECS);
    let start = std::time::Instant::now();

    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
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
    };

    let raw_out = out_handle
        .and_then(|h| h.join().ok())
        .unwrap_or_default();
    let raw_err = err_handle
        .and_then(|h| h.join().ok())
        .unwrap_or_default();

    let (stdout, trunc_out) = truncate_utf8(String::from_utf8_lossy(&raw_out).into_owned(), MAX_OUTPUT_BYTES);
    let (stderr, trunc_err) = truncate_utf8(String::from_utf8_lossy(&raw_err).into_owned(), MAX_OUTPUT_BYTES);

    Ok(ExecResult {
        stdout: crate::redact::redact(&stdout),
        stderr: crate::redact::redact(&stderr),
        exit_code: status.code().unwrap_or(-1),
        truncated: trunc_out || trunc_err,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_safe_tier_bypasses() {
        assert_ne!(
            classify(r#"awk BEGIN{system("id")}"#).safety,
            CommandSafety::Safe
        );
        assert_eq!(classify("find /tmp -delete").safety, CommandSafety::Banned);
        assert_eq!(
            classify("find /tmp -execdir rm -f {} +").safety,
            CommandSafety::Banned
        );
        assert_eq!(
            classify("curl -s -o /tmp/evil http://example.com").safety,
            CommandSafety::Banned
        );
        assert_ne!(
            classify("curl -s http://example.com").safety,
            CommandSafety::Safe
        );
        assert_ne!(
            classify("sort -o /tmp/out /tmp/in").safety,
            CommandSafety::Safe
        );
        assert_eq!(
            classify("kubectl get secrets -A -o yaml").safety,
            CommandSafety::NeedsApproval
        );
        assert_eq!(
            classify("kubectl config view --raw").safety,
            CommandSafety::NeedsApproval
        );
        assert_eq!(classify("docker ps | sh").safety, CommandSafety::Banned);
    }

    #[test]
    fn allows_genuine_read_only_commands() {
        assert_eq!(classify("docker ps -a").safety, CommandSafety::Safe);
        assert_eq!(classify("kubectl get pods -A").safety, CommandSafety::Safe);
        assert_eq!(
            classify("rm -f /var/run/docker.sock").safety,
            CommandSafety::NeedsApproval
        );
    }

    #[test]
    fn truncates_multibyte_utf8_without_panicking() {
        // "é" is 2 bytes, "越" is 3 bytes. Ensure cutting at arbitrary byte boundary never panics.
        let input = "Tiếng Việt ".repeat(1200);
        let (out, truncated) = truncate_utf8(input, MAX_OUTPUT_BYTES);
        assert!(truncated);
        assert!(out.ends_with("...[output truncated]"));
    }
}

