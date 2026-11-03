use axum::{http::StatusCode, response::Json};
use crate::api_server::{ApiResponse, ok};
use serde_json::json;

pub async fn api_capabilities() -> (StatusCode, Json<ApiResponse<serde_json::Value>>) {
    let caps = json!({
        "version": "1.1",
        "description": "ColimaUI capabilities schema for AI Agents",
        "endpoints": {
            "/api/health": "GET - Liveness probe (unauthenticated)",
            "/api/capabilities": "GET - Fetch this schema",
            "/api/cli/chat": "POST - Headless CLI natural-language prompt handler",
            "/api/ai/chat": "POST - Chat with configured LLM",
            "/api/ai/conversations": "GET, POST - List or create AI conversations",
            "/api/ai/history": "GET, POST - Load or append AI conversation history",
            "/api/sandbox/execute": "POST - Classify and execute safe shell commands",
            "/api/sandbox/execute-approved": "POST - Execute user-approved shell commands",
            "/api/kb/query": "POST - Query knowledge bank",
            "/api/kb/search": "POST - Search user memories",
            "/api/settings": "GET, POST - Read/write application settings",
            "/api/instances": "GET - List Colima instances",
            "/api/instances/status": "GET - Get status of a Colima instance",
            "/api/instances/start": "POST - Start a Colima instance",
            "/api/instances/stop": "POST - Stop a Colima instance",
            "/api/containers": "GET - List Docker containers",
            "/api/images": "GET - List Docker images",
            "/api/volumes": "GET - List Docker volumes",
            "/api/networks": "GET - List Docker networks",
            "/api/compose": "GET - List Docker Compose projects",
            "/api/compose/diagnose": "POST - Diagnose Docker Compose issues",
            "/api/k8s/pods": "GET - List Kubernetes pods",
            "/api/lima": "GET - List Lima VMs",
            "/api/security/scan": "POST - Scan container image for vulnerabilities",
            "/api/topology": "GET - Fetch container, network, and volume topology graph",
            "/api/activity": "GET - Query local action history",
            "/api/diagnostics/bundle": "POST - Generate redacted diagnostic bundle",
            "/api/environment/resolutions": "GET - Inspect path/socket resolution provenance and launchd-vs-shell environment duality",
            "/api/system/reference": "GET - Read bundled CLI reference docs",
            "/api/events": "GET - Subscribe to SSE event stream (?token=...&topics=...)",
        },
        "event_bus": {
            "list-instances": { "category": "SAFE", "description": "List all Colima instances" },
            "start-instance": { "category": "NORMAL", "description": "Start an existing Colima instance" },
            "stop-instance": { "category": "NORMAL", "description": "Stop a Colima instance" },
            "delete-instance": { "category": "DANGEROUS", "description": "Delete a Colima instance permanently" },
            "list-containers": { "category": "SAFE", "description": "List all containers" },
            "stop-container": { "category": "NORMAL", "description": "Stop a container" },
            "remove-container": { "category": "DANGEROUS", "description": "Remove a container" },
            "k8s-list-resources": { "category": "SAFE", "description": "List resources of a specific type in a namespace" },
            "k8s-delete": { "category": "DANGEROUS", "description": "Delete a Kubernetes resource" },
            "system-host-specs": { "category": "SAFE", "description": "Get host specifications" },
            "cli-exec": { "category": "NORMAL", "description": "Execute a command through the App CLI gateway" }
        }
    });

    ok(caps)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn capabilities_schema_lists_real_routes_and_events_only() {
        let (status, Json(resp)) = api_capabilities().await;
        assert_eq!(status, StatusCode::OK);
        let data = resp.data.expect("capabilities payload must be present");

        let endpoints = data["endpoints"].as_object().expect("endpoints map");
        assert!(!endpoints.contains_key("/api/cli/execute_stream"));
        assert!(!endpoints.contains_key("/api/auth/token"));
        for required in [
            "/api/health",
            "/api/capabilities",
            "/api/cli/chat",
            "/api/ai/chat",
            "/api/ai/conversations",
            "/api/ai/history",
            "/api/sandbox/execute",
            "/api/sandbox/execute-approved",
            "/api/instances",
            "/api/instances/status",
            "/api/instances/start",
            "/api/instances/stop",
            "/api/containers",
            "/api/images",
            "/api/volumes",
            "/api/networks",
            "/api/compose",
            "/api/compose/diagnose",
            "/api/k8s/pods",
            "/api/lima",
            "/api/security/scan",
            "/api/topology",
            "/api/activity",
            "/api/diagnostics/bundle",
            "/api/system/reference",
            "/api/events",
        ] {
            assert!(endpoints.contains_key(required), "missing endpoint {required}");
        }

        let events = data["event_bus"].as_object().expect("event_bus map");
        assert!(!events.contains_key("colima-start"));
        assert!(!events.contains_key("docker-list"));
        assert_eq!(events["list-instances"]["category"], "SAFE");
        assert_eq!(events["start-instance"]["category"], "NORMAL");
        assert_eq!(events["stop-instance"]["category"], "NORMAL");
        assert_eq!(events["delete-instance"]["category"], "DANGEROUS");
        assert_eq!(events["list-containers"]["category"], "SAFE");
        assert_eq!(events["stop-container"]["category"], "NORMAL");
        assert_eq!(events["k8s-list-resources"]["category"], "SAFE");
        assert_eq!(events["system-host-specs"]["category"], "SAFE");
        assert_eq!(events["cli-exec"]["category"], "NORMAL");
    }
}
