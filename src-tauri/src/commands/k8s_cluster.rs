//! Kubernetes commands scoped to the cluster itself — contexts, nodes, events,
//! port forwards, and the health/benchmark reports.
//!
//! Like [`crate::commands::k8s_resources`], these existed only as HTTP routes,
//! so they worked in browser mode and failed in the desktop app.
//!
//! `cluster_health_report` and `run_benchmark` are `pub` and deliberately not
//! `#[tauri::command]`: `routes/k8s.rs` calls them too. They are long enough
//! that a second copy would drift, which is the same failure that produced the
//! duplicated `.ai-panel` CSS.

use crate::error::ColimaError;
use crate::helpers::{run_blocking, run_cmd, PORT_FORWARDS};
use crate::validation::is_valid_k8s_name;

// ===== Contexts =====
//
// Context names are NOT validated. Unlike a namespace or a pod, a kubeconfig
// context can legitimately contain `/`, `:` and `@` — an EKS context is a full
// ARN. `is_valid_k8s_name` would reject those and lock those users out. The
// value is an argv element, never shell input, so there is nothing to escape.

#[tauri::command]
pub async fn k8s_contexts() -> Result<String, ColimaError> {
    run_blocking(|| run_cmd("kubectl", &["config", "get-contexts", "-o", "name"]))
        .await
        .map_err(ColimaError::from)
}

#[tauri::command]
pub async fn k8s_current_context() -> Result<String, ColimaError> {
    run_blocking(|| run_cmd("kubectl", &["config", "current-context"]))
        .await
        .map_err(ColimaError::from)
}

#[tauri::command]
pub async fn k8s_set_context(context: String) -> Result<String, ColimaError> {
    async move {
        if context.is_empty() {
            return Err("Context name is empty".to_string());
        }
        run_blocking(move || run_cmd("kubectl", &["config", "use-context", &context])).await
    }
    .await
    .map_err(ColimaError::from)
}

// ===== Nodes and events =====

#[tauri::command]
pub async fn k8s_nodes_json() -> Result<String, ColimaError> {
    run_blocking(|| run_cmd("kubectl", &["get", "nodes", "-o", "json"]))
        .await
        .map_err(ColimaError::from)
}

#[tauri::command]
pub async fn k8s_events_json(namespace: String) -> Result<String, ColimaError> {
    async move {
        if !namespace.is_empty() && namespace != "all" && !is_valid_k8s_name(&namespace) {
            return Err(format!("Invalid namespace: {namespace}"));
        }
        run_blocking(move || {
            if namespace.is_empty() || namespace == "all" {
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
                run_cmd(
                    "kubectl",
                    &["get", "events", "-o", "json", "-n", &namespace],
                )
            }
        })
        .await
    }
    .await
    .map_err(ColimaError::from)
}

/// Cordon, uncordon or drain a node.
///
/// The action is matched against a closed set rather than interpolated, so an
/// unknown value fails here instead of becoming an unintended kubectl verb.
#[tauri::command]
pub async fn k8s_node_action(name: String, action: String) -> Result<String, ColimaError> {
    async move {
        if !is_valid_k8s_name(&name) {
            return Err(format!("Invalid node name: {name}"));
        }
        run_blocking(move || match action.as_str() {
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
            _ => Err(format!("Unknown action: {action}")),
        })
        .await
    }
    .await
    .map_err(ColimaError::from)
}

// ===== Port forwards =====

#[tauri::command]
pub async fn k8s_pf_list() -> Result<String, ColimaError> {
    run_blocking(|| {
        let fwds = PORT_FORWARDS.lock().map_err(|e| format!("{e}"))?;
        Ok(fwds
            .iter()
            .map(|(port, pid)| format!("{port}:{pid}"))
            .collect::<Vec<_>>()
            .join("\n"))
    })
    .await
    .map_err(ColimaError::from)
}

#[tauri::command]
pub async fn k8s_pf_start(
    namespace: String,
    name: String,
    local_port: u32,
    remote_port: u32,
    resource_type: String,
) -> Result<String, ColimaError> {
    async move {
        if !is_valid_k8s_name(&namespace) {
            return Err(format!("Invalid namespace: {namespace}"));
        }
        if !is_valid_k8s_name(&name) {
            return Err(format!("Invalid resource name: {name}"));
        }
        if !is_valid_k8s_name(&resource_type) {
            return Err(format!("Invalid resource type: {resource_type}"));
        }
        run_blocking(move || {
            use std::process::{Command, Stdio};

            let target = format!("{resource_type}/{name}");
            let ports = format!("{local_port}:{remote_port}");

            // Output is discarded: kubectl port-forward writes a line per
            // connection, and nothing reads it. Left on a pipe it would fill
            // the buffer and block the forward.
            let child = Command::new("kubectl")
                .args(["port-forward", "-n", &namespace, &target, &ports])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .map_err(|e| format!("Failed to start port-forward: {e}"))?;

            if let Ok(mut fwds) = PORT_FORWARDS.lock() {
                fwds.insert(local_port.to_string(), child.id());
            }
            Ok(format!(
                "Port forward started: localhost:{local_port} → {ports}"
            ))
        })
        .await
    }
    .await
    .map_err(ColimaError::from)
}

#[tauri::command]
pub async fn k8s_pf_stop(local_port: u32) -> Result<String, ColimaError> {
    run_blocking(move || {
        if let Ok(mut fwds) = PORT_FORWARDS.lock() {
            if let Some(pid) = fwds.remove(&local_port.to_string()) {
                #[cfg(unix)]
                unsafe {
                    libc::kill(pid as i32, libc::SIGTERM);
                }
                return Ok(format!("Port forward on {local_port} stopped"));
            }
        }

        // Not in the registry — the app restarted while the forward survived.
        // Fall back to whatever holds the port.
        let _ = std::process::Command::new("lsof")
            .args(["-ti", &format!(":{local_port}")])
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
        Ok(format!("Port forward on {local_port} stopped"))
    })
    .await
    .map_err(ColimaError::from)
}

// ===== Reports =====

#[tauri::command]
pub async fn k8s_cluster_health() -> Result<String, ColimaError> {
    run_blocking(cluster_health_report)
        .await
        .map_err(ColimaError::from)
}

#[tauri::command]
pub async fn k8s_benchmark(
    url: String,
    concurrency: u32,
    requests: u32,
    method: String,
) -> Result<String, ColimaError> {
    run_benchmark(url, concurrency, requests, method)
        .await
        .map_err(ColimaError::from)
}

/// Score a cluster out of 100 and list what dragged it down.
///
/// Each `kubectl` call is `unwrap_or_default()`ed on purpose: a cluster missing
/// PVCs, or an RBAC that forbids listing events, should still produce a report
/// from whatever else is readable rather than failing outright.
pub fn cluster_health_report() -> Result<String, String> {
    let pods_raw = run_cmd(
        "kubectl",
        &["get", "pods", "--all-namespaces", "-o", "json"],
    )
    .unwrap_or_default();
    let deploys_raw = run_cmd(
        "kubectl",
        &["get", "deployments", "--all-namespaces", "-o", "json"],
    )
    .unwrap_or_default();
    let pvcs_raw = run_cmd("kubectl", &["get", "pvc", "--all-namespaces", "-o", "json"])
        .unwrap_or_default();
    let events_raw = run_cmd(
        "kubectl",
        &[
            "get",
            "events",
            "--all-namespaces",
            "--field-selector=type=Warning",
            "-o",
            "json",
        ],
    )
    .unwrap_or_default();
    let nodes_raw = run_cmd("kubectl", &["get", "nodes", "-o", "json"]).unwrap_or_default();

    let mut issues: Vec<serde_json::Value> = Vec::new();
    let mut score: u32 = 100;

    if let Ok(pods) = serde_json::from_str::<serde_json::Value>(&pods_raw) {
        if let Some(items) = pods["items"].as_array() {
            let total_pods = items.len();
            let mut unhealthy = 0u32;
            for pod in items {
                let phase = pod["status"]["phase"].as_str().unwrap_or("");
                let name = pod["metadata"]["name"].as_str().unwrap_or("");
                let ns = pod["metadata"]["namespace"].as_str().unwrap_or("");

                if phase == "Failed" || phase == "Unknown" {
                    unhealthy += 1;
                    issues.push(serde_json::json!({
                        "severity": "error",
                        "category": "Pod",
                        "resource": format!("{ns}/{name}"),
                        "message": format!("Pod is in {phase} phase")
                    }));
                } else if phase == "Pending" {
                    unhealthy += 1;
                    issues.push(serde_json::json!({
                        "severity": "warning",
                        "category": "Pod",
                        "resource": format!("{ns}/{name}"),
                        "message": "Pod is pending"
                    }));
                }

                if let Some(statuses) = pod["status"]["containerStatuses"].as_array() {
                    for cs in statuses {
                        let restarts = cs["restartCount"].as_u64().unwrap_or(0);
                        let ready = cs["ready"].as_bool().unwrap_or(false);
                        let cname = cs["name"].as_str().unwrap_or("");
                        if restarts > 5 {
                            issues.push(serde_json::json!({
                                "severity": "warning",
                                "category": "Pod",
                                "resource": format!("{ns}/{name}"),
                                "message": format!("Container {cname} has {restarts} restarts")
                            }));
                        }
                        if !ready {
                            issues.push(serde_json::json!({
                                "severity": "warning",
                                "category": "Pod",
                                "resource": format!("{ns}/{name}"),
                                "message": format!("Container {cname} is not ready")
                            }));
                        }
                        if let Some(waiting) = cs["state"]["waiting"]["reason"].as_str() {
                            if waiting == "CrashLoopBackOff"
                                || waiting == "ImagePullBackOff"
                                || waiting == "ErrImagePull"
                            {
                                issues.push(serde_json::json!({
                                    "severity": "error",
                                    "category": "Pod",
                                    "resource": format!("{ns}/{name}"),
                                    "message": format!("Container {cname} in {waiting}")
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
                "message": format!("{total_pods} total, {unhealthy} unhealthy")
            }));
        }
    }

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
                        "resource": format!("{ns}/{name}"),
                        "message": format!("Only {ready}/{desired} replicas ready")
                    }));
                }
                if available < desired {
                    issues.push(serde_json::json!({
                        "severity": "warning",
                        "category": "Deployment",
                        "resource": format!("{ns}/{name}"),
                        "message": format!("Only {available}/{desired} replicas available")
                    }));
                }
            }
        }
    }

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
                        "resource": format!("{ns}/{name}"),
                        "message": format!("PVC is {phase}")
                    }));
                }
            }
        }
    }

    if let Ok(nodes) = serde_json::from_str::<serde_json::Value>(&nodes_raw) {
        if let Some(items) = nodes["items"].as_array() {
            for node in items {
                let name = node["metadata"]["name"].as_str().unwrap_or("");
                if let Some(conds) = node["status"]["conditions"].as_array() {
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
                        if (ctype == "MemoryPressure"
                            || ctype == "DiskPressure"
                            || ctype == "PIDPressure")
                            && status == "True"
                        {
                            score = score.saturating_sub(10);
                            issues.push(serde_json::json!({
                                "severity": "error",
                                "category": "Node",
                                "resource": name,
                                "message": format!("{ctype} detected")
                            }));
                        }
                    }
                }
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

    if let Ok(events) = serde_json::from_str::<serde_json::Value>(&events_raw) {
        if let Some(items) = events["items"].as_array() {
            let warning_count = items.len();
            if warning_count > 0 {
                score = score.saturating_sub(std::cmp::min(warning_count as u32, 10));
                for evt in items.iter().rev().take(5) {
                    let reason = evt["reason"].as_str().unwrap_or("");
                    let msg = evt["message"]
                        .as_str()
                        .unwrap_or("")
                        .chars()
                        .take(120)
                        .collect::<String>();
                    let obj = format!(
                        "{}/{}",
                        evt["involvedObject"]["kind"].as_str().unwrap_or(""),
                        evt["involvedObject"]["name"].as_str().unwrap_or("")
                    );
                    issues.push(serde_json::json!({
                        "severity": "warning",
                        "category": "Event",
                        "resource": obj,
                        "message": format!("{reason}: {msg}")
                    }));
                }
                issues.push(serde_json::json!({
                    "severity": "info",
                    "category": "Summary",
                    "resource": "Events",
                    "message": format!("{warning_count} warning events")
                }));
            }
        }
    }

    Ok(serde_json::json!({
        "score": score,
        "grade": grade_for(score),
        "issues": issues
    })
    .to_string())
}

fn grade_for(score: u32) -> &'static str {
    match score {
        90..=u32::MAX => "A",
        75..=89 => "B",
        60..=74 => "C",
        40..=59 => "D",
        _ => "F",
    }
}

/// Fire `requests` HTTP requests at `url`, `concurrency` at a time, and report
/// latency percentiles.
pub async fn run_benchmark(
    url: String,
    concurrency: u32,
    requests: u32,
    method: String,
) -> Result<String, String> {
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err("URL must start with http:// or https://".to_string());
    }

    // Clamped so a mistyped field cannot turn into a self-inflicted DoS.
    let concurrency = concurrency.clamp(1, 100);
    let total = requests.clamp(1, 10_000);
    let method = if method.is_empty() {
        "GET".to_string()
    } else {
        method.to_uppercase()
    };

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
        let permit = semaphore
            .clone()
            .acquire_owned()
            .await
            .map_err(|e| format!("Benchmark semaphore closed: {e}"))?;
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
    lats.sort_unstable();

    let count = lats.len();
    let pct = |p: usize| -> u128 {
        if count == 0 {
            0
        } else {
            // Index from the last element so p100 cannot run off the end.
            lats[((count - 1) * p) / 100]
        }
    };
    let avg = if count > 0 {
        lats.iter().sum::<u128>() / count as u128
    } else {
        0
    };
    let rps = if total_time > 0 {
        count as f64 / total_time as f64 * 1000.0
    } else {
        0.0
    };

    Ok(serde_json::json!({
        "total_requests": total,
        "success": successes.load(std::sync::atomic::Ordering::Relaxed),
        "failed": failures.load(std::sync::atomic::Ordering::Relaxed),
        "total_time_ms": total_time,
        "avg_latency_ms": avg,
        "min_latency_ms": lats.first().copied().unwrap_or(0),
        "max_latency_ms": lats.last().copied().unwrap_or(0),
        "p50_ms": pct(50),
        "p95_ms": pct(95),
        "p99_ms": pct(99),
        "requests_per_sec": format!("{rps:.1}"),
        "concurrency": concurrency,
        "method": method,
    })
    .to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grades_span_the_whole_range() {
        assert_eq!(grade_for(100), "A");
        assert_eq!(grade_for(90), "A");
        assert_eq!(grade_for(89), "B");
        assert_eq!(grade_for(60), "C");
        assert_eq!(grade_for(40), "D");
        assert_eq!(grade_for(0), "F");
    }

    #[tokio::test]
    async fn benchmark_rejects_non_http_urls() {
        let e = run_benchmark("file:///etc/passwd".into(), 1, 1, "GET".into())
            .await
            .unwrap_err();
        assert!(e.contains("http://"), "unexpected error: {e}");
    }
}
