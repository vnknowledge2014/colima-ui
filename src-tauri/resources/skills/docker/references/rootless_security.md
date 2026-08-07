# Docker Rootless Security in Colima: Procedural Guide

## 1. Permission Denied Errors
**Problem**: A container fails to start with `Permission denied` on a volume mount.
**Solution**: Docker in Colima runs in rootless mode by default. The internal container user (often root, UID 0) is mapped to the unprivileged user on the host. If the host directory doesn't allow access by the mapped UID, it fails.
**Procedure**:
1. Check the mapped UID/GID inside the container shell:
   `[RUN_APPROVE: docker exec <container> id]`
2. Adjust permissions on the host directory to allow the mapped UID.
3. Alternatively, force the container to run as the correct UID by modifying the run command or Compose file:
   `user: "1000:1000"`

## 2. Privileged Ports (Under 1024)
**Problem**: Container cannot bind to ports like 80 or 443.
**Solution**: Rootless Docker cannot bind to privileged ports on the VM interface directly without configuration.
**Procedure**:
- Use higher ports (e.g., 8080) and map them: `-p 8080:80`
- Colima handles the `localhost` forwarding to the Mac host automatically, bypassing the privileged port restriction on the Mac side, but the container *inside* the VM still binds to the unprivileged port.

## 3. Host Network Mode
**Problem**: `--network host` doesn't expose ports to the Mac `localhost`.
**Solution**: In rootless Docker (and Docker for Mac / Colima in general), the "host" network is the VM's network, NOT the Mac's network.
**Procedure**: Always explicitly map ports `-p <host_port>:<container_port>` rather than relying on `--network host`.
