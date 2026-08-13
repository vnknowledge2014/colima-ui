---
title: 'Terminal integration: real PTY backend'
description: >-
  Replace the script(1)+HTTP-polling terminal with a real PTY over a streaming
  transport, and decide native-vs-embedded terminal strategy
status: in-progress
priority: P2
branch: dev
tags:
  - terminal
  - pty
  - tauri
  - xterm
blockedBy: []
blocks: []
created: '2026-08-11T02:22:00.343Z'
createdBy: 'ck:plan'
source: skill
---

# Terminal integration: real PTY backend

## Overview

The app already ships a terminal, so this is not a greenfield integration — it is
a correctness replacement. Today's implementation fakes a TTY with `script(1)`
and moves bytes over HTTP polling. That works for `ls` and breaks for anything
interactive.

## Current state (measured, not assumed)

| Layer | File | What it does today |
|-------|------|--------------------|
| Emulator | `src/pages/TerminalInstance.svelte` | xterm.js + fit + web-links addons |
| Transport | same, lines 93-115 | **one HTTP POST per keystroke**; `GET /api/terminal/read` polled every **100 ms** |
| Routes | `src-tauri/src/routes/ws.rs` | `create` / `write` / `read` / `close` / `resize` — despite the filename there is **no WebSocket** |
| Session | `src-tauri/src/terminal_session.rs` | spawns `script` wrapping `colima ssh` / `limactl shell`, pipes stdin/stdout into reader threads |
| Deps | `src-tauri/Cargo.toml` | **no** `portable-pty`, `termwiz`, or `alacritty_terminal` |

Two facts drive the whole plan:

1. **There is no PTY.** `script(1)` is a workaround to make the child believe it
   has a TTY. It gives echo, and nothing else: no `SIGWINCH`, no window size, no
   job control, no reliable signal delivery.
2. **`/api/terminal/resize` exists and is never called** by the frontend, and
   could not do anything useful if it were — there is no pty handle to resize.

## Decision: native, embedded, or neither

The user asked which kind of terminal to integrate. Three real options:

| Option | What it means | Cost | When it wins |
|--------|---------------|------|--------------|
| **A. Embedded + real PTY** (recommended) | `portable-pty` in Rust, xterm.js in the webview | Medium — 1 crate, 1 transport rewrite | You want `k9s`, `vim`, `top`, Ctrl+C to work in-app |
| **B. Hand off to the OS terminal** | `open -a Terminal "colima ssh"` / `wt.exe` | Tiny | You want zero security surface and zero maintenance; user leaves the app |
| **C. Keep it non-interactive** | Drop the shell, keep streaming logs + the existing `sandbox_execute` command runner | Tiny | Interactive shell is not actually a product requirement |

Do **not** reach for a server-side terminal emulator crate (`alacritty_terminal`,
`termwiz`). Those parse escape sequences into a grid — xterm.js already does
that in the webview. You would be paying twice.

This plan implements **A**, and keeps **B** as a one-line escape hatch ("Open in
Terminal.app") because it costs almost nothing and covers the cases the embedded
terminal gets wrong.

## Integration caveats — the checklist

### PTY semantics
1. `script(1)` flags differ between BSD (macOS) and GNU (Linux). The current
   call is not portable. A real PTY removes the problem entirely.
2. Size must be pushed on every change: `pty.resize(rows, cols)` → child gets
   `SIGWINCH`. Without it, `vim`/`htop` render into the wrong box.
3. Resize triggers: panel drag, window resize, font-size change, tab activation,
   sidebar collapse. Debounce, and never resize to 0×0 (some shells crash).
4. Signals must reach the PTY: `Ctrl+C`, `Ctrl+D`, `Ctrl+Z`, `Ctrl+\`. Verify the
   webview does not swallow them first.
5. Read **bytes, not strings**. UTF-8 sequences split across read boundaries —
   decoding each chunk independently corrupts CJK/emoji/box-drawing output.
6. Delete the `\r?\n` → `\r\n` normalization (`TerminalInstance.svelte:111`). It
   is compensating for `script`, and it mangles cursor-addressed TUI output.

### Transport
7. Polling every 100 ms costs latency and CPU while idle, and drops interleaved
   output under burst. One HTTP request per keypress is worse.
8. Tauri IPC only (`Channel<Vec<u8>>` or events). No port, no token round-trip,
   no CORS, no upgrade handshake to authenticate.
9. ~~Browser path: WebSocket~~ — **cut by validation.** The terminal is desktop
   only, so the HTTP terminal routes are deleted rather than upgraded. This
   removes the plan's single largest risk.
10. Apply backpressure: bound the output ring buffer, drop oldest, and cap
    xterm's `scrollback`. An unbounded buffer plus `yes` is an OOM.

### Lifecycle
11. Session id is `${sessionId}-${Date.now()}` (`TerminalInstance.svelte:39`), so
    every remount is a new session. If `close` fails, the `script`/ssh child is
    orphaned. Reap on: close, idle timeout, app exit, VM stop.
12. Kill the whole process group, not just the direct child — ssh spawns
    descendants.
13. Handle the VM disappearing under a live session (`colima stop`) without
    hanging the reader thread.

### Security — this endpoint is remote code execution
14. Bind to `127.0.0.1` only, and require the existing bearer token **on the
    WebSocket upgrade too**, not just on the REST routes.
15. Validate `profile` / `vm_type` against a strict allowlist before they reach
    a command line. They are currently interpolated into a spawn.
16. Never log terminal I/O — users type credentials into shells.
17. Update `src-tauri/capabilities/default.json` explicitly for whatever new IPC
    surface is added.

### UX
18. Preserve scrollback across tab switches (the current `display:none` approach
    already does — keep it, do not unmount).
19. Copy/paste conventions differ: `Cmd+C/V` on macOS, `Ctrl+Shift+C/V`
    elsewhere. Enable bracketed paste; chunk large pastes.
20. Drive the xterm theme from the app's CSS tokens so it follows the header /
    panel work already done.
21. `FitAddon` + `ResizeObserver` can loop. Guard, as the AI panel does.

## Phases

| Phase | Name | Status |
|-------|------|--------|
| 1 | [Decide terminal strategy](./phase-01-decide-terminal-strategy.md) | Completed |
| 2 | [PTY backend](./phase-02-pty-backend.md) | Completed |
| 3 | [Transport and streaming](./phase-03-transport-and-streaming.md) | Completed |
| 4 | [Frontend xterm rewrite](./phase-04-frontend-xterm-rewrite.md) | Completed |
| 5 | [Security and lifecycle](./phase-05-security-and-lifecycle.md) | Completed |
| 6 | [Verification](./phase-06-verification.md) | In Progress — **auto + OS-level verified (Session 2, 2026-08-12)**; in-app visual pass needs eyes (see phase-06) |
| 7 | [K8s exec sessions and shell history](./phase-07-k8s-exec-and-shell-history.md) | Completed |

**Execution order:** 1 → 2 → 3 → 4 → {5, 7 in parallel} → 6. Phase 7 was added
after scaffolding, so its number trails its position — it runs before
verification, not after. Phase 1 is closed by Validation Session 1 below.

## Two follow-on requirements

**Container shell must open in-app.** `src-tauri/src/routes/k8s.rs:509` currently
builds a `kubectl exec -it` string and hands it to `osascript` → Terminal.app.
The shell button should instead open a Terminal tab attached to that container.
This is blocked on Phase 2: `kubectl exec -it` needs a real TTY, so without the
PTY it would reproduce the exact bug this plan exists to fix. See Phase 7 Part A.

**Per-session shell history.** Worth splitting into three things, because only
two are buildable. The shell's own `↑` history works if each target gets a stable
`HISTFILE` — but not for exec, where container filesystems are ephemeral and
often shell-less. A record of *which sessions were opened* is easy and useful. A
searchable list of *commands run* is not buildable from a raw PTY without
reimplementing line editing, and should not be attempted. See Phase 7 Part B.

## Acceptance criteria

- [ ] `vim`, `htop`, and `k9s` are usable in-app, including resize.
- [ ] `Ctrl+C` interrupts a running command instead of printing `^C`.
- [ ] Idle terminal costs no periodic network or CPU.
- [ ] Closing a tab or quitting the app leaves no orphaned `ssh` / `script` process.
- [ ] No terminal route is reachable over HTTP at all.
- [ ] Container shell button opens an in-app tab, not Terminal.app.
- [ ] `↑` recalls history within a profile; profiles do not share history.
- [ ] Several tabs on one target run independently; the session cap holds.
- [ ] "Open in Terminal.app" survives as an explicit secondary action.

## Validation Log

### Session 1 — 2026-08-11

**Verification pass (tier: Full, 7 phases).** 13 claims checked — 12 verified,
1 failed.

| Claim | Result |
|-------|--------|
| `TerminalInstance.svelte:39` mints session id with `Date.now()` | VERIFIED |
| `TerminalInstance.svelte:93-115` POST-per-keystroke + 100 ms poll | VERIFIED |
| `TerminalInstance.svelte:111` normalizes `\r?\n` | VERIFIED |
| `routes/ws.rs` contains no WebSocket (0 matches) | VERIFIED |
| `api_server.rs:238-242` registers 5 terminal routes | VERIFIED |
| `/api/terminal/resize` registered (1) but never called by frontend (0) | VERIFIED |
| `terminal_session.rs:75` spawns `Command::new("script")` | VERIFIED |
| `Cargo.toml` has no `portable-pty` (0 matches) | VERIFIED |
| `routes/k8s.rs:509` `api_k8s_exec` shells out to `osascript` | VERIFIED |
| `Kubernetes.svelte:310` calls `k8sApi.exec` + toast | VERIFIED |
| `store.svelte.ts` deep-link convention exists | VERIFIED |
| xterm + fit + web-links deps present | VERIFIED |
| `is_valid_k8s_name` defined in `routes/k8s.rs` | **FAILED** — it lives in `src-tauri/src/validation.rs:85`, alongside `is_valid_profile_name:120`, `is_valid_resource_name:133`, `is_valid_container_id:24`. `routes/k8s.rs` only calls it. Corrected in Phase 2 and Phase 5. |

**Decisions confirmed:**

1. **Terminal is Tauri-only.** No browser-mode terminal. WebSocket transport,
   upgrade authentication, and `Origin` checking are all cut; the HTTP terminal
   routes are deleted rather than ported. Windows is out of scope by
   implication — `colima` does not run there — so ConPTY divergence is moot.
2. **Real PTY is required.** `vim`/`k9s`/`htop` are in scope, which is also the
   precondition for the in-app K8s shell (`kubectl exec -it` needs a TTY).
   Options B and C are not taken; B survives only as a secondary action.
3. **Multiple concurrent sessions per target.** Each tab owns its own PTY. This
   makes a session cap and an idle timeout mandatory rather than optional.
4. **Per-profile `HISTFILE` written inside the VM**, at
   `~/.colima-ui/history-<profile>`, with `HISTCONTROL=ignorespace`. Exec
   sessions still get no persistent history.

**Net effect:** Phase 3 loses the WebSocket path, Phase 5 loses its two highest
-risk items and gains session-cap work, Phase 6 loses the WS security matrix.

### Whole-Plan Consistency Sweep

Re-read `plan.md` and all seven phase files after propagation. Two stale claims
found and fixed:

- `phase-02:36` still said the `SessionManager` shape keeps "`routes/ws.rs` and
  both transports" source-compatible — but `routes/ws.rs` is now deleted in
  Phase 3 and there is only one transport.
- `phase-04:27` still described `terminal-transport.ts` as "one module, both
  transports".

Re-swept after fixing: no remaining references to a second transport, a browser
path, or WebSocket authentication outside explicitly-cut context. The corrected
validator location (`validation.rs`, not `routes/k8s.rs`) appears consistently in
Phases 2 and 5. Phase 1 is marked complete.

**Unresolved contradictions: none.**

A second-order consequence surfaced during the sweep and was folded into Phase 7:
multiple tabs per profile now share one `HISTFILE`, and the shell's default
append-on-exit loses history from every tab but the last one closed.

### Session 2 — 2026-08-11 (implementation audit)

Code-level audit of all seven phases. Phases 2, 3, 4, 5, 7 verified complete
against the source; Phase 6 is the only one still open.

| Phase | Evidence |
|-------|----------|
| 2 | `portable-pty` in `Cargo.toml`; zero `Command::new("script")` remaining; `resize` → `PtySize`. Process-group kill deliberately skipped — `terminal_session.rs:460` documents that ssh's own hangup reaps descendants. |
| 3 | `routes/ws.rs` deleted; zero `api/terminal` matches in `api_server.rs`; Tauri `Channel<>` in `commands/terminal.rs`. |
| 4 | `terminal-transport.ts` exists; zero 100 ms polling; `\r?\n` normalization gone. Remaining `Date.now()` (`Terminal.svelte:163`) mints the **tab** id, not the session id — correct. |
| 5 | `MAX_SESSIONS = 16` (`commands/terminal.rs:18`, enforced :64); `MAX_IDLE = 30 min` + 60 s sweep; `ensure_valid_profile` / `is_valid_k8s_name` guard argv; `MAX_BUFFER_BYTES = 1 MiB` with drop-oldest trim. |
| 7 | `Kubernetes.svelte:318` calls `openTerminalSession` (in-app tab); `k8sApi.exec` → osascript survives only as the secondary action. `history_preamble` sets per-profile `HISTFILE` + `PROMPT_COMMAND='history -a'`, which closes the multi-tab append loss noted in Session 1. |
| 6 | **Automated portion done**: `cargo test terminal_session` → 12 passed, 0 failed (injection guards, UTF-8 split-read carry, zero-size resize refusal, per-profile history). **Manual matrix untouched** — needs a real VM and cluster. |

`svelte-check` shows 144 pre-existing errors across 36 unrelated files; no
terminal file is among them.

## Open questions

None. All three prior questions were closed in Validation Session 1.
