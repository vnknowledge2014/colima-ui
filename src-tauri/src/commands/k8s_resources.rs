//! Kubernetes commands that act on resources inside a cluster.
//!
//! Every command here existed only as an HTTP route. `call()` in the frontend
//! invokes a Tauri command in the desktop app and falls back to HTTP in browser
//! mode, so the whole group worked in the browser and failed in the app with
//! "Command <name> not found". Cluster-scoped operations live in
//! [`crate::commands::k8s_cluster`].
//!
//! Names are validated before they reach argv. They are argv elements rather
//! than shell input, so this is not an injection guard so much as a way to
//! fail in the UI with a readable message instead of in kubectl's stderr.

use crate::error::ColimaError;
use crate::helpers::{run_blocking, run_cmd};
use crate::validation::{is_valid_k8s_name, K8S_DELETABLE_RESOURCES};

fn ensure_name(label: &str, value: &str) -> Result<(), String> {
    if !is_valid_k8s_name(value) {
        return Err(format!("Invalid {label}: {value}"));
    }
    Ok(())
}

/// An empty namespace, or the literal "all", means cluster-wide. Only a
/// concrete namespace is validated — the other two are sentinels the callers
/// already use.
fn ensure_optional_namespace(ns: &str) -> Result<(), String> {
    if ns.is_empty() || ns == "all" {
        return Ok(());
    }
    ensure_name("namespace", ns)
}

#[tauri::command]
pub async fn k8s_apply(yaml: String, namespace: String) -> Result<String, ColimaError> {
    async move {
        ensure_optional_namespace(&namespace)?;
        run_blocking(move || {
            use std::io::Write;
            use std::process::{Command, Stdio};

            // The manifest goes in on stdin, never on the command line: it is
            // arbitrary multi-line user content.
            let mut args = vec!["apply", "-f", "-"];
            if !namespace.is_empty() && namespace != "all" {
                args.push("-n");
                args.push(&namespace);
            }

            let mut child = Command::new("kubectl")
                .args(&args)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .map_err(|e| format!("Failed to spawn kubectl: {e}"))?;

            if let Some(mut stdin) = child.stdin.take() {
                stdin
                    .write_all(yaml.as_bytes())
                    .map_err(|e| format!("Failed to write YAML: {e}"))?;
            }

            let output = child
                .wait_with_output()
                .map_err(|e| format!("Failed to wait: {e}"))?;

            if output.status.success() {
                Ok(String::from_utf8_lossy(&output.stdout).to_string())
            } else {
                Err(String::from_utf8_lossy(&output.stderr).to_string())
            }
        })
        .await
    }
    .await
    .map_err(ColimaError::from)
}

#[tauri::command]
pub async fn k8s_yaml(
    resource_type: String,
    namespace: String,
    name: String,
) -> Result<String, ColimaError> {
    async move {
        ensure_name("resource type", &resource_type)?;
        ensure_name("resource name", &name)?;
        ensure_optional_namespace(&namespace)?;
        run_blocking(move || {
            if namespace.is_empty() || namespace == "all" {
                run_cmd("kubectl", &["get", &resource_type, &name, "-o", "yaml"])
            } else {
                run_cmd(
                    "kubectl",
                    &["get", &resource_type, &name, "-n", &namespace, "-o", "yaml"],
                )
            }
        })
        .await
    }
    .await
    .map_err(ColimaError::from)
}

/// Delete is restricted to an allow-list of resource types.
///
/// Without it a typo — or a caller passing something like `nodes` — turns a
/// routine cleanup into cluster damage that no confirmation dialog can undo.
#[tauri::command]
pub async fn k8s_delete_resource(
    resource_type: String,
    namespace: String,
    name: String,
) -> Result<String, ColimaError> {
    async move {
        if !K8S_DELETABLE_RESOURCES.contains(&resource_type.as_str()) {
            return Err(format!(
                "Resource type '{resource_type}' is not allowed for deletion"
            ));
        }
        ensure_name("resource name", &name)?;
        ensure_name("namespace", &namespace)?;
        run_blocking(move || {
            run_cmd(
                "kubectl",
                &["delete", &resource_type, &name, "-n", &namespace],
            )
        })
        .await
    }
    .await
    .map_err(ColimaError::from)
}

#[tauri::command]
pub async fn k8s_restart(
    resource_type: String,
    namespace: String,
    name: String,
) -> Result<String, ColimaError> {
    async move {
        ensure_name("resource type", &resource_type)?;
        ensure_name("resource name", &name)?;
        ensure_name("namespace", &namespace)?;
        run_blocking(move || {
            let target = format!("{resource_type}/{name}");
            run_cmd("kubectl", &["rollout", "restart", &target, "-n", &namespace])
        })
        .await
    }
    .await
    .map_err(ColimaError::from)
}

#[tauri::command]
pub async fn k8s_generic_scale(
    resource_type: String,
    namespace: String,
    name: String,
    replicas: u32,
) -> Result<String, ColimaError> {
    async move {
        ensure_name("resource type", &resource_type)?;
        ensure_name("resource name", &name)?;
        ensure_name("namespace", &namespace)?;
        run_blocking(move || {
            let replicas_arg = format!("--replicas={replicas}");
            run_cmd(
                "kubectl",
                &[
                    "scale",
                    &resource_type,
                    &name,
                    "-n",
                    &namespace,
                    &replicas_arg,
                ],
            )
        })
        .await
    }
    .await
    .map_err(ColimaError::from)
}

#[tauri::command]
pub async fn k8s_crds() -> Result<String, ColimaError> {
    run_blocking(|| run_cmd("kubectl", &["get", "crd", "-o", "json"]))
        .await
        .map_err(ColimaError::from)
}

#[tauri::command]
pub async fn k8s_crd_resources(
    resource: String,
    namespace: String,
) -> Result<String, ColimaError> {
    async move {
        // A CRD plural is `widgets.example.com` — dots are meaningful here, so
        // this uses the same shape check the HTTP route does rather than
        // `is_valid_k8s_name`, which would also accept underscores.
        if resource.is_empty()
            || !resource
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
        {
            return Err(format!("Invalid CRD resource: {resource}"));
        }
        ensure_optional_namespace(&namespace)?;
        run_blocking(move || {
            if namespace.is_empty() || namespace == "all" {
                run_cmd(
                    "kubectl",
                    &["get", &resource, "-o", "json", "--all-namespaces"],
                )
            } else {
                run_cmd("kubectl", &["get", &resource, "-o", "json", "-n", &namespace])
            }
        })
        .await
    }
    .await
    .map_err(ColimaError::from)
}

#[tauri::command]
pub async fn k8s_pod_containers(namespace: String, pod: String) -> Result<String, ColimaError> {
    async move {
        ensure_name("namespace", &namespace)?;
        ensure_name("pod name", &pod)?;
        run_blocking(move || {
            run_cmd(
                "kubectl",
                &[
                    "get",
                    "pod",
                    "-n",
                    &namespace,
                    &pod,
                    "-o",
                    "jsonpath={.spec.containers[*].name}",
                ],
            )
        })
        .await
    }
    .await
    .map_err(ColimaError::from)
}

#[tauri::command]
pub async fn k8s_container_logs(
    namespace: String,
    pod: String,
    container: String,
    lines: u32,
    previous: bool,
) -> Result<String, ColimaError> {
    async move {
        ensure_name("namespace", &namespace)?;
        ensure_name("pod name", &pod)?;
        if !container.is_empty() {
            ensure_name("container name", &container)?;
        }
        run_blocking(move || {
            let tail = lines.to_string();
            let mut args = vec!["logs", "-n", &namespace, &pod, "--tail", &tail, "--timestamps"];
            if !container.is_empty() {
                args.push("-c");
                args.push(&container);
            }
            if previous {
                args.push("--previous");
            }
            run_cmd("kubectl", &args)
        })
        .await
    }
    .await
    .map_err(ColimaError::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delete_rejects_types_outside_the_allow_list() {
        assert!(!K8S_DELETABLE_RESOURCES.contains(&"nodes"));
        assert!(K8S_DELETABLE_RESOURCES.contains(&"pods"));
    }

    #[test]
    fn hostile_names_are_rejected() {
        for bad in ["../../etc/passwd", "a b", "-oProxyCommand=x", ""] {
            assert!(ensure_name("pod name", bad).is_err(), "accepted {bad:?}");
        }
    }

    #[test]
    fn namespace_sentinels_bypass_validation() {
        assert!(ensure_optional_namespace("").is_ok());
        assert!(ensure_optional_namespace("all").is_ok());
        assert!(ensure_optional_namespace("kube-system").is_ok());
        assert!(ensure_optional_namespace("bad ns").is_err());
    }
}
