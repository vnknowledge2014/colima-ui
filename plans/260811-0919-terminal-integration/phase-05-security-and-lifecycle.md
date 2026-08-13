---
phase: 5
title: Security and lifecycle
status: in-progress
priority: P1
dependencies:
  - 3
effort: M
---

# Phase 5: Security and lifecycle

## Overview

A terminal endpoint is arbitrary remote code execution on the user's machine.
Treat it as the most sensitive surface in the app, and make sure no process
outlives the UI that owns it.

## Requirements

- Functional: no unauthenticated path to a shell; no orphaned processes.
- Non-functional: no terminal I/O in logs.

## Threat model

What this actually exposes: a shell inside the user's Colima VM, and — once
Phase 7 lands — inside cluster containers. The attacker of interest is any other
local process or page that can reach `127.0.0.1:11420`, not a remote network
attacker (the server binds loopback).

## Implementation Steps

<!-- Updated: Validation Session 1 - steps 1-3 deleted with browser mode; session cap added -->

1. ~~Authenticate the WebSocket at upgrade time~~ — **cut.** Validation made the
   terminal Tauri-only; Phase 3 deletes the HTTP routes outright. There is no
   network-reachable shell endpoint left to authenticate.
2. ~~Keep the bind on `127.0.0.1`~~ — still true of the rest of the API server,
   but no longer this phase's concern.
3. ~~Reject cross-origin upgrades~~ — **cut** with the WebSocket.
4. Allowlist `profile` / `vm_type` / namespace / pod / container before they
   reach a command line. The validators already exist in
   `src-tauri/src/validation.rs` — `is_valid_profile_name` (line 120),
   `is_valid_k8s_name` (line 85), `is_valid_resource_name` (line 133),
   `is_valid_container_id` (line 24). `routes/k8s.rs` calls them; it does not
   define them. Reuse, do not re-implement.
   <!-- Updated: Validation Session 1 - corrected validator location -->

5. Drop the `osascript` path from `k8s_exec` once Phase 7 lands, or keep it
   behind an explicit "open externally" action. AppleScript string escaping is a
   permanent injection risk that disappears with the PTY.
6. Never log stdin/stdout. Users type passwords and tokens into shells.
7. Reap on every exit path: tab close, app quit, VM stop, idle timeout,
   transport disconnect. Kill the process **group**.
8. Add an idle timeout so a forgotten tab does not hold a VM session forever.
9. **Cap concurrent sessions.** Validation allows many tabs per target, so
   nothing bounds PTY count by construction any more — each tab is a real shell
   process inside the VM. Enforce a ceiling and refuse politely past it.
10. Update `src-tauri/capabilities/default.json` for the new IPC surface.

## Success Criteria

- [ ] `grep -rn "api/terminal" src src-tauri` returns nothing — no HTTP shell.
- [ ] `pgrep -f "colima ssh"` is clean after closing all tabs.
- [ ] Quitting the app leaves no descendants.
- [ ] Opening tabs past the cap is refused with a clear message, not a silent
      spawn or a crash.
- [ ] An idle session is reaped on schedule.
- [ ] `grep -ri "stdout" src-tauri/src/terminal_session.rs` shows no logging of
      session content.

## Risk Assessment

<!-- Updated: Validation Session 1 - top risk removed, new one introduced -->

The original top risk — authenticating a streaming shell endpoint — is gone with
browser mode.

The risk that replaced it is quieter: multi-tab sessions mean N shells inside the
VM, each holding an ssh connection. Without the cap and the idle timeout in
steps 8-9, a user who opens tabs all day silently exhausts the VM. This is now
the most likely way this feature misbehaves in real use.
