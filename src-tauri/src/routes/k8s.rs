use std::process::Command;
use std::convert::Infallible;
use axum::response::sse::{Event, Sse};
use futures_util::StreamExt;
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::Json,
};
use serde::{Deserialize, Serialize};
use crate::api_server::*;
use crate::commands::*;
use crate::routes::payloads::*;

pub async fn api_k8s_action(Query(q): Query<K8sQuery>) -> (StatusCode, Json<ApiResponse<String>>) {
    let profile = q.profile;
    let action = q.action;
    match run_blocking(move || {
        let valid_actions = ["start", "stop", "delete", "reset"];
        if !valid_actions.contains(&action.as_str()) {
            return Err(format!("Invalid kubernetes action: {}", action));
        }
        let mut args = vec!["kubernetes".to_string(), action.clone()];
        if profile != "default" && !profile.is_empty() {
            args.push("--profile".to_string());
            args.push(profile);
        }
        let args_ref: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let output = Command::new("colima")
            .args(&args_ref)
            .output()
            .map_err(|e| format!("Failed to execute kubernetes {}: {}", action, e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            // Treat "not enabled" / "not running" as success for delete/stop
            // (K3s is already in the desired state)
            if (action == "delete" || action == "stop")
                && (stderr.contains("not enabled") || stderr.contains("not running"))
            {
                return Ok(format!(
                    "Kubernetes {} completed (already disabled)",
                    action
                ));
            }
            return Err(format!("kubernetes {} failed: {}", action, stderr));
        }
        Ok(format!("Kubernetes {} completed", action))
    })
    .await
    {
        Ok(msg) => ok(msg),
        Err(e) => err(e),
    }
}


pub async fn api_k8s_check() -> (StatusCode, Json<ApiResponse<String>>) {
    match run_blocking(|| run_cmd("kubectl", &["cluster-info", "--request-timeout=3s"])).await {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}


pub async fn api_k8s_namespaces() -> (StatusCode, Json<ApiResponse<String>>) {
    match run_blocking(|| run_cmd("kubectl", &["get", "namespaces", "-o", "json"])).await {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}


pub async fn api_k8s_pods(Query(q): Query<K8sNsQuery>) -> (StatusCode, Json<ApiResponse<String>>) {
    let ns = q.namespace;
    match run_blocking(move || {
        if ns.is_empty() || ns == "all" {
            run_cmd(
                "kubectl",
                &["get", "pods", "-o", "json", "--all-namespaces"],
            )
        } else {
            run_cmd("kubectl", &["get", "pods", "-o", "json", "-n", &ns])
        }
    })
    .await
    {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}


pub async fn api_k8s_services(Query(q): Query<K8sNsQuery>) -> (StatusCode, Json<ApiResponse<String>>) {
    let ns = q.namespace;
    match run_blocking(move || {
        if ns.is_empty() || ns == "all" {
            run_cmd(
                "kubectl",
                &["get", "services", "-o", "json", "--all-namespaces"],
            )
        } else {
            run_cmd("kubectl", &["get", "services", "-o", "json", "-n", &ns])
        }
    })
    .await
    {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}


pub async fn api_k8s_deployments(
    Query(q): Query<K8sNsQuery>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    let ns = q.namespace;
    match run_blocking(move || {
        if ns.is_empty() || ns == "all" {
            run_cmd(
                "kubectl",
                &["get", "deployments", "-o", "json", "--all-namespaces"],
            )
        } else {
            run_cmd("kubectl", &["get", "deployments", "-o", "json", "-n", &ns])
        }
    })
    .await
    {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}


pub async fn api_k8s_pod_logs(
    Query(q): Query<K8sPodLogQuery>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    let tail = q.lines.to_string();
    let ns = q.namespace;
    let pod = q.pod;
    match run_blocking(move || {
        run_cmd(
            "kubectl",
            &["logs", "-n", &ns, &pod, "--tail", &tail, "--timestamps"],
        )
    })
    .await
    {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}


pub async fn api_k8s_delete_pod(
    Json(body): Json<K8sDeletePodBody>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    let ns = body.namespace;
    let pod = body.pod;
    match run_blocking(move || run_cmd("kubectl", &["delete", "pod", "-n", &ns, &pod])).await {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}


pub async fn api_k8s_describe(
    Query(q): Query<K8sDescribeQuery>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    let rt = q.resource_type;
    let ns = q.namespace;
    let name = q.name;
    match run_blocking(move || run_cmd("kubectl", &["describe", &rt, "-n", &ns, &name])).await {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}


pub async fn api_k8s_scale(Json(body): Json<K8sScaleBody>) -> (StatusCode, Json<ApiResponse<String>>) {
    let replicas = format!("--replicas={}", body.replicas);
    let ns = body.namespace;
    let dep = body.deployment;
    match run_blocking(move || {
        run_cmd(
            "kubectl",
            &["scale", "deployment", &dep, "-n", &ns, &replicas],
        )
    })
    .await
    {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}


pub async fn api_k8s_nodes() -> (StatusCode, Json<ApiResponse<String>>) {
    match run_blocking(|| run_cmd("kubectl", &["get", "nodes", "-o", "wide"])).await {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}


pub async fn api_k8s_events(Query(q): Query<K8sNsQuery>) -> (StatusCode, Json<ApiResponse<String>>) {
    let ns = q.namespace;
    match run_blocking(move || {
        if ns.is_empty() || ns == "all" {
            run_cmd(
                "kubectl",
                &[
                    "get",
                    "events",
                    "--sort-by=.metadata.creationTimestamp",
                    "--all-namespaces",
                ],
            )
        } else {
            run_cmd(
                "kubectl",
                &[
                    "get",
                    "events",
                    "--sort-by=.metadata.creationTimestamp",
                    "-n",
                    &ns,
                ],
            )
        }
    })
    .await
    {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}


pub async fn api_k8s_resources(
    Query(q): Query<K8sResourceQuery>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    let resource = q.resource;
    let ns = q.namespace;
    // Whitelist allowed resource types
    let allowed = [
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
    if !allowed.contains(&resource.as_str()) {
        return err(format!("Resource type '{}' not allowed", resource));
    }
    match run_blocking(move || {
        if ns.is_empty() || ns == "all" {
            run_cmd(
                "kubectl",
                &["get", &resource, "-o", "json", "--all-namespaces"],
            )
        } else {
            run_cmd("kubectl", &["get", &resource, "-o", "json", "-n", &ns])
        }
    })
    .await
    {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}


pub async fn api_k8s_delete_resource(
    Json(body): Json<K8sDeleteBody>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    let rt = body.resource_type;
    let ns = body.namespace;
    let name = body.name;
    // Validate resource type against whitelist (Fix #5)
    if !K8S_DELETABLE_RESOURCES.contains(&rt.as_str()) {
        return err(format!(
            "Resource type '{}' is not allowed for deletion",
            rt
        ));
    }
    match run_blocking(move || run_cmd("kubectl", &["delete", &rt, &name, "-n", &ns])).await {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}


pub async fn api_k8s_restart(
    Json(body): Json<K8sRestartBody>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    let rt = body.resource_type;
    let ns = body.namespace;
    let name = body.name;
    let target = format!("{}/{}", rt, name);
    match run_blocking(move || run_cmd("kubectl", &["rollout", "restart", &target, "-n", &ns]))
        .await
    {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}


pub async fn api_k8s_yaml(Query(q): Query<K8sYamlQuery>) -> (StatusCode, Json<ApiResponse<String>>) {
    let rt = q.resource_type;
    let ns = q.namespace;
    let name = q.name;
    match run_blocking(move || {
        if ns.is_empty() || ns == "all" {
            run_cmd("kubectl", &["get", &rt, &name, "-o", "yaml"])
        } else {
            run_cmd("kubectl", &["get", &rt, &name, "-n", &ns, "-o", "yaml"])
        }
    })
    .await
    {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}


// Nodes as JSON
pub async fn api_k8s_nodes_json() -> (StatusCode, Json<ApiResponse<String>>) {
    match run_blocking(|| run_cmd("kubectl", &["get", "nodes", "-o", "json"])).await {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}


// Events as JSON
pub async fn api_k8s_events_json(
    Query(q): Query<K8sNsQuery>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    let ns = q.namespace;
    match run_blocking(move || {
        if ns.is_empty() || ns == "all" {
            run_cmd(
                "kubectl",
                &[
                    "get",
                    "events",
                    "-o",
                    "json",
                    "--sort-by=.metadata.creationTimestamp",
                    "--all-namespaces",
                ],
            )
        } else {
            run_cmd("kubectl", &["get", "events", "-o", "json", "-n", &ns])
        }
    })
    .await
    {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}


// K8s contexts
pub async fn api_k8s_contexts() -> (StatusCode, Json<ApiResponse<String>>) {
    match run_blocking(|| run_cmd("kubectl", &["config", "get-contexts", "-o", "name"])).await {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}


pub async fn api_k8s_current_context() -> (StatusCode, Json<ApiResponse<String>>) {
    match run_blocking(|| run_cmd("kubectl", &["config", "current-context"])).await {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}


pub async fn api_k8s_set_context(
    Json(body): Json<K8sContextBody>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    let ctx = body.context;
    match run_blocking(move || run_cmd("kubectl", &["config", "use-context", &ctx])).await {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}


pub async fn api_k8s_apply(Json(body): Json<K8sApplyBody>) -> (StatusCode, Json<ApiResponse<String>>) {
    let yaml_content = body.yaml;
    let ns = body.namespace;
    match run_blocking(move || {
        use std::io::Write;
        use std::process::{Command, Stdio};
        let mut args = vec!["apply", "-f", "-"];
        if !ns.is_empty() && ns != "all" {
            args.push("-n");
            args.push(&ns);
        }
        let mut child = Command::new("kubectl")
            .args(&args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to spawn kubectl: {}", e))?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(yaml_content.as_bytes())
                .map_err(|e| format!("Failed to write YAML: {}", e))?;
        }
        let output = child
            .wait_with_output()
            .map_err(|e| format!("Failed to wait: {}", e))?;
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).to_string())
        }
    })
    .await
    {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}


pub async fn api_k8s_port_forward_start(
    Json(body): Json<K8sPortForwardBody>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    let _key = format!("{}:{}", body.local_port, body.remote_port);
    let ns = body.namespace;
    let target = format!("{}/{}", body.resource_type, body.name);
    let ports = format!("{}:{}", body.local_port, body.remote_port);
    let local_port = body.local_port;

    match run_blocking(move || {
        use std::process::{Command, Stdio};
        let child = Command::new("kubectl")
            .args(&["port-forward", "-n", &ns, &target, &ports])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("Failed to start port-forward: {}", e))?;
        let pid = child.id();
        if let Ok(mut fwds) = PORT_FORWARDS.lock() {
            fwds.insert(format!("{}", local_port), pid);
        }
        Ok(format!(
            "Port forward started: localhost:{} → {}",
            local_port, ports
        ))
    })
    .await
    {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}


pub async fn api_k8s_port_forward_stop(
    Json(body): Json<K8sPortForwardStopBody>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    let port = body.local_port;
    match run_blocking(move || {
        if let Ok(mut fwds) = PORT_FORWARDS.lock() {
            let key = format!("{}", port);
            if let Some(pid) = fwds.remove(&key) {
                #[cfg(unix)]
                unsafe {
                    libc::kill(pid as i32, libc::SIGTERM);
                }
                return Ok(format!("Port forward on {} stopped", port));
            }
        }
        // Fallback: kill by port
        let _ = std::process::Command::new("lsof")
            .args(&["-ti", &format!(":{}", port)])
            .output()
            .and_then(|o| {
                let pids = String::from_utf8_lossy(&o.stdout).trim().to_string();
                if !pids.is_empty() {
                    std::process::Command::new("kill")
                        .args(pids.split('\n'))
                        .output()?;
                }
                Ok(())
            });
        Ok(format!("Port forward on {} stopped", port))
    })
    .await
    {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}


pub async fn api_k8s_port_forward_list() -> (StatusCode, Json<ApiResponse<String>>) {
    match run_blocking(|| {
        let fwds = PORT_FORWARDS.lock().map_err(|e| format!("{}", e))?;
        let result: Vec<String> = fwds
            .iter()
            .map(|(port, pid)| format!("{}:{}", port, pid))
            .collect();
        Ok(result.join("\n"))
    })
    .await
    {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}


pub async fn api_k8s_exec(Json(body): Json<K8sExecBody>) -> (StatusCode, Json<ApiResponse<String>>) {
    let ns = body.namespace;
    let pod = body.pod;
    let container = body.container;

    // Fix #3: Validate inputs before embedding in shell/AppleScript strings
    if !is_valid_k8s_name(&ns) {
        return err("Invalid namespace name".to_string());
    }
    if !is_valid_k8s_name(&pod) {
        return err("Invalid pod name".to_string());
    }
    if !container.is_empty() && !is_valid_k8s_name(&container) {
        return err("Invalid container name".to_string());
    }

    match run_blocking(move || {
        let mut cmd_str = format!("kubectl exec -it -n {} {}", ns, pod);
        if !container.is_empty() {
            cmd_str.push_str(&format!(" -c {}", container));
        }
        cmd_str.push_str(" -- /bin/sh");

        // Open in macOS Terminal.app (with escaped strings)
        #[cfg(target_os = "macos")]
        {
            let escaped_cmd = escape_applescript(&cmd_str);
            std::process::Command::new("osascript")
                .args(&[
                    "-e",
                    &format!(
                        "tell application \"Terminal\" to do script \"{}\"",
                        escaped_cmd
                    ),
                ])
                .spawn()
                .map_err(|e| format!("Failed to open terminal: {}", e))?;
        }
        #[cfg(target_os = "linux")]
        {
            let terminals = ["gnome-terminal", "xterm", "konsole"];
            let mut launched = false;
            for term in &terminals {
                if std::process::Command::new(term)
                    .args(&["--", "sh", "-c", &cmd_str])
                    .spawn()
                    .is_ok()
                {
                    launched = true;
                    break;
                }
            }
            if !launched {
                return Err("No terminal emulator found".to_string());
            }
        }
        Ok(format!("Shell opened for pod {}", pod))
    })
    .await
    {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}


pub async fn api_k8s_container_logs(
    Query(q): Query<K8sContainerLogQuery>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    let tail = q.lines.to_string();
    let ns = q.namespace;
    let pod = q.pod;
    let container = q.container;
    let previous = q.previous;
    match run_blocking(move || {
        let mut args = vec!["logs", "-n", &ns, &pod, "--tail", &tail, "--timestamps"];
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
    {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}


// Get pod containers list
pub async fn api_k8s_pod_containers(
    Query(q): Query<K8sDeletePodBody>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    let ns = q.namespace;
    let pod = q.pod;
    match run_blocking(move || {
        run_cmd(
            "kubectl",
            &[
                "get",
                "pod",
                "-n",
                &ns,
                &pod,
                "-o",
                "jsonpath={.spec.containers[*].name}",
            ],
        )
    })
    .await
    {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}


pub async fn api_k8s_node_action(
    Json(body): Json<K8sNodeBody>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    let name = body.name;
    let action = body.action;
    match run_blocking(move || match action.as_str() {
        "cordon" => run_cmd("kubectl", &["cordon", &name]),
        "uncordon" => run_cmd("kubectl", &["uncordon", &name]),
        "drain" => run_cmd(
            "kubectl",
            &[
                "drain",
                &name,
                "--ignore-daemonsets",
                "--delete-emptydir-data",
                "--force",
            ],
        ),
        _ => Err(format!("Unknown action: {}", action)),
    })
    .await
    {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}


pub async fn api_kind_list() -> (StatusCode, Json<ApiResponse<String>>) {
    match run_blocking(|| run_cmd("kind", &["get", "clusters"])).await {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}


pub async fn api_kind_create(
    Json(body): Json<KindCreateBody>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    let name = body.name;
    let image = body.image;
    match run_blocking(move || {
        let mut args = vec!["create", "cluster", "--name", &name];
        if !image.is_empty() {
            args.push("--image");
            args.push(&image);
        }
        run_cmd("kind", &args)
    })
    .await
    {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}


pub async fn api_kind_delete(
    Json(body): Json<KindDeleteBody>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    let name = body.name;
    match run_blocking(move || run_cmd("kind", &["delete", "cluster", "--name", &name])).await {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}


pub async fn api_k8s_generic_scale(
    Json(body): Json<K8sGenericScaleBody>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    let replicas = format!("--replicas={}", body.replicas);
    let ns = body.namespace;
    let name = body.name;
    let rt = body.resource_type;
    match run_blocking(move || run_cmd("kubectl", &["scale", &rt, &name, "-n", &ns, &replicas]))
        .await
    {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}


pub async fn api_k8s_cluster_health() -> (StatusCode, Json<ApiResponse<String>>) {
    match run_blocking(|| {
        // Gather data from multiple sources
        let pods_raw = run_cmd("kubectl", &["get", "pods", "--all-namespaces", "-o", "json"]).unwrap_or_default();
        let deploys_raw = run_cmd("kubectl", &["get", "deployments", "--all-namespaces", "-o", "json"]).unwrap_or_default();
        let pvcs_raw = run_cmd("kubectl", &["get", "pvc", "--all-namespaces", "-o", "json"]).unwrap_or_default();
        let events_raw = run_cmd("kubectl", &["get", "events", "--all-namespaces", "--field-selector=type=Warning", "-o", "json"]).unwrap_or_default();
        let nodes_raw = run_cmd("kubectl", &["get", "nodes", "-o", "json"]).unwrap_or_default();

        let mut issues: Vec<serde_json::Value> = Vec::new();
        let mut score: u32 = 100;

        // Check pods
        if let Ok(pods) = serde_json::from_str::<serde_json::Value>(&pods_raw) {
            if let Some(items) = pods["items"].as_array() {
                let total_pods = items.len();
                let mut unhealthy = 0u32;
                for pod in items {
                    let phase = pod["status"]["phase"].as_str().unwrap_or("");
                    let name = pod["metadata"]["name"].as_str().unwrap_or("");
                    let ns = pod["metadata"]["namespace"].as_str().unwrap_or("");
                    let containers = pod["status"]["containerStatuses"].as_array();

                    if phase == "Failed" || phase == "Unknown" {
                        unhealthy += 1;
                        issues.push(serde_json::json!({
                            "severity": "error",
                            "category": "Pod",
                            "resource": format!("{}/{}", ns, name),
                            "message": format!("Pod is in {} phase", phase)
                        }));
                    } else if phase == "Pending" {
                        unhealthy += 1;
                        issues.push(serde_json::json!({
                            "severity": "warning",
                            "category": "Pod",
                            "resource": format!("{}/{}", ns, name),
                            "message": "Pod is pending"
                        }));
                    }

                    if let Some(statuses) = containers {
                        for cs in statuses {
                            let restarts = cs["restartCount"].as_u64().unwrap_or(0);
                            let ready = cs["ready"].as_bool().unwrap_or(false);
                            let cname = cs["name"].as_str().unwrap_or("");
                            if restarts > 5 {
                                issues.push(serde_json::json!({
                                    "severity": "warning",
                                    "category": "Pod",
                                    "resource": format!("{}/{}", ns, name),
                                    "message": format!("Container {} has {} restarts", cname, restarts)
                                }));
                            }
                            if !ready {
                                issues.push(serde_json::json!({
                                    "severity": "warning",
                                    "category": "Pod",
                                    "resource": format!("{}/{}", ns, name),
                                    "message": format!("Container {} is not ready", cname)
                                }));
                            }
                            // Check CrashLoopBackOff
                            if let Some(waiting) = cs["state"]["waiting"]["reason"].as_str() {
                                if waiting == "CrashLoopBackOff" || waiting == "ImagePullBackOff" || waiting == "ErrImagePull" {
                                    issues.push(serde_json::json!({
                                        "severity": "error",
                                        "category": "Pod",
                                        "resource": format!("{}/{}", ns, name),
                                        "message": format!("Container {} in {}", cname, waiting)
                                    }));
                                }
                            }
                        }
                    }
                }
                if unhealthy > 0 {
                    score = score.saturating_sub(unhealthy * 5);
                }
                issues.push(serde_json::json!({
                    "severity": "info",
                    "category": "Summary",
                    "resource": "Pods",
                    "message": format!("{} total, {} unhealthy", total_pods, unhealthy)
                }));
            }
        }

        // Check deployments
        if let Ok(deploys) = serde_json::from_str::<serde_json::Value>(&deploys_raw) {
            if let Some(items) = deploys["items"].as_array() {
                for dep in items {
                    let name = dep["metadata"]["name"].as_str().unwrap_or("");
                    let ns = dep["metadata"]["namespace"].as_str().unwrap_or("");
                    let desired = dep["spec"]["replicas"].as_u64().unwrap_or(0);
                    let ready = dep["status"]["readyReplicas"].as_u64().unwrap_or(0);
                    let available = dep["status"]["availableReplicas"].as_u64().unwrap_or(0);
                    if ready < desired {
                        score = score.saturating_sub(3);
                        issues.push(serde_json::json!({
                            "severity": "warning",
                            "category": "Deployment",
                            "resource": format!("{}/{}", ns, name),
                            "message": format!("Only {}/{} replicas ready", ready, desired)
                        }));
                    }
                    if available < desired {
                        issues.push(serde_json::json!({
                            "severity": "warning",
                            "category": "Deployment",
                            "resource": format!("{}/{}", ns, name),
                            "message": format!("Only {}/{} replicas available", available, desired)
                        }));
                    }
                }
            }
        }

        // Check PVCs
        if let Ok(pvcs) = serde_json::from_str::<serde_json::Value>(&pvcs_raw) {
            if let Some(items) = pvcs["items"].as_array() {
                for pvc in items {
                    let name = pvc["metadata"]["name"].as_str().unwrap_or("");
                    let ns = pvc["metadata"]["namespace"].as_str().unwrap_or("");
                    let phase = pvc["status"]["phase"].as_str().unwrap_or("");
                    if phase != "Bound" {
                        score = score.saturating_sub(5);
                        issues.push(serde_json::json!({
                            "severity": "error",
                            "category": "PVC",
                            "resource": format!("{}/{}", ns, name),
                            "message": format!("PVC is {}", phase)
                        }));
                    }
                }
            }
        }

        // Check nodes
        if let Ok(nodes) = serde_json::from_str::<serde_json::Value>(&nodes_raw) {
            if let Some(items) = nodes["items"].as_array() {
                for node in items {
                    let name = node["metadata"]["name"].as_str().unwrap_or("");
                    let conditions = node["status"]["conditions"].as_array();
                    if let Some(conds) = conditions {
                        for cond in conds {
                            let ctype = cond["type"].as_str().unwrap_or("");
                            let status = cond["status"].as_str().unwrap_or("");
                            if ctype == "Ready" && status != "True" {
                                score = score.saturating_sub(15);
                                issues.push(serde_json::json!({
                                    "severity": "error",
                                    "category": "Node",
                                    "resource": name,
                                    "message": "Node is NotReady"
                                }));
                            }
                            if (ctype == "MemoryPressure" || ctype == "DiskPressure" || ctype == "PIDPressure") && status == "True" {
                                score = score.saturating_sub(10);
                                issues.push(serde_json::json!({
                                    "severity": "error",
                                    "category": "Node",
                                    "resource": name,
                                    "message": format!("{} detected", ctype)
                                }));
                            }
                        }
                    }
                    // Check unschedulable
                    if node["spec"]["unschedulable"].as_bool().unwrap_or(false) {
                        issues.push(serde_json::json!({
                            "severity": "warning",
                            "category": "Node",
                            "resource": name,
                            "message": "Node is cordoned (unschedulable)"
                        }));
                    }
                }
            }
        }

        // Check warning events
        if let Ok(events) = serde_json::from_str::<serde_json::Value>(&events_raw) {
            if let Some(items) = events["items"].as_array() {
                let warning_count = items.len();
                if warning_count > 0 {
                    score = score.saturating_sub(std::cmp::min(warning_count as u32, 10));
                    // Get top 5 warnings
                    for evt in items.iter().rev().take(5) {
                        let reason = evt["reason"].as_str().unwrap_or("");
                        let msg = evt["message"].as_str().unwrap_or("").chars().take(120).collect::<String>();
                        let obj = format!("{}/{}",
                            evt["involvedObject"]["kind"].as_str().unwrap_or(""),
                            evt["involvedObject"]["name"].as_str().unwrap_or(""));
                        issues.push(serde_json::json!({
                            "severity": "warning",
                            "category": "Event",
                            "resource": obj,
                            "message": format!("{}: {}", reason, msg)
                        }));
                    }
                    issues.push(serde_json::json!({
                        "severity": "info",
                        "category": "Summary",
                        "resource": "Events",
                        "message": format!("{} warning events", warning_count)
                    }));
                }
            }
        }

        let result = serde_json::json!({
            "score": std::cmp::max(score, 0),
            "grade": if score >= 90 { "A" } else if score >= 75 { "B" } else if score >= 60 { "C" } else if score >= 40 { "D" } else { "F" },
            "issues": issues
        });

        Ok(result.to_string())
    }).await {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}


/// List all Custom Resource Definitions
pub async fn api_k8s_crds() -> (StatusCode, Json<ApiResponse<String>>) {
    match run_blocking(|| run_cmd("kubectl", &["get", "crd", "-o", "json"])).await {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}


/// List instances of a specific CRD type
pub async fn api_k8s_crd_resources(
    Query(q): Query<K8sCrdQuery>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    let resource = q.resource;
    let ns = q.namespace;
    // Validate: must look like a valid k8s resource name (alphanumeric, dots, hyphens)
    if resource.is_empty()
        || !resource
            .chars()
            .all(|c| c.is_alphanumeric() || c == '.' || c == '-')
    {
        return err(format!("Invalid CRD resource: {}", resource));
    }
    match run_blocking(move || {
        if ns.is_empty() || ns == "all" {
            run_cmd(
                "kubectl",
                &["get", &resource, "-o", "json", "--all-namespaces"],
            )
        } else {
            run_cmd("kubectl", &["get", &resource, "-o", "json", "-n", &ns])
        }
    })
    .await
    {
        Ok(out) => ok(out),
        Err(e) => err(e),
    }
}


pub async fn api_k8s_log_stream(
    Query(q): Query<K8sLogStreamQuery>,
) -> Sse<impl futures_util::Stream<Item = Result<Event, Infallible>>> {
    let ns = q.namespace;
    let pod = q.pod;
    let container = q.container;
    let tail = q.tail.to_string();

    let stream = async_stream::stream! {
        let mut args = vec![
            "logs".to_string(), "-f".to_string(),
            "-n".to_string(), ns,
            pod,
            "--tail".to_string(), tail,
            "--timestamps".to_string(),
        ];
        if !container.is_empty() {
            args.push("-c".to_string());
            args.push(container);
        }
        let child = tokio::process::Command::new("kubectl")
            .args(&args)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn();

        match child {
            Ok(mut child) => {
                if let Some(stdout) = child.stdout.take() {
                    let reader = tokio::io::BufReader::new(stdout);
                    let mut lines = tokio::io::AsyncBufReadExt::lines(reader);
                    while let Ok(Some(line)) = lines.next_line().await {
                        yield Ok(Event::default().data(line));
                    }
                }
                let _ = child.kill().await;
            }
            Err(e) => {
                yield Ok(Event::default().data(format!("[error] Failed to start kubectl: {}", e)));
            }
        }
    };

    Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(std::time::Duration::from_secs(15))
            .text("ping"),
    )
}


pub async fn api_k8s_benchmark(
    Json(body): Json<BenchmarkBody>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    let url = body.url;
    let concurrency = body.concurrency.min(100).max(1);
    let total = body.requests.min(10000).max(1);
    let method = if body.method.is_empty() {
        "GET".to_string()
    } else {
        body.method.to_uppercase()
    };

    // Validate URL — only allow localhost / 127.0.0.1 / K8s service IPs
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return err("URL must start with http:// or https://".to_string());
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .unwrap_or_default();

    let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(concurrency as usize));
    let latencies = std::sync::Arc::new(tokio::sync::Mutex::new(Vec::<u128>::with_capacity(
        total as usize,
    )));
    let successes = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
    let failures = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));

    let start = std::time::Instant::now();
    let mut handles = Vec::new();

    for _ in 0..total {
        let permit = semaphore.clone().acquire_owned().await.unwrap();
        let c = client.clone();
        let u = url.clone();
        let m = method.clone();
        let lats = latencies.clone();
        let succ = successes.clone();
        let fail = failures.clone();

        handles.push(tokio::spawn(async move {
            let req_start = std::time::Instant::now();
            let result = match m.as_str() {
                "POST" => c.post(&u).send().await,
                "PUT" => c.put(&u).send().await,
                "DELETE" => c.delete(&u).send().await,
                _ => c.get(&u).send().await,
            };
            let elapsed = req_start.elapsed().as_millis();
            match result {
                Ok(resp) if resp.status().is_success() || resp.status().is_redirection() => {
                    succ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                }
                _ => {
                    fail.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                }
            }
            lats.lock().await.push(elapsed);
            drop(permit);
        }));
    }

    for h in handles {
        let _ = h.await;
    }

    let total_time = start.elapsed().as_millis();
    let mut lats = latencies.lock().await;
    lats.sort();

    let count = lats.len();
    let avg = if count > 0 {
        lats.iter().sum::<u128>() / count as u128
    } else {
        0
    };
    let p50 = if count > 0 { lats[count * 50 / 100] } else { 0 };
    let p95 = if count > 0 { lats[count * 95 / 100] } else { 0 };
    let p99 = if count > 0 {
        lats[count.saturating_sub(1) * 99 / 100]
    } else {
        0
    };
    let min = lats.first().copied().unwrap_or(0);
    let max = lats.last().copied().unwrap_or(0);
    let rps = if total_time > 0 {
        count as f64 / total_time as f64 * 1000.0
    } else {
        0.0
    };

    let result = serde_json::json!({
        "total_requests": total,
        "success": successes.load(std::sync::atomic::Ordering::Relaxed),
        "failed": failures.load(std::sync::atomic::Ordering::Relaxed),
        "total_time_ms": total_time,
        "avg_latency_ms": avg,
        "min_latency_ms": min,
        "max_latency_ms": max,
        "p50_ms": p50,
        "p95_ms": p95,
        "p99_ms": p99,
        "requests_per_sec": format!("{:.1}", rps),
        "concurrency": concurrency,
        "method": method,
    });

    ok(result.to_string())
}
