# Colima Security Socket: Procedural Guide

## 1. "Cannot connect to the Docker daemon"
**Problem**: The Docker client (or a tool like docker-compose) returns `Cannot connect to the Docker daemon at unix:///var/run/docker.sock. Is the docker daemon running?`
**Procedure**:
1. Check if the instance is running: `[QUERY: colima-status | {"profile": "default"}]`.
2. Inspect the active resolved socket path via `[QUERY: environment-resolutions]` (or `[NAVIGATE: environment]`). Depending on Colima version and `$COLIMA_HOME`, Colima places the socket at `~/.config/colima/<profile>/docker.sock` (Colima `0.10.x` XDG default) or legacy `~/.colima/<profile>/docker.sock`.
3. Inform the user they can configure external CLI tools to use the resolved socket by exporting `DOCKER_HOST` (or switching Docker context with `docker context use colima`):
   `export DOCKER_HOST="unix://${HOME}/.config/colima/default/docker.sock"` *(or `unix://${HOME}/.colima/default/docker.sock` on legacy layouts)*
4. Alternatively, use ColimaUI directly, which resolves and connects to the active Colima socket automatically.

## 2. Symlink Conflicts
**Problem**: External tools expecting `/var/run/docker.sock` connect to the wrong runtime or fail due to a stale symlink.
**Solution**: Another application (like Docker Desktop or OrbStack) might be owning `/var/run/docker.sock`.
**Procedure**:
1. Run `[RUN: ls -l /var/run/docker.sock]` to inspect what `/var/run/docker.sock` links to and who owns it.
2. If the user wants to remove a stale user-writable `/var/run/docker.sock` after quitting Docker Desktop/OrbStack, run:
   `[RUN_APPROVE: rm -f /var/run/docker.sock]`
   `[EVENT_APPROVE: colima-restart | {"profile": "default"}]`
   *(Note: `sudo` is blocked by the shell sandbox — if `/var/run/docker.sock` is owned by `root`, ask the user to remove it in their own terminal or rely on `docker context use colima` / `DOCKER_HOST` from section 1.)*
