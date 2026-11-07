# Colima Mount Volumes: Procedural Guide

## 1. Changing Mount Types (sshfs vs virtiofs)
**Problem**: Disk I/O inside the container is too slow when compiling code or running databases.
**Solution**: Use `virtiofs` on macOS (requires VZ mode) instead of `sshfs`.
**Procedure**:
1. Check the current mount type: `[QUERY: colima-status | {"profile": "<name>"}]` (look for `mount_type` / `mountType`).
2. To change it, guide the user to `[NAVIGATE: settings]` (Colima Config editor, which resolves `$COLIMA_HOME`, Colima `0.10.x` XDG `~/.config/colima/<name>/colima.yaml`, or legacy `~/.colima/<name>/colima.yaml`) and update:
   `mountType: virtiofs` (Requires `vmType: vz`).
3. Restart the instance to apply changes: `[EVENT_APPROVE: colima-restart | {"profile": "<name>"}]`

## 2. File Permission Denied on Host Mounts
**Problem**: A container cannot write to a host-mounted directory.
**Procedure**:
1. Verify how the directory is mounted in `colima.yaml` via `[NAVIGATE: settings]`. By default, Colima mounts the user's home directory (`~`) as writable.
2. If the path is outside `~` (e.g., `/tmp` or an external drive), ensure it is added to the `mounts` array in `colima.yaml`:
   ```yaml
   mounts:
     - location: /Volumes/ExternalDrive
       writable: true
   ```
3. After changing `mounts`, restart the instance: `[EVENT_APPROVE: colima-restart | {"profile": "<name>"}]`
