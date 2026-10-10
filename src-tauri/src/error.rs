//! The error contract shared by both entry points (Tauri IPC and the HTTP API).
//!
//! # Why this is a struct and not an enum
//!
//! The previous shape was `#[serde(tag = "type", content = "message")]` over
//! newtype variants carrying a `String`. Two problems:
//!
//!   1. It could not carry structure. Adding a field to a variant turns
//!      `message` from a string into an object, and every consumer that renders
//!      `message` starts printing `[object Object]`.
//!   2. It was not actually used. Of ~130 construction sites, 108 went through
//!      `ColimaError::from(String)`, i.e. everything landed in `Unknown`. The
//!      type said "classified error" while carrying an unclassified string.
//!
//! So classification happens *here*, inside `From<String>`. Every existing
//! `ColimaError::from(...)` call site gains a real error code without being
//! touched, and new context (the command that failed, its exit code) can be
//! layered on where it is available via [`ColimaError::with_command`].
//!
//! # Who owns the user-facing wording
//!
//! Rust emits a stable `code` plus technical `detail`. The frontend maps `code`
//! to localized text through `src/lib/i18n.svelte.ts` — it must not be Rust's
//! job to pick the user's language. `hint` is an English fallback for surfaces
//! that have no i18n (the CLI binaries, logs); the frontend prefers its own
//! translation and falls back to `hint`, then `detail`.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Stable, machine-readable classification. Frontends switch on this, so
/// renaming a variant is a breaking change to the API contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    /// A required CLI tool is not installed or not on PATH.
    NotInstalled,
    /// The tool is installed but the VM/daemon is not running.
    NotRunning,
    /// The OS refused the operation.
    PermissionDenied,
    /// The operation exceeded its deadline.
    Timeout,
    /// A CLI command ran and returned a non-zero exit status.
    CommandFailed,
    /// A network operation failed.
    Network,
    /// Input rejected before anything was executed.
    Validation,
    /// The requested resource does not exist.
    NotFound,
    /// A fault inside this application.
    Internal,
    /// Unclassified.
    Unknown,
}

/// The error payload returned to both the Tauri frontend and HTTP clients.
#[derive(Debug, Clone, Serialize)]
pub struct ColimaError {
    pub code: ErrorCode,
    /// Technical description. Already redacted — see [`crate::redact`].
    pub detail: String,
    /// The command that failed, when known (e.g. `colima start --profile dev`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    /// English fallback remediation text. The frontend prefers its own
    /// translation keyed on `code`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
    /// Knowledge Base article slug for "how do I fix this".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doc_id: Option<String>,
}

impl ColimaError {
    /// Build an error with an explicit code, skipping message classification.
    pub fn new(code: ErrorCode, detail: impl Into<String>) -> Self {
        let detail = crate::redact::redact(&detail.into());
        let (hint, doc_id) = remediation_for(code);
        Self {
            code,
            detail,
            command: None,
            exit_code: None,
            hint,
            doc_id,
        }
    }

    pub fn validation(detail: impl Into<String>) -> Self {
        Self::new(ErrorCode::Validation, detail)
    }

    pub fn not_found(detail: impl Into<String>) -> Self {
        Self::new(ErrorCode::NotFound, detail)
    }

    pub fn internal(detail: impl Into<String>) -> Self {
        Self::new(ErrorCode::Internal, detail)
    }

    pub fn network(detail: impl Into<String>) -> Self {
        Self::new(ErrorCode::Network, detail)
    }

    pub fn command_failed(detail: impl Into<String>) -> Self {
        Self::new(ErrorCode::CommandFailed, detail)
    }

    /// Attach the command that produced this error.
    pub fn with_command(mut self, command: impl Into<String>) -> Self {
        self.command = Some(crate::redact::redact(&command.into()));
        self
    }

    /// Attach the exit status of the command that produced this error.
    pub fn with_exit_code(mut self, code: i32) -> Self {
        self.exit_code = Some(code);
        self
    }
}

/// Classify a raw error message.
///
/// Patterns are matched against lowercased text and ordered most-specific
/// first: "cannot connect to the docker daemon" is a `NotRunning` signal even
/// though it also contains the word "connect", which would otherwise read as a
/// network fault.
pub fn classify(message: &str) -> ErrorCode {
    let m = message.to_lowercase();

    const NOT_RUNNING: &[&str] = &[
        "cannot connect to the docker daemon",
        "is the docker daemon running",
        "colima is not running",
        "instance is not running",
        "no such instance",
        "cannot connect to the kubernetes",
        "the connection to the server",
    ];
    const NOT_INSTALLED: &[&str] = &[
        "command not found",
        "executable file not found",
        "no such file or directory (os error 2)",
        "is not installed",
        "not found in path",
    ];
    const PERMISSION: &[&str] = &[
        "permission denied",
        "operation not permitted",
        "eacces",
        "access is denied",
        "requires root",
    ];
    // Deliberately not the bare word "timeout": a message about a `--timeout`
    // flag is not a timeout, and misclassifying it advises restarting the VM.
    const TIMEOUT: &[&str] = &["timed out", "deadline exceeded", "context deadline"];
    // No bare "not found" either — "image not found, pulling…" is progress
    // output, not a failure to locate something the user asked for.
    const NOT_FOUND: &[&str] = &[
        "no such container",
        "no such image",
        "no such volume",
        "no such network",
        "no such object",
        "container not found",
        "image not found:",
        "volume not found",
        "network not found",
    ];
    const NETWORK: &[&str] = &[
        "connection refused",
        "network is unreachable",
        "dns error",
        "dial tcp",
        "could not resolve host",
        // Specific TLS failures only — the bare word "certificate" also appears
        // in configuration errors that have nothing to do with the network.
        "x509",
        "certificate verify",
        "certificate has expired",
        "unable to verify",
    ];

    // Order matters — see the doc comment.
    for (patterns, code) in [
        (NOT_RUNNING, ErrorCode::NotRunning),
        (NOT_INSTALLED, ErrorCode::NotInstalled),
        (PERMISSION, ErrorCode::PermissionDenied),
        (TIMEOUT, ErrorCode::Timeout),
        (NOT_FOUND, ErrorCode::NotFound),
        (NETWORK, ErrorCode::Network),
    ] {
        if patterns.iter().any(|p| m.contains(p)) {
            return code;
        }
    }

    ErrorCode::Unknown
}

/// English fallback hint + Knowledge Base slug for a code.
///
/// The KB slugs point at articles created in the "Colima Config & Knowledge
/// Base" phase; a slug that has no article yet simply renders no link.
fn remediation_for(code: ErrorCode) -> (Option<String>, Option<String>) {
    let (hint, doc) = match code {
        ErrorCode::NotRunning => (
            "The Colima VM is not running. Start it from the Instances page.",
            "start-colima",
        ),
        ErrorCode::NotInstalled => (
            "A required command-line tool is missing. Check the Setup page for install instructions.",
            "install-colima",
        ),
        ErrorCode::PermissionDenied => (
            "The operating system refused this operation. Check file ownership and permissions.",
            "common-errors",
        ),
        ErrorCode::Timeout => (
            "The operation took too long. The VM may be busy or unresponsive — try restarting it.",
            "common-errors",
        ),
        ErrorCode::Network => (
            "A network request failed. Check connectivity and any proxy settings.",
            "common-errors",
        ),
        ErrorCode::NotFound => return (None, None),
        ErrorCode::Validation => return (None, None),
        ErrorCode::CommandFailed | ErrorCode::Internal | ErrorCode::Unknown => return (None, None),
    };
    (Some(hint.to_string()), Some(doc.to_string()))
}

impl fmt::Display for ColimaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.detail)
    }
}

impl std::error::Error for ColimaError {}

/// Classifying conversion. This is why the ~108 existing `ColimaError::from`
/// call sites did not need to change when the contract gained structure.
impl From<String> for ColimaError {
    fn from(s: String) -> Self {
        Self::new(classify(&s), s)
    }
}

impl From<&str> for ColimaError {
    fn from(s: &str) -> Self {
        Self::from(s.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_real_stderr_samples() {
        let cases = [
            (
                "Cannot connect to the Docker daemon at unix:///var/run/docker.sock",
                ErrorCode::NotRunning,
            ),
            ("colima: command not found", ErrorCode::NotInstalled),
            (
                "Error response from daemon: No such container: web",
                ErrorCode::NotFound,
            ),
            (
                "open /Users/x/.colima/default/colima.yaml: permission denied",
                ErrorCode::PermissionDenied,
            ),
            (
                "Docker command timed out (daemon may be unresponsive)",
                ErrorCode::Timeout,
            ),
            // A message that merely mentions a timeout setting is not a timeout.
            (
                "invalid value for --timeout: expected a duration",
                ErrorCode::Unknown,
            ),
            // Progress output, not a failure to find something.
            ("image not found, pulling from registry", ErrorCode::Unknown),
            (
                "dial tcp 127.0.0.1:6443: connect: connection refused",
                ErrorCode::Network,
            ),
            ("something entirely unexpected", ErrorCode::Unknown),
        ];
        for (msg, expected) in cases {
            assert_eq!(classify(msg), expected, "misclassified: {msg}");
        }
    }

    #[test]
    fn not_running_wins_over_network_for_daemon_socket_errors() {
        // Contains "connect", which would otherwise look like a network fault.
        assert_eq!(
            classify("Cannot connect to the Docker daemon. Is the docker daemon running?"),
            ErrorCode::NotRunning
        );
    }

    #[test]
    fn from_string_classifies_without_call_site_changes() {
        let e = ColimaError::from("Cannot connect to the Docker daemon".to_string());
        assert_eq!(e.code, ErrorCode::NotRunning);
        assert!(e.hint.is_some(), "NotRunning should carry a hint");
        assert_eq!(e.doc_id.as_deref(), Some("start-colima"));
    }

    #[test]
    fn detail_is_redacted_on_construction() {
        let e = ColimaError::from(
            "Request failed: https://x.test/v1?key=AIzaSy0123456789abcdefghijkl".to_string(),
        );
        assert!(
            !e.detail.contains("AIzaSy0123456789abcdefghijkl"),
            "secret leaked into detail: {}",
            e.detail
        );
    }

    #[test]
    fn context_builders_attach_and_redact() {
        let e = ColimaError::command_failed("boom")
            .with_command("curl https://x.test?token=AIzaSy0123456789abcdefghijkl")
            .with_exit_code(1);
        assert_eq!(e.exit_code, Some(1));
        let cmd = e.command.expect("command attached");
        assert!(!cmd.contains("AIzaSy0123456789abcdefghijkl"), "secret leaked: {cmd}");
    }

    #[test]
    fn serializes_with_a_stable_snake_case_code_and_omits_empty_context() {
        let json = serde_json::to_string(&ColimaError::validation("bad name")).expect("serialize");
        assert!(json.contains(r#""code":"validation""#), "unexpected shape: {json}");
        assert!(json.contains(r#""detail":"bad name""#), "unexpected shape: {json}");
        // Absent context must not appear at all, so the frontend can use
        // presence as a signal.
        assert!(!json.contains("command"), "empty command should be omitted: {json}");
        assert!(!json.contains("exit_code"), "empty exit_code should be omitted: {json}");
    }
}
