//! Docker-layer topology graph: containers and the networks, volumes, compose
//! projects and images they are attached to.
//!
//! This is the Docker graph. The Kubernetes resource graph is a separate thing
//! living in the frontend under `pages/XRay.svelte` — the two are unrelated and
//! must stay that way.
//!
//! The graph is assembled here rather than in the UI so the relationship logic
//! lives in one place and the page costs a single round-trip.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// One vertex of the topology graph.
///
/// `id` is namespaced by kind (`container:abc`, `network:def`) because a volume
/// and a compose project can legitimately share a name.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TopologyNode {
    pub id: String,
    /// container | network | volume | project | image | service
    pub kind: String,
    pub label: String,
    /// running | stopped | unhealthy | notCreated | none
    pub status: String,
    pub meta: serde_json::Value,
}

/// A directed relationship.
///
/// Most edges start at a container, which is the only thing that attaches to
/// networks, volumes and images. `dependsOn` is the exception: it runs between
/// two compose services, either of which may be a container or a service that
/// was never created.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TopologyEdge {
    pub from: String,
    pub to: String,
    /// network | volume | project | image | dependsOn
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TopologyGraph {
    pub nodes: Vec<TopologyNode>,
    pub edges: Vec<TopologyEdge>,
    /// Subsystems that failed to list. A network lookup that errors out looks
    /// exactly like "no networks" in the graph, so the failure is reported
    /// rather than quietly rendering containers with no attachments.
    pub warnings: Vec<String>,
}

/// Read a string field from `docker ps --format json` output, tolerating both
/// the CLI's capitalised keys and the lower-cased shape Bollard produces.
fn field(value: &serde_json::Value, upper: &str, lower: &str) -> String {
    value
        .get(upper)
        .or_else(|| value.get(lower))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

/// Split a Docker CLI comma-separated list field, dropping empties.
fn split_list(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

/// Look up a compose label whether `Labels` arrived as an object (Bollard, or
/// the CLI path after `normalize_cli_labels`) or as a raw `a=1,b=2` string.
fn label(value: &serde_json::Value, key: &str) -> String {
    let labels = match value.get("Labels").or_else(|| value.get("labels")) {
        Some(l) => l,
        None => return String::new(),
    };
    if let Some(obj) = labels.as_object() {
        return obj
            .get(key)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
    }
    if let Some(raw) = labels.as_str() {
        for pair in raw.split(',') {
            if let Some((k, v)) = pair.split_once('=') {
                if k.trim() == key {
                    return v.trim().to_string();
                }
            }
        }
    }
    String::new()
}

/// Collapse Docker's state plus health into the three colours the UI draws.
fn container_status(state: &str, health: &str) -> &'static str {
    if health.eq_ignore_ascii_case("unhealthy") {
        return "unhealthy";
    }
    if state.eq_ignore_ascii_case("running") {
        "running"
    } else {
        "stopped"
    }
}

/// Append an edge unless the same pair was already recorded.
///
/// The `to` id is namespaced by kind, so the pair alone identifies the edge.
fn push_edge(
    edges: &mut Vec<TopologyEdge>,
    seen: &mut HashSet<(String, String)>,
    from: &str,
    to: &str,
    kind: &str,
) {
    if seen.insert((from.to_string(), to.to_string())) {
        edges.push(TopologyEdge {
            from: from.to_string(),
            to: to.to_string(),
            kind: kind.to_string(),
        });
    }
}

/// Build the Docker topology graph for the current engine.
///
/// Container listing is fatal on failure — without containers there is no graph
/// worth drawing. Networks and volumes degrade to empty *with a warning* so a
/// graph still renders when only one subcommand is unhappy, without the UI
/// silently claiming those attachments do not exist.
#[tauri::command]
pub async fn get_topology() -> Result<TopologyGraph, crate::error::ColimaError> {
    let containers = crate::commands::containers::list_containers_cli(true)
        .await
        .map_err(crate::error::ColimaError::from)?;

    let mut warnings: Vec<String> = Vec::new();
    let networks = match crate::commands::networks::list_networks().await {
        Ok(list) => list,
        Err(e) => {
            warnings.push(format!("networks unavailable: {}", e));
            Vec::new()
        }
    };
    let volumes = match crate::commands::volumes::list_volumes().await {
        Ok(list) => list,
        Err(e) => {
            warnings.push(format!("volumes unavailable: {}", e));
            Vec::new()
        }
    };

    let mut nodes: Vec<TopologyNode> = Vec::new();
    let mut edges: Vec<TopologyEdge> = Vec::new();
    // A container can mount the same volume at two paths, which Docker reports
    // as a repeated entry in `Mounts`. Emitting both would produce duplicate
    // edges — invisible in the drawing, but a duplicate-key crash in the
    // renderer's keyed loop.
    let mut seen_edges: HashSet<(String, String)> = HashSet::new();

    // Networks and volumes are keyed by name because that is what a container's
    // `Networks` / `Mounts` fields reference.
    let mut network_ids: HashMap<String, String> = HashMap::new();
    for n in &networks {
        let id = format!("network:{}", n.id);
        network_ids.insert(n.name.clone(), id.clone());
        nodes.push(TopologyNode {
            id,
            kind: "network".into(),
            label: n.name.clone(),
            status: "none".into(),
            meta: serde_json::json!({
                "driver": n.driver,
                "scope": n.scope,
                "internal": n.internal,
            }),
        });
    }

    let mut volume_ids: HashMap<String, String> = HashMap::new();
    for v in &volumes {
        let id = format!("volume:{}", v.name);
        volume_ids.insert(v.name.clone(), id.clone());
        nodes.push(TopologyNode {
            id,
            kind: "volume".into(),
            label: v.name.clone(),
            status: "none".into(),
            meta: serde_json::json!({
                "driver": v.driver,
                "mountpoint": v.mountpoint,
            }),
        });
    }

    // Project and image nodes are derived from the containers that reference
    // them, so every one of them ends up with at least one edge. Projects are
    // taken from labels first and topped up from `docker compose ls` further
    // down, which is what surfaces a project whose containers do not exist yet.
    let mut project_ids: HashMap<String, String> = HashMap::new();
    let mut running_projects: HashSet<String> = HashSet::new();
    let mut image_ids: HashMap<String, String> = HashMap::new();
    // Which container, if any, realises `(project, service)`.
    let mut service_containers: HashMap<(String, String), String> = HashMap::new();

    for c in &containers {
        let cid = field(c, "ID", "id");
        if cid.is_empty() {
            continue;
        }
        let node_id = format!("container:{}", cid);
        let raw_names = field(c, "Names", "names");
        let state = field(c, "State", "state");
        let health = field(c, "HealthStatus", "healthStatus");
        let image = field(c, "Image", "image");
        let status = container_status(&state, &health);

        // container → network. Only link networks we actually listed, so a
        // stale name never invents a phantom node.
        //
        // The first listed network is also reported as the container's primary
        // one: the UI draws containers *inside* that network's box, and picking
        // the group in the renderer instead would mean re-deriving here-only
        // knowledge from the edge list.
        let mut primary_network: Option<String> = None;
        for name in split_list(&field(c, "Networks", "networks")) {
            if let Some(target) = network_ids.get(&name) {
                if primary_network.is_none() {
                    primary_network = Some(target.clone());
                }
                push_edge(&mut edges, &mut seen_edges, &node_id, target, "network");
            }
        }

        nodes.push(TopologyNode {
            id: node_id.clone(),
            kind: "container".into(),
            label: split_list(&raw_names)
                .first()
                .cloned()
                .unwrap_or_else(|| cid.chars().take(12).collect()),
            status: status.into(),
            meta: serde_json::json!({
                "containerId": cid,
                "image": image,
                "state": state,
                "health": health,
                "statusText": field(c, "Status", "status"),
                "ports": field(c, "Ports", "ports"),
                "primaryNetwork": primary_network,
            }),
        });

        // container → volume. `Mounts` mixes bind paths with volume names;
        // matching against the volume list is what separates them.
        for mount in split_list(&field(c, "Mounts", "mounts")) {
            if let Some(target) = volume_ids.get(&mount) {
                push_edge(&mut edges, &mut seen_edges, &node_id, target, "volume");
            }
        }

        // container → compose project
        let project = label(c, "com.docker.compose.project");
        if !project.is_empty() {
            let target = project_ids
                .entry(project.clone())
                .or_insert_with(|| format!("project:{}", project))
                .clone();
            if status == "running" {
                running_projects.insert(project.clone());
            }
            push_edge(&mut edges, &mut seen_edges, &node_id, &target, "project");

            // Remember which container realises which service, so a declared
            // service that already exists is drawn once — as its container —
            // rather than twice, as a container and a phantom beside it.
            let service = label(c, "com.docker.compose.service");
            if !service.is_empty() {
                service_containers.insert((project.clone(), service), node_id.clone());
            }
        }

        // container → image
        if !image.is_empty() {
            let target = image_ids
                .entry(image.clone())
                .or_insert_with(|| format!("image:{}", image))
                .clone();
            push_edge(&mut edges, &mut seen_edges, &node_id, &target, "image");
        }
    }

    // Services the compose files declare. A project that is down has containers
    // nowhere in `docker ps`, so without this the graph shows an empty project —
    // or nothing at all — precisely when the user is trying to work out what
    // failed to come up.
    let compose_projects = match crate::commands::compose::list_compose_projects().await {
        Ok(list) => list,
        Err(e) => {
            warnings.push(format!("compose projects unavailable: {}", e));
            Vec::new()
        }
    };
    let (declared, compose_warnings) =
        crate::commands::compose_services::declared_services(&compose_projects);
    warnings.extend(compose_warnings);

    // Resolve every declared service to the node that represents it: its
    // container when one exists, a not-created placeholder otherwise. Doing this
    // before the edges means `depends_on` can point at either kind without
    // caring which.
    let mut service_nodes: HashMap<(String, String), String> = HashMap::new();
    for svc in &declared {
        let key = (svc.project.clone(), svc.name.clone());
        if let Some(container_id) = service_containers.get(&key) {
            service_nodes.insert(key, container_id.clone());
            continue;
        }
        let node_id = format!("service:{}/{}", svc.project, svc.name);
        nodes.push(TopologyNode {
            id: node_id.clone(),
            kind: "service".into(),
            label: svc.name.clone(),
            status: "notCreated".into(),
            meta: serde_json::json!({
                "project": svc.project,
                "image": svc.image,
                "service": svc.name,
            }),
        });

        // The project node may not exist yet — a fully stopped project has no
        // containers to have created it.
        let project_node = project_ids
            .entry(svc.project.clone())
            .or_insert_with(|| format!("project:{}", svc.project))
            .clone();
        push_edge(
            &mut edges,
            &mut seen_edges,
            &node_id,
            &project_node,
            "project",
        );
        service_nodes.insert(key, node_id);
    }

    for svc in &declared {
        let Some(from) = service_nodes.get(&(svc.project.clone(), svc.name.clone())) else {
            continue;
        };
        for dep in &svc.depends_on {
            // A dependency naming a service the file never defines is a typo in
            // the user's compose file; drawing an edge to a node that does not
            // exist would turn that into a rendering bug instead.
            if let Some(to) = service_nodes.get(&(svc.project.clone(), dep.clone())) {
                push_edge(&mut edges, &mut seen_edges, from, to, "dependsOn");
            }
        }
    }

    for (name, id) in &project_ids {
        nodes.push(TopologyNode {
            id: id.clone(),
            kind: "project".into(),
            label: name.clone(),
            status: if running_projects.contains(name) {
                "running".into()
            } else {
                "stopped".into()
            },
            meta: serde_json::json!({ "project": name }),
        });
    }

    for (image, id) in &image_ids {
        nodes.push(TopologyNode {
            id: id.clone(),
            kind: "image".into(),
            label: image.clone(),
            status: "none".into(),
            meta: serde_json::json!({ "image": image }),
        });
    }

    Ok(TopologyGraph {
        nodes,
        edges,
        warnings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn health_overrides_running_state() {
        assert_eq!(container_status("running", "unhealthy"), "unhealthy");
        assert_eq!(container_status("running", "healthy"), "running");
        assert_eq!(container_status("exited", ""), "stopped");
    }

    #[test]
    fn labels_read_from_both_shapes() {
        let as_object = serde_json::json!({
            "Labels": { "com.docker.compose.project": "web" }
        });
        let as_string = serde_json::json!({
            "Labels": "a=1,com.docker.compose.project=web,b=2"
        });
        assert_eq!(label(&as_object, "com.docker.compose.project"), "web");
        assert_eq!(label(&as_string, "com.docker.compose.project"), "web");
        assert_eq!(label(&as_string, "missing"), "");
    }

    /// `docker run -v cache:/a -v cache:/b` reports `Mounts: "cache,cache"`.
    /// Two identical edges are invisible in the drawing but crash the renderer's
    /// keyed loop, so they must never leave this module.
    #[test]
    fn repeated_mount_yields_one_edge() {
        let mut edges = Vec::new();
        let mut seen = HashSet::new();
        push_edge(&mut edges, &mut seen, "container:a", "volume:cache", "volume");
        push_edge(&mut edges, &mut seen, "container:a", "volume:cache", "volume");
        assert_eq!(edges.len(), 1);

        // A different container mounting the same volume is a real second edge.
        push_edge(&mut edges, &mut seen, "container:b", "volume:cache", "volume");
        assert_eq!(edges.len(), 2);
    }

    /// A container carries both compose labels, and both must be readable —
    /// the service label is what stops a running service being drawn twice,
    /// once as its container and once as a not-created phantom beside it.
    #[test]
    fn service_label_is_read_alongside_project() {
        let as_object = serde_json::json!({
            "Labels": {
                "com.docker.compose.project": "shop",
                "com.docker.compose.service": "api"
            }
        });
        assert_eq!(label(&as_object, "com.docker.compose.service"), "api");

        let as_string = serde_json::json!({
            "Labels": "com.docker.compose.project=shop,com.docker.compose.service=api"
        });
        assert_eq!(label(&as_string, "com.docker.compose.service"), "api");
    }

    #[test]
    fn split_list_drops_blanks() {
        assert_eq!(split_list(" a , ,b "), vec!["a".to_string(), "b".to_string()]);
        assert!(split_list("").is_empty());
    }
}
