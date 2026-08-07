---
name: colima-manager
description: Control and orchestrate Colima UI via HTTP API capabilities
---

# Colima Manager Skill

This skill allows orchestrators and external agents to control the Colima UI backend via its HTTP API.

## Requirements

The ColimaUI backend must be running on `http://localhost:11420`.

## Discovery

Start by fetching the capabilities schema from the backend:
```bash
curl -s http://localhost:11420/api/capabilities
```
This will return a JSON object outlining the available endpoints and the event bus.

## Running Commands

To execute commands safely and stream the output back to avoid timeouts, use the `execute_stream` endpoint:

```bash
curl -s -N -X POST http://localhost:11420/api/cli/execute_stream \
  -H "Content-Type: application/json" \
  -d '{"command": "docker ps"}'
```

The output will be streamed as Server-Sent Events (SSE) with event types like `stdout`, `stderr`, and `exit`.

## Common Capabilities

- K8s management
- Colima instance lifecycle (start, stop, etc.)
- Docker containers lifecycle
- Diagnostics and system specifications

Check the capabilities schema for the full up-to-date list of endpoints and event categories.
