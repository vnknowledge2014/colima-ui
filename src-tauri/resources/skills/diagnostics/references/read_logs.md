# Reading Logs: Procedural Guide

## 1. Colima Daemon Logs
**Problem**: Colima fails during start with a generic error, or `colima status` shows a crash.
**Procedure**:
Check the high-level Colima daemon logs.
- `[RUN_APPROVE: tail -n 50 ~/.colima/<profile>/colima.log]` (General startup errors)
- `[RUN_APPROVE: tail -n 50 ~/.colima/<profile>/daemon/daemon.log]` (Daemon lifecycle errors)

## 2. Lima VM Hypervisor Logs
**Problem**: The VM itself fails to boot before Colima even connects.
**Procedure**:
Check the low-level hypervisor (QEMU/VZ) logs.
- `[RUN_APPROVE: tail -n 50 ~/.colima/<profile>/.lima/colima/ha.stderr.log]`
- `[RUN_APPROVE: tail -n 50 ~/.colima/<profile>/.lima/colima/ha.stdout.log]`

## 3. macOS App Logs
**Problem**: The ColimaUI interface is bugging out or failing to emit events.
**Procedure**:
Tauri app logs on macOS are stored in the system logs or standard user Library paths, but it's usually easier to run the app from terminal or check the Web Inspector Console.
- Tell the user to right-click the UI, select "Inspect Element", and look at the Console tab.
