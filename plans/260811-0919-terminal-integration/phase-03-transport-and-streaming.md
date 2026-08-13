---
phase: 3
title: Transport and streaming
status: in-progress
priority: P1
dependencies:
  - 2
effort: M
---

# Phase 3: Transport and streaming

## Overview

Replace request-per-keystroke plus 100 ms polling with a push transport, so
latency is one hop and an idle terminal costs nothing.

## Requirements

- Functional: bidirectional byte stream per session; resize messages.
- Non-functional: idle session issues zero periodic requests; burst output does
  not drop or reorder.

## Architecture

<!-- Updated: Validation Session 1 - Tauri-only, WebSocket path cut -->

**One transport: Tauri IPC.** Validation closed browser mode as out of scope, so
the WebSocket upgrade, its authentication, and its `Origin` check are all cut.

- IPC `Channel<Vec<u8>>` from Rust to the webview for output; a `terminal_write`
  command inbound. No port, no token round-trip, no CORS, no handshake.
- The five HTTP terminal routes in `api_server.rs:238-242` are **deleted**, not
  ported. A shell endpoint that no longer exists cannot be authenticated wrong.

Command surface:

```
terminal_create(kind) -> session_id            # Tauri command
terminal_write(session_id, bytes)              # Tauri command
terminal_resize(session_id, rows, cols)        # Tauri command
terminal_close(session_id)                     # Tauri command
Channel<Vec<u8>>                               # Rust -> webview, output
   + { "type": "exit", "code": n } on death
```

## Related Code Files

- Delete: `src-tauri/src/routes/ws.rs` (nothing left in it once the routes go)
- Modify: `src-tauri/src/api_server.rs` (drop routes 238-242)
- Modify: `src-tauri/src/lib.rs` (register the four commands + channel)
- Modify: `src-tauri/capabilities/default.json`

## Implementation Steps

1. Add the four Tauri commands, backed by the Phase 2 `SessionManager`.
2. Bridge the session's reader thread into an IPC `Channel`.
3. Emit an `exit` message when the child dies; do not leave the UI thinking it
   is connected.
4. Apply backpressure: if the consumer is slow, drop from the oldest end of the
   ring buffer rather than growing it.
5. Delete `/api/terminal/*` (all five routes) and the frontend polling loop in
   **one** commit, so no intermediate state ships both an authenticated-nothing
   HTTP shell and the IPC one.
6. Grep for stragglers afterwards: `grep -rn "api/terminal" src src-tauri` must
   come back empty.

## Success Criteria

- [ ] Idle terminal: zero requests/second (verify in devtools network panel).
- [ ] `yes | head -c 10M` does not stall the UI or exhaust memory.
- [ ] Keystroke-to-echo latency visibly lower than the polling build.
- [ ] Killing the shell surfaces an `exit` message in the UI.
- [ ] `grep -rn "api/terminal" src src-tauri` returns nothing.

## Risk Assessment

<!-- Updated: Validation Session 1 - both original risks eliminated -->

Both risks originally listed here are gone. Validation cut browser mode, so
there is no second transport and no WebSocket authentication to get wrong.

What remains: the delete-and-replace must land atomically. Shipping the IPC path
while the unauthenticated HTTP shell routes still exist would be strictly worse
than today.
