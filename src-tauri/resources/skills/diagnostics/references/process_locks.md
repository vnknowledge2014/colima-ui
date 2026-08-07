# Process Locks & Zombie VMs: Procedural Guide

## 1. Finding Stale Socket Locks
**Problem**: Colima won't start, error mentions `ha.sock: bind: address already in use`.
**Procedure**:
1. Run `[RUN_APPROVE: ls -l ~/.colima/<profile>/.lima/colima/ha.sock]`
2. If it exists, but the VM is definitely stopped, it's a stale lock. Delete it:
   `[RUN_APPROVE: rm ~/.colima/<profile>/.lima/colima/ha.sock]`

## 2. Killing Zombie QEMU/VZ Processes
**Problem**: Colima fails to start because the hypervisor is already running, or memory is fully consumed by a hung VM process.
**Procedure**:
1. Find running Lima/QEMU processes:
   `[RUN_APPROVE: ps aux | grep -i lima]`
2. Identify the PID of the stuck `qemu-system-aarch64` or `vz` process.
3. Propose killing it safely:
   `[RUN_APPROVE: kill <PID>]`
4. If it refuses to die, force kill:
   `[RUN_APPROVE: kill -9 <PID>]`
5. After killing, suggest restarting the instance via the UI.
