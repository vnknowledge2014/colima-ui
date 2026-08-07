# Colima Security Socket: Procedural Guide

## 1. "Cannot connect to the Docker daemon"
**Problem**: The Docker client (or a tool like docker-compose) returns `Cannot connect to the Docker daemon at unix:///var/run/docker.sock. Is the docker daemon running?`
**Procedure**:
1. Check if the instance is running: `colima status --profile <name>`.
2. Find the correct socket location. By default, Colima places it in `~/.colima/<profile>/docker.sock`.
3. Inform the user they can configure their tools to use this socket by exporting the `DOCKER_HOST` environment variable:
   `export DOCKER_HOST="unix://${HOME}/.colima/default/docker.sock"`
4. Alternatively, use the Colima UI which handles the socket linking transparently.

## 2. Symlink Conflicts
**Problem**: Colima fails to start with an error about symlinking `/var/run/docker.sock`.
**Solution**: Another application (like Docker Desktop or OrbStack) might be locking or owning the `/var/run/docker.sock` file.
**Procedure**:
1. Run `[RUN_APPROVE: ls -l /var/run/docker.sock]` to see what it links to.
2. If the user wants Colima to take ownership, they must quit Docker Desktop, then run:
   `[RUN_APPROVE: sudo rm -f /var/run/docker.sock]`
   `[EVENT_APPROVE: restart-instance | default]`
3. If they don't want to use sudo, they can rely on the `DOCKER_HOST` env var method instead (see section 1).
