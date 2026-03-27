use serde::{Deserialize, Serialize};
use std::process::Command;

fn kubectl_cmd() -> Command {
    Command::new("kubectl")
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
pub async fn k8s_check() -> Result<String, String> {
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
}

/// List namespaces
#[tauri::command]
pub async fn k8s_namespaces() -> Result<Vec<K8sNamespace>, String> {
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
}

/// List pods in a namespace (empty = all namespaces)
#[tauri::command]
pub async fn k8s_pods(namespace: String) -> Result<Vec<K8sPod>, String> {
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
                .map(|s| {
                    s.iter()
                        .map(|c| c["restartCount"].as_i64().unwrap_or(0))
                        .sum()
                })
                .unwrap_or(0);

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
}

/// List services in a namespace
#[tauri::command]
pub async fn k8s_services(namespace: String) -> Result<Vec<K8sService>, String> {
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
}

/// List deployments in a namespace
#[tauri::command]
pub async fn k8s_deployments(namespace: String) -> Result<Vec<K8sDeployment>, String> {
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
}

/// Get pod logs
#[tauri::command]
pub async fn k8s_pod_logs(namespace: String, pod: String, lines: u32) -> Result<String, String> {
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
}

/// Delete a pod
#[tauri::command]
pub async fn k8s_delete_pod(namespace: String, pod: String) -> Result<String, String> {
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
}

/// Describe a resource (pod, service, deployment, etc.)
#[tauri::command]
pub async fn k8s_describe(
    namespace: String,
    resource_type: String,
    name: String,
) -> Result<String, String> {
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
}

/// Scale a deployment
#[tauri::command]
pub async fn k8s_scale(
    namespace: String,
    deployment: String,
    replicas: u32,
) -> Result<String, String> {
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
}

/// Get cluster nodes
#[tauri::command]
pub async fn k8s_nodes() -> Result<String, String> {
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
}

/// Get events in a namespace
#[tauri::command]
pub async fn k8s_events(namespace: String) -> Result<String, String> {
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
}
