---
phase: 2
title: PTY backend
status: in-progress
priority: P1
dependencies:
  - 1
effort: M
---

# Phase 2: PTY backend

## Overview

Replace the `script(1)` wrapper with a real pseudo-terminal, so the child process
gets a genuine TTY: window size, signals, and job control.

## Requirements

- Functional: spawn `colima ssh -p <profile>` / `limactl shell <name>` on a PTY;
  read/write bytes; resize.
- Non-functional: no per-platform shell hacks; process group reaped on drop.

## Architecture

`portable-pty` (used by WezTerm; covers Unix openpty and Windows ConPTY) replaces
`Command::new("script")`.

```
PtySystem::openpty(PtySize { rows, cols, .. })
  -> pair.slave.spawn_command(cmd)        // child
  -> pair.master.try_clone_reader()       // -> reader thread -> ring buffer
  -> pair.master.take_writer()            // <- stdin
  -> pair.master.resize(PtySize { .. })   // SIGWINCH
```

Keep the existing `SessionManager` shape and swap only the internals, so Phase 3
can put the Tauri commands on top of it without a second rewrite.
<!-- Updated: Validation Session 1 - single transport; routes/ws.rs is deleted in Phase 3 -->

## Related Code Files

- Modify: `src-tauri/Cargo.toml` (add `portable-pty`)
- Rewrite: `src-tauri/src/terminal_session.rs`
- Touch: `src-tauri/src/routes/ws.rs` (wire the real `resize`)

## Implementation Steps

1. Add `portable-pty` to `Cargo.toml`.
2. Rewrite `SessionManager::create` to open a PTY and spawn onto the slave.
3. Store `master`, `writer`, `child`, and the output ring buffer per session.
4. Implement `resize(session_id, rows, cols)` against `master.resize`.
5. Make the output buffer hold **bytes**, not `String` — no lossy UTF-8 decode at
   this layer.
6. Bound the buffer; drop oldest on overflow.
7. `Drop` / `close` kills the process **group** and joins the reader thread.
8. Validate `profile` and `vm_type` before building the command. Do not write a
   new validator — `src-tauri/src/validation.rs` already exposes
   `is_valid_profile_name`, `is_valid_k8s_name`, `is_valid_resource_name`, and
   `is_valid_container_id`. Use `is_valid_profile_name` here.
   <!-- Updated: Validation Session 1 - validators live in validation.rs, not routes/k8s.rs -->


## Success Criteria

<!-- Updated: implementation session - runtime criteria blocked, see below -->

- [x] `printf '你好'` renders correctly when split across reads — covered by
      `truncated_character_is_carried_not_mangled` and
      `invalid_byte_does_not_disable_carry_for_later_truncation`.
- [x] Hostile profile names and unknown `vm_type` rejected before argv.
- [ ] `stty size` inside the session reports the real grid. **BLOCKED**
- [ ] `vim` opens, redraws on resize, and exits cleanly. **BLOCKED**
- [ ] `Ctrl+C` interrupts `sleep 100`. **BLOCKED**
- [ ] Closing a session leaves no `ssh` descendant. **BLOCKED**

### Verification status

`colima` is not installed on the development machine and `~/.colima/_lima/`
holds no `ssh.config`, so no session can be opened here. Every criterion that
needs a live VM is unverified — the code compiles and the pure logic is unit
tested, which is *not* the same as knowing the pty works.

These four must be exercised on a machine with a running Colima instance before
Phase 2 is called done. Until then, treat "PTY backend complete" as "written and
type-checked", not "working".

## Risk Assessment

- `portable-pty` pulls a moderate dependency tree — acceptable; it is the
  de-facto crate and avoids hand-rolling `openpty`/ConPTY.
- Reader thread can block on a dead VM: use a non-blocking read or a read
  timeout so `close` cannot hang.
