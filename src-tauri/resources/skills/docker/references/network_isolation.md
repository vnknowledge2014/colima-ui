# Docker Network Isolation: Procedural Guide

## 1. Inter-Container Communication
**Problem**: Two containers cannot ping each other by name.
**Solution**: Containers on the default `bridge` network can only communicate by IP address, which changes. DNS resolution is disabled on the default bridge.
**Procedure**:
1. Create a custom bridge network:
   `[RUN_APPROVE: docker network create my-app-net]`
2. Attach both containers to this network:
   `[RUN_APPROVE: docker network connect my-app-net <container1>]`
   `[RUN_APPROVE: docker network connect my-app-net <container2>]`
3. Containers can now communicate using their container names as hostnames.

## 2. Diagnosing Network Issues
**Problem**: A container cannot reach the internet or a database on the host machine.
**Procedure**:
1. Inspect the container's network settings:
   `[RUN_APPROVE: docker inspect <container> --format='{{json .NetworkSettings.Networks}}']`
2. Enter the container and test connectivity:
   `[RUN_APPROVE: docker exec -it <container> ping -c 3 8.8.8.8]`
3. Reaching services on the Mac host:
   Use the special DNS name `host.docker.internal` from inside the container. This resolves to the internal IP address used by the host.

## 3. Network Cleanup
**Problem**: Docker complains about overlapping IP ranges or network exhaustion.
**Procedure**:
1. List unused networks:
   `[RUN_APPROVE: docker network ls -f "type=custom"]`
2. Remove unused networks safely:
   `[RUN_APPROVE: docker network prune]`
