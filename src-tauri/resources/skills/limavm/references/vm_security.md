# Lima VM Security & Isolation: Procedural Guide

## 1. Process Isolation
**Problem**: The user is concerned about a container breaking out and compromising their Mac.
**Procedure**:
Lima isolates the VM from the macOS host using virtualization (QEMU or VZ). 
- The VM itself runs as a normal, unprivileged user process on macOS.
- Even if a container breaks out into the VM, they only have access to the Linux guest OS, not the macOS host OS.

## 2. File Mounting Security
**Problem**: The user doesn't want the VM to have write access to their entire home directory.
**Procedure**:
1. Open the built-in Colima Config (`colima.yaml`) editor via `[NAVIGATE: settings]` to inspect the `mounts` section (ColimaUI automatically resolves `$COLIMA_HOME`, Colima `0.10.x` XDG `~/.config/colima/default/colima.yaml`, or legacy `~/.colima/default/colima.yaml` — never pass `~` paths to `cat` in the shell sandbox).
2. By default, Colima mounts the user's home directory (`~`) as writable so developers can easily share source code.
3. To tighten isolation, change `writable: false` in the `mounts` section, or mount only specific project subdirectories instead of the entire home directory.
4. Restart the instance after saving: `[EVENT_APPROVE: colima-restart | {"profile": "default"}]`.
