//! Security validation utilities.
//!
//! Input sanitization for shell commands, container IDs, Kubernetes resource names,
//! and AppleScript injection prevention.

/// Banned shell metacharacters for container/VM exec commands (Fix #2)
pub fn contains_shell_injection(input: &str) -> bool {
    let banned_chars = [';', '`', '$'];
    let banned_seqs = ["&&", "||", "$(", "${", "..", ">/"];
    for ch in &banned_chars {
        if input.contains(*ch) {
            return true;
        }
    }
    for seq in &banned_seqs {
        if input.contains(seq) {
            return true;
        }
    }
    false
}

/// Validate container_id format (hex string, 1-128 chars)
pub fn is_valid_container_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id.chars().all(|c| {
            c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.' || c == '/' || c == ':'
        })
}

/// Dangerous docker run flags that could lead to container escape (Fix #10)
pub const BANNED_DOCKER_FLAGS: &[&str] = &[
    "--privileged",
    "--pid=host",
    "--pid",
    "host",
    "--net=host",
    "--network=host",
    "--cap-add=SYS_ADMIN",
    "--cap-add=ALL",
    "--security-opt=apparmor:unconfined",
    "--security-opt=seccomp:unconfined",
];

/// Whitelist of resource types allowed for deletion (Fix #5)
pub const K8S_DELETABLE_RESOURCES: &[&str] = &[
    "pods",
    "pod",
    "deployments",
    "deployment",
    "services",
    "service",
    "configmaps",
    "configmap",
    "secrets",
    "secret",
    "statefulsets",
    "statefulset",
    "daemonsets",
    "daemonset",
    "replicasets",
    "replicaset",
    "jobs",
    "job",
    "cronjobs",
    "cronjob",
    "ingresses",
    "ingress",
    "persistentvolumeclaims",
    "pvc",
    "endpoints",
    "serviceaccounts",
    "serviceaccount",
    "networkpolicies",
    "networkpolicy",
    "poddisruptionbudgets",
    "pdb",
    "horizontalpodautoscalers",
    "hpa",
];

/// Validate a Kubernetes resource name (namespace, pod, container)
/// Must be alphanumeric, hyphens, dots, underscores only.
pub fn is_valid_k8s_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 253
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.' || c == '_')
}

/// Escape a string for embedding inside AppleScript double-quoted strings (Fix #3).
/// Escapes backslashes and double quotes to prevent injection.
pub fn escape_applescript(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}
