use serde::{Deserialize, Serialize};
use std::process::Command;

fn kubectl_cmd() -> Command {
    let mut cmd = Command::new(crate::path_util::resolve_binary("kubectl"));
    crate::path_util::apply_path_to_cmd(&mut cmd);
    cmd
}

fn ensure_valid_optional_namespace(namespace: &str) -> Result<(), crate::error::ColimaError> {
    if !namespace.is_empty() && namespace != "all" && !crate::validation::is_valid_k8s_name(namespace) {
        return Err(crate::error::ColimaError::validation(format!(
            "Invalid namespace: {namespace}"
        )));
    }
    Ok(())
}

fn ensure_valid_required_k8s_param(label: &str, val: &str) -> Result<(), crate::error::ColimaError> {
    if !crate::validation::is_valid_k8s_name(val) {
        return Err(crate::error::ColimaError::validation(format!(
            "Invalid {label}: {val}"
        )));
    }
    Ok(())
}

/// Normalize singular Kubernetes resource names to the canonical plural forms
/// expected by [`K8S_LISTABLE_RESOURCES`].
pub fn normalize_listable_resource(resource: &str) -> String {
    let trimmed = resource.trim().to_ascii_lowercase();
    match trimmed.as_str() {
        "pod" => "pods".to_string(),
        "deployment" => "deployments".to_string(),
        "service" | "svc" => "services".to_string(),
        "namespace" | "ns" => "namespaces".to_string(),
        "configmap" | "cm" => "configmaps".to_string(),
        "secret" => "secrets".to_string(),
        "statefulset" | "sts" => "statefulsets".to_string(),
        "daemonset" | "ds" => "daemonsets".to_string(),
        "replicaset" | "rs" => "replicasets".to_string(),
        "job" => "jobs".to_string(),
        "cronjob" | "cj" => "cronjobs".to_string(),
        "ingress" | "ing" => "ingresses".to_string(),
        "persistentvolume" => "persistentvolumes".to_string(),
        "persistentvolumeclaim" => "persistentvolumeclaims".to_string(),
        "endpoint" | "ep" => "endpoints".to_string(),
        "serviceaccount" | "sa" => "serviceaccounts".to_string(),
        "role" => "roles".to_string(),
        "rolebinding" => "rolebindings".to_string(),
        "clusterrole" => "clusterroles".to_string(),
        "clusterrolebinding" => "clusterrolebindings".to_string(),
        "storageclass" | "sc" => "storageclasses".to_string(),
        "networkpolicy" | "netpol" => "networkpolicies".to_string(),
        "horizontalpodautoscaler" => "horizontalpodautoscalers".to_string(),
        "limitrange" => "limitranges".to_string(),
        "resourcequota" => "resourcequotas".to_string(),
        "poddisruptionbudget" => "poddisruptionbudgets".to_string(),
        _ => trimmed,
    }
}

/// Kubernetes Namespace
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct K8sNamespace {
    pub name: String,
    pub status: String,
    pub age: String,
}

/// Kubernetes Pod
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct K8sPod {
    pub name: String,
    pub namespace: String,
    pub status: String,
    pub ready: String,
    pub restarts: String,
    pub age: String,
    pub node: String,
}

/// Kubernetes Service
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct K8sService {
    pub name: String,
    pub namespace: String,
    #[serde(rename = "type")]
    pub svc_type: String,
    pub cluster_ip: String,
    pub ports: String,
    pub age: String,
}

/// Kubernetes Deployment
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct K8sDeployment {
    pub name: String,
    pub namespace: String,
    pub ready: String,
    pub available: String,
    pub age: String,
}

/// Check if kubectl is available and connected
#[tauri::command]
pub async fn k8s_check() -> Result<String, crate::error::ColimaError> {
    crate::helpers::run_blocking(|| {
        let output = kubectl_cmd()
            .args(["cluster-info", "--request-timeout=3s"])
            .output()
            .map_err(|e| format!("kubectl not available: {}", e))?;

        if !output.status.success() {
            return Err(format!(
                "Cluster not reachable: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    })
    .await
    .map_err(crate::error::ColimaError::from)
}

/// List namespaces
#[tauri::command]
pub async fn k8s_namespaces() -> Result<Vec<K8sNamespace>, crate::error::ColimaError> {
    crate::helpers::run_blocking(|| {
        let output = kubectl_cmd()
            .args(["get", "namespaces", "-o", "json"])
            .output()
            .map_err(|e| format!("Failed to list namespaces: {}", e))?;

        if !output.status.success() {
            return Err(format!(
                "kubectl get namespaces failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let parsed: serde_json::Value =
            serde_json::from_str(&stdout).map_err(|e| format!("Failed to parse namespaces: {}", e))?;

        let empty = vec![];
        let items = parsed["items"].as_array().unwrap_or(&empty);
        let namespaces: Vec<K8sNamespace> = items
            .iter()
            .map(|item| {
                let name = item["metadata"]["name"].as_str().unwrap_or("").to_string();
                let status = item["status"]["phase"]
                    .as_str()
                    .unwrap_or("Unknown")
                    .to_string();
                let creation = item["metadata"]["creationTimestamp"].as_str().unwrap_or("");
                K8sNamespace {
                    name,
                    status,
                    age: creation.to_string(),
                }
            })
            .collect();

        Ok(namespaces)
    })
    .await
    .map_err(crate::error::ColimaError::from)
}

/// List pods in a namespace (empty = all namespaces)
#[tauri::command]
pub async fn k8s_pods(namespace: String) -> Result<Vec<K8sPod>, crate::error::ColimaError> {
    ensure_valid_optional_namespace(&namespace)?;
    crate::helpers::run_blocking(move || {
        let mut args = vec!["get", "pods", "-o", "json"];
        if namespace.is_empty() || namespace == "all" {
            args.push("--all-namespaces");
        } else {
            args.push("-n");
            args.push(&namespace);
        }

        let output = kubectl_cmd()
            .args(&args)
            .output()
            .map_err(|e| format!("Failed to list pods: {}", e))?;

        if !output.status.success() {
            return Err(format!(
                "kubectl get pods failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let parsed: serde_json::Value =
            serde_json::from_str(&stdout).map_err(|e| format!("Failed to parse pods: {}", e))?;

        let empty = vec![];
        let items = parsed["items"].as_array().unwrap_or(&empty);
        let pods: Vec<K8sPod> = items
            .iter()
            .map(|item| {
                let name = item["metadata"]["name"].as_str().unwrap_or("").to_string();
                let ns = item["metadata"]["namespace"]
                    .as_str()
                    .unwrap_or("")
                    .to_string();
                let phase = item["status"]["phase"]
                    .as_str()
                    .unwrap_or("Unknown")
                    .to_string();
                let node = item["spec"]["nodeName"].as_str().unwrap_or("").to_string();

                // Calculate ready containers
                let container_statuses = item["status"]["containerStatuses"].as_array();
                let (ready_count, total_count) = if let Some(statuses) = container_statuses {
                    let ready = statuses
                        .iter()
                        .filter(|s| s["ready"].as_bool().unwrap_or(false))
                        .count();
                    (ready, statuses.len())
                } else {
                    (0, 0)
                };

                // Calculate restarts
                let restarts: i64 = container_statuses
                    .map_or(0, |s| {
                        s.iter()
                            .map(|c| c["restartCount"].as_i64().unwrap_or(0))
                            .sum()
                    });

                let creation = item["metadata"]["creationTimestamp"].as_str().unwrap_or("");

                K8sPod {
                    name,
                    namespace: ns,
                    status: phase,
                    ready: format!("{}/{}", ready_count, total_count),
                    restarts: restarts.to_string(),
                    age: creation.to_string(),
                    node,
                }
            })
            .collect();

        Ok(pods)
    })
    .await
    .map_err(crate::error::ColimaError::from)
}

/// List services in a namespace
#[tauri::command]
pub async fn k8s_services(namespace: String) -> Result<Vec<K8sService>, crate::error::ColimaError> {
    ensure_valid_optional_namespace(&namespace)?;
    crate::helpers::run_blocking(move || {
        let mut args = vec!["get", "services", "-o", "json"];
        if namespace.is_empty() || namespace == "all" {
            args.push("--all-namespaces");
        } else {
            args.push("-n");
            args.push(&namespace);
        }

        let output = kubectl_cmd()
            .args(&args)
            .output()
            .map_err(|e| format!("Failed to list services: {}", e))?;

        if !output.status.success() {
            return Err(format!(
                "kubectl get services failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let parsed: serde_json::Value =
            serde_json::from_str(&stdout).map_err(|e| format!("Failed to parse services: {}", e))?;

        let empty = vec![];
        let items = parsed["items"].as_array().unwrap_or(&empty);
        let services: Vec<K8sService> = items
            .iter()
            .map(|item| {
                let name = item["metadata"]["name"].as_str().unwrap_or("").to_string();
                let ns = item["metadata"]["namespace"]
                    .as_str()
                    .unwrap_or("")
                    .to_string();
                let svc_type = item["spec"]["type"]
                    .as_str()
                    .unwrap_or("ClusterIP")
                    .to_string();
                let cluster_ip = item["spec"]["clusterIP"]
                    .as_str()
                    .unwrap_or("None")
                    .to_string();
                let creation = item["metadata"]["creationTimestamp"].as_str().unwrap_or("");

                // Parse ports
                let ports_arr = item["spec"]["ports"].as_array();
                let ports = ports_arr
                    .map(|ps| {
                        ps.iter()
                            .map(|p| {
                                let port = p["port"].as_i64().unwrap_or(0);
                                let target = p["targetPort"]
                                    .as_i64()
                                    .or_else(|| p["targetPort"].as_str().and_then(|s| s.parse().ok()))
                                    .unwrap_or(port);
                                let protocol = p["protocol"].as_str().unwrap_or("TCP");
                                format!("{}/{} → {}", port, protocol, target)
                            })
                            .collect::<Vec<_>>()
                            .join(", ")
                    })
                    .unwrap_or_default();

                K8sService {
                    name,
                    namespace: ns,
                    svc_type,
                    cluster_ip,
                    ports,
                    age: creation.to_string(),
                }
            })
            .collect();

        Ok(services)
    })
    .await
    .map_err(crate::error::ColimaError::from)
}

/// List deployments in a namespace
#[tauri::command]
pub async fn k8s_deployments(namespace: String) -> Result<Vec<K8sDeployment>, crate::error::ColimaError> {
    ensure_valid_optional_namespace(&namespace)?;
    crate::helpers::run_blocking(move || {
        let mut args = vec!["get", "deployments", "-o", "json"];
        if namespace.is_empty() || namespace == "all" {
            args.push("--all-namespaces");
        } else {
            args.push("-n");
            args.push(&namespace);
        }

        let output = kubectl_cmd()
            .args(&args)
            .output()
            .map_err(|e| format!("Failed to list deployments: {}", e))?;

        if !output.status.success() {
            return Err(format!(
                "kubectl get deployments failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let parsed: serde_json::Value =
            serde_json::from_str(&stdout).map_err(|e| format!("Failed to parse deployments: {}", e))?;

        let empty = vec![];
        let items = parsed["items"].as_array().unwrap_or(&empty);
        let deployments: Vec<K8sDeployment> = items
            .iter()
            .map(|item| {
                let name = item["metadata"]["name"].as_str().unwrap_or("").to_string();
                let ns = item["metadata"]["namespace"]
                    .as_str()
                    .unwrap_or("")
                    .to_string();
                let creation = item["metadata"]["creationTimestamp"].as_str().unwrap_or("");

                let replicas = item["spec"]["replicas"].as_i64().unwrap_or(0);
                let ready = item["status"]["readyReplicas"].as_i64().unwrap_or(0);
                let available = item["status"]["availableReplicas"].as_i64().unwrap_or(0);

                K8sDeployment {
                    name,
                    namespace: ns,
                    ready: format!("{}/{}", ready, replicas),
                    available: available.to_string(),
                    age: creation.to_string(),
                }
            })
            .collect();

        Ok(deployments)
    })
    .await
    .map_err(crate::error::ColimaError::from)
}

/// Get pod logs
#[tauri::command]
pub async fn k8s_pod_logs(namespace: String, pod: String, lines: u32) -> Result<String, crate::error::ColimaError> {
    ensure_valid_required_k8s_param("namespace", &namespace)?;
    ensure_valid_required_k8s_param("pod", &pod)?;
    crate::helpers::run_blocking(move || {
        let tail = lines.to_string();
        let output = kubectl_cmd()
            .args([
                "logs",
                "-n",
                &namespace,
                &pod,
                "--tail",
                &tail,
                "--timestamps",
            ])
            .output()
            .map_err(|e| format!("Failed to get pod logs: {}", e))?;

        if !output.status.success() {
            return Err(format!(
                "kubectl logs failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    })
    .await
    .map_err(crate::error::ColimaError::from)
}

/// Delete a pod
#[tauri::command]
pub async fn k8s_delete_pod(namespace: String, pod: String) -> Result<String, crate::error::ColimaError> {
    ensure_valid_required_k8s_param("namespace", &namespace)?;
    ensure_valid_required_k8s_param("pod", &pod)?;
    crate::helpers::run_blocking(move || {
        let output = kubectl_cmd()
            .args(["delete", "pod", "-n", &namespace, &pod])
            .output()
            .map_err(|e| format!("Failed to delete pod: {}", e))?;

        if !output.status.success() {
            return Err(format!(
                "kubectl delete pod failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        Ok(format!("Pod {} deleted", pod))
    })
    .await
    .map_err(crate::error::ColimaError::from)
}

/// Describe a resource (pod, service, deployment, etc.)
#[tauri::command]
pub async fn k8s_describe(     namespace: String,     resource_type: String,     name: String, ) -> Result<String, crate::error::ColimaError> {
    ensure_valid_required_k8s_param("namespace", &namespace)?;
    ensure_valid_required_k8s_param("resource type", &resource_type)?;
    ensure_valid_required_k8s_param("resource name", &name)?;
    crate::helpers::run_blocking(move || {
        let output = kubectl_cmd()
            .args(["describe", &resource_type, "-n", &namespace, &name])
            .output()
            .map_err(|e| format!("Failed to describe: {}", e))?;

        if !output.status.success() {
            return Err(format!(
                "kubectl describe failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    })
    .await
    .map_err(crate::error::ColimaError::from)
}

/// Open a pod shell in the OS terminal.
///
/// This is the *secondary* path. The primary one is an in-app Terminal tab
/// backed by a real pty, which is why this command briefly disappeared — the
/// frontend call site in `k8s.ts` outlived its registration and every click
/// returned "Command k8s_exec not found". It is registered again because
/// handing the session to Terminal.app is still the escape hatch when the
/// embedded terminal gets something wrong.
///
/// Names are validated before they reach a command line: they are interpolated
/// into an AppleScript string on macOS, so an unchecked name would be an
/// injection.
#[tauri::command]
pub async fn k8s_exec(
    namespace: String,
    pod: String,
    container: String,
) -> Result<String, crate::error::ColimaError> {
    async move {
        if !crate::validation::is_valid_k8s_name(&namespace) {
            return Err(format!("Invalid namespace: {namespace}"));
        }
        if !crate::validation::is_valid_k8s_name(&pod) {
            return Err(format!("Invalid pod name: {pod}"));
        }
        if !container.is_empty() && !crate::validation::is_valid_k8s_name(&container) {
            return Err(format!("Invalid container name: {container}"));
        }

        let mut cmd_str = format!("kubectl exec -it -n {namespace} {pod}");
        if !container.is_empty() {
            cmd_str.push_str(&format!(" -c {container}"));
        }
        cmd_str.push_str(" -- /bin/sh");

        #[cfg(target_os = "macos")]
        {
            let escaped = crate::validation::escape_applescript(&cmd_str);
            std::process::Command::new("osascript")
                .args([
                    "-e",
                    &format!("tell application \"Terminal\" to do script \"{escaped}\""),
                ])
                .spawn()
                .map_err(|e| format!("Failed to open terminal: {e}"))?;
        }

        #[cfg(target_os = "linux")]
        {
            let terminals = ["gnome-terminal", "xterm", "konsole"];
            let launched = terminals.iter().any(|term| {
                std::process::Command::new(term)
                    .args(["--", "sh", "-c", &cmd_str])
                    .spawn()
                    .is_ok()
            });
            if !launched {
                return Err("No terminal emulator found".to_string());
            }
        }

        Ok(format!("Shell opened for pod {pod}"))
    }
    .await
    .map_err(|e: String| crate::error::ColimaError::from(e))
}

/// Scale a deployment
#[tauri::command]
pub async fn k8s_scale(     namespace: String,     deployment: String,     replicas: u32, ) -> Result<String, crate::error::ColimaError> {
    ensure_valid_required_k8s_param("namespace", &namespace)?;
    ensure_valid_required_k8s_param("deployment", &deployment)?;
    crate::helpers::run_blocking(move || {
        let replicas_str = format!("--replicas={}", replicas);
        let output = kubectl_cmd()
            .args([
                "scale",
                "deployment",
                &deployment,
                "-n",
                &namespace,
                &replicas_str,
            ])
            .output()
            .map_err(|e| format!("Failed to scale: {}", e))?;

        if !output.status.success() {
            return Err(format!(
                "kubectl scale failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        Ok(format!(
            "Deployment {} scaled to {} replicas",
            deployment, replicas
        ))
    })
    .await
    .map_err(crate::error::ColimaError::from)
}

/// Get cluster nodes
#[tauri::command]
pub async fn k8s_nodes() -> Result<String, crate::error::ColimaError> {
    crate::helpers::run_blocking(|| {
        let output = kubectl_cmd()
            .args(["get", "nodes", "-o", "wide"])
            .output()
            .map_err(|e| format!("Failed to get nodes: {}", e))?;

        if !output.status.success() {
            return Err(format!(
                "kubectl get nodes failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    })
    .await
    .map_err(crate::error::ColimaError::from)
}

/// Resource types the generic browser is allowed to list.
///
/// An allowlist rather than validation, because the value goes straight into
/// `kubectl get <type>` and there is no reason to support arbitrary strings.
pub const K8S_LISTABLE_RESOURCES: &[&str] = &[
    "pods",
    "deployments",
    "services",
    "namespaces",
    "configmaps",
    "secrets",
    "statefulsets",
    "daemonsets",
    "replicasets",
    "jobs",
    "cronjobs",
    "ingresses",
    "persistentvolumes",
    "persistentvolumeclaims",
    "pv",
    "pvc",
    "endpoints",
    "serviceaccounts",
    "roles",
    "rolebindings",
    "clusterroles",
    "clusterrolebindings",
    "storageclasses",
    "networkpolicies",
    "horizontalpodautoscalers",
    "hpa",
    "limitranges",
    "resourcequotas",
    "poddisruptionbudgets",
    "pdb",
];

/// List any allowlisted resource type as JSON.
///
/// This existed only as an HTTP route, so the Kubernetes browser worked in
/// browser mode and failed in the desktop app with "Command k8s_resources not
/// found". The implementation lives here and `routes/k8s.rs` delegates to it,
/// matching how the rest of the backend is layered.
#[tauri::command]
pub async fn k8s_resources(
    resource: String,
    namespace: String,
) -> Result<String, crate::error::ColimaError> {
    ensure_valid_optional_namespace(&namespace)?;
    let normalized = normalize_listable_resource(&resource);
    if !K8S_LISTABLE_RESOURCES.contains(&normalized.as_str()) {
        return Err(crate::error::ColimaError::validation(format!(
            "Resource type '{}' not allowed",
            resource
        )));
    }

    let all_namespaces = namespace.is_empty() || namespace == "all";
    crate::helpers::run_blocking(move || {
        let mut args = vec!["get", normalized.as_str(), "-o", "json"];
        if all_namespaces {
            args.push("--all-namespaces");
        } else {
            args.push("-n");
            args.push(namespace.as_str());
        }
        let output = kubectl_cmd()
            .args(&args)
            .output()
            .map_err(|e| format!("Failed to run kubectl: {}", e))?;

        if !output.status.success() {
            return Err(format!(
                "kubectl get failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    })
    .await
    .map_err(crate::error::ColimaError::from)
}

/// Get events in a namespace
#[tauri::command]
pub async fn k8s_events(namespace: String) -> Result<String, crate::error::ColimaError> {
    ensure_valid_optional_namespace(&namespace)?;
    crate::helpers::run_blocking(move || {
        let mut args = vec!["get", "events", "--sort-by=.metadata.creationTimestamp"];
        if namespace.is_empty() || namespace == "all" {
            args.push("--all-namespaces");
        } else {
            args.push("-n");
            args.push(&namespace);
        }

        let output = kubectl_cmd()
            .args(&args)
            .output()
            .map_err(|e| format!("Failed to get events: {}", e))?;

        if !output.status.success() {
            return Err(format!(
                "kubectl get events failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    })
    .await
    .map_err(crate::error::ColimaError::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_listable_resource_singulars() {
        assert_eq!(normalize_listable_resource("ingress"), "ingresses");
        assert_eq!(normalize_listable_resource("pod"), "pods");
        assert_eq!(normalize_listable_resource("service"), "services");
        assert_eq!(normalize_listable_resource("deployment"), "deployments");
        assert_eq!(normalize_listable_resource("ingresses"), "ingresses");
        assert_eq!(normalize_listable_resource("pods"), "pods");
        for r in ["ingress", "pod", "service", "deployment"] {
            let norm = normalize_listable_resource(r);
            assert!(
                K8S_LISTABLE_RESOURCES.contains(&norm.as_str()),
                "normalized '{}' -> '{}' must be in K8S_LISTABLE_RESOURCES",
                r,
                norm
            );
        }
    }
}
