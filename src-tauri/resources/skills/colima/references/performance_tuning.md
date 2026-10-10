# Colima Performance Tuning: Procedural Guide

## 1. High CPU Usage on Idle
**Problem**: The `qemu-system-aarch64` or `vz` process consumes 100% CPU on macOS even when no containers are active.
**Procedure**:
1. Check if Kubernetes is enabled (`colima status --profile <name>`). K3s creates constant background CPU load. If the user doesn't need K8s, suggest they disable it.
2. Check if the VM is running out of memory. If swapping heavily, CPU usage spikes. Increase RAM in the Preset.

## 2. Choosing VM Types (Apple Silicon)
**Problem**: The user wants the absolute best performance on an M1/M2/M3 Mac.
**Procedure**:
1. Ensure the Preset uses `vmType: vz` (Apple's Virtualization framework) rather than `qemu`.
2. Ensure `mountType: virtiofs` is used for fastest file sharing.
3. Note: `vz` requires macOS 13+. If the user is on an older OS, they must fall back to `qemu`.

## 3. Storage Optimization
**Problem**: The VM disk is full or fragmented.
**Procedure**:
Colima creates a sparse disk image. It grows automatically but doesn't shrink automatically.
To reclaim space, the easiest procedural fix is to delete and recreate the instance (warn the user this deletes all data).
