# Reading Logs: Procedural Guide

> **Path & Sandbox Note**: Colima `0.10.x` defaults to XDG `~/.config/colima` while older installs use `~/.colima` (or `$COLIMA_HOME`). Lima hypervisor logs live under `<colima-home>/_lima/colima/` (for `default`) or `<colima-home>/_lima/colima-<profile>/` (or `$LIMA_HOME`). Because the shell sandbox executes binaries directly without `~` tilde expansion, either run `[DIAGNOSE]` (which collects Colima & Lima logs automatically) or run `[QUERY: environment-resolutions]` first to get the exact `/Users/<user>/...` absolute path.

## 1. Colima Daemon Logs
**Problem**: Colima fails during start with a generic error, or `colima status` shows a crash.
**Procedure**:
Resolve `<colima-home>` via `[QUERY: environment-resolutions]`, then read the concrete absolute path (or run `[DIAGNOSE]`):
- `[RUN: tail -n 50 /Users/<user>/.config/colima/<profile>/daemon/daemon.log]` *(or `/Users/<user>/.colima/<profile>/daemon/daemon.log` on legacy layouts)*

## 2. Lima VM Hypervisor Logs
**Problem**: The VM itself fails to boot before Colima even connects.
**Procedure**:
Check the low-level hypervisor (VZ/QEMU) logs under `_lima/colima` (for `default`) or `_lima/colima-<profile>`:
- `[RUN: tail -n 50 /Users/<user>/.config/colima/_lima/colima/ha.stderr.log]`
- `[RUN: tail -n 50 /Users/<user>/.config/colima/_lima/colima/ha.stdout.log]`

## 3. macOS App Logs
**Problem**: The ColimaUI interface is bugging out or failing to emit events.
**Procedure**:
Tauri app logs and redacted crash reports on macOS can be exported via Settings, `~/.colima-ui/crashes`, or inspected in the Web Inspector Console:
- Tell the user to right-click the UI, select "Inspect Element", and look at the Console tab.
