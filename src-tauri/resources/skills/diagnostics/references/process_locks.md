# Process Locks & Zombie VMs: Procedural Guide

## 1. Finding Stale Socket Locks
**Problem**: Colima won't start, error mentions `ha.sock: bind: address already in use`.
**Procedure**:
1. Check `[QUERY: list-instances]` and `[QUERY: environment-resolutions]` first to confirm the VM is stopped and obtain the concrete `<colima-home>` path (`$COLIMA_HOME`, Colima `0.10.x` XDG `~/.config/colima`, or legacy `~/.colima`; Lima stores instances under `<colima-home>/_lima/colima` for `default` or `<colima-home>/_lima/colima-<profile>` for named profiles — or `$LIMA_HOME`).
2. Using the concrete absolute path from `environment-resolutions` (never `~`, because the shell sandbox does not invoke a shell for tilde expansion), check whether the host-agent socket still exists:
   `[RUN: ls -l /Users/<user>/.config/colima/_lima/colima/ha.sock]`
3. If the VM is confirmed stopped and no Lima hostagent is running (check section 2), instruct the user to remove the stale `ha.sock` file manually in `[NAVIGATE: terminal]` or their macOS terminal (arbitrary `rm` outside `/var/run/docker.sock` is blocked by the shell sandbox), then start the instance again.

## 2. Killing Zombie QEMU/VZ Processes
**Problem**: Colima fails to start because the hypervisor is already running, or memory is fully consumed by a hung VM process.
**Procedure**:
1. Find running Lima/QEMU/VZ processes without shell pipes (`|` is blocked by the sandbox):
   `[RUN: pgrep -fl lima]`
2. Identify the PID of the stuck `limactl`, `qemu-system-aarch64`, or `vz` process.
3. Propose terminating it cleanly first:
   `[RUN_APPROVE: kill <PID>]`
4. Only if it refuses to exit, force kill:
   `[RUN_APPROVE: kill -9 <PID>]`
5. After terminating the zombie process, restart or start the instance via `[EVENT_APPROVE: start-instance | {"profile": "default"}]`.
