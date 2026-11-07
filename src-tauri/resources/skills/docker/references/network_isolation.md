# Docker Network Isolation: Procedural Guide

## 1. Inter-Container Communication
**Problem**: Two containers cannot ping each other by name.
**Solution**: Containers on the default `bridge` network can only communicate by IP address, which changes. DNS resolution is disabled on the default bridge.
**Procedure**:
1. Create a custom bridge network via the App Event Bus:
   `[EVENT_APPROVE: network-create | {"name": "my-app-net"}]`
2. Attach existing containers to this network (or launch new containers on `my-app-net`):
   `[EVENT_APPROVE: cli-exec | {"command": "docker", "args": ["network", "connect", "my-app-net", "<container1>"]}]`
   `[EVENT_APPROVE: cli-exec | {"command": "docker", "args": ["network", "connect", "my-app-net", "<container2>"]}]`
3. Containers can now communicate using their container names as hostnames (inspect visually in `[NAVIGATE: topology]`).

## 2. Diagnosing Network Issues
**Problem**: A container cannot reach the internet or a database on the host machine.
**Procedure**:
1. Inspect the container's `NetworkSettings.Networks` via the App Event Bus:
   `[QUERY: inspect-container | {"id": "<container>"}]`
2. Run a non-interactive connectivity check inside the container (never use `-it` outside a PTY; for an interactive shell, open `[NAVIGATE: containers]` or `[NAVIGATE: terminal]`):
   `[EVENT_APPROVE: container-exec | {"id": "<container>", "command": "ping -c 3 8.8.8.8"}]`
3. Reaching services on the Mac host:
   Use the special DNS name `host.docker.internal` from inside the container. This resolves to the internal IP address used by the host.

## 3. Network Cleanup
**Problem**: Docker complains about overlapping IP ranges or network exhaustion.
**Procedure**:
1. List existing Docker networks:
   `[QUERY: network-list]`
2. Remove unused networks safely:
   `[EVENT_APPROVE: network-prune]`
