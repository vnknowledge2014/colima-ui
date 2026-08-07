# Colima Mount Volumes: Procedural Guide

## 1. Changing Mount Types (sshfs vs virtiofs)
**Problem**: Disk I/O inside the container is too slow when compiling code or running databases.
**Solution**: Use `virtiofs` on macOS (requires VZ mode) instead of `sshfs`.
**Procedure**:
1. Check the current mount type: `colima status --profile <name>` (Look for mountType).
2. To change it, the user must update their Preset or edit the config:
   `mountType: virtiofs` (Requires `vmType: vz`).
3. The instance must be restarted. `[EVENT_APPROVE: restart-instance | <name>]`

## 2. File Permission Denied on Host Mounts
**Problem**: A container cannot write to a host-mounted directory.
**Procedure**:
1. Verify how the directory is mounted in `colima.yaml`. By default, Colima mounts the user's home directory (`~`) as writable.
2. If the path is outside `~` (e.g., `/tmp` or an external drive), ensure it is added to the `mounts` array in `colima.yaml`.
   ```yaml
   mounts:
     - location: /Volumes/ExternalDrive
       writable: true
   ```
3. After changing `mounts`, restart the instance.
