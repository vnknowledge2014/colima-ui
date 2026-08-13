---
phase: 1
title: "Decide terminal strategy"
status: pending
priority: P1
dependencies: []
effort: "S"
---

# Phase 1: Decide terminal strategy

## Overview

Lock the three open questions before any code moves, because each one removes or
adds a whole phase. This phase produces a decision, not an implementation.

## Requirements

- Functional: pick embedded-PTY (A), OS-handoff (B), or non-interactive (C).
- Non-functional: the decision must state which platforms and which transports
  are in scope, since that determines the security surface.

## Decisions

<!-- Updated: Validation Session 1 - all four closed -->

**Closed in Validation Session 1 (2026-08-11). This phase is complete.**

1. **Browser mode: out.** Terminal is Tauri-only, over IPC. WebSocket transport,
   upgrade auth, and `Origin` checks are all cut from the plan.
2. **Windows: out**, by implication — `colima` does not run there.
3. **Sessions per target: many.** Each tab owns a PTY, so a session cap and idle
   timeout become mandatory.
4. **Interactive shell: required.** `vim`/`k9s`/`htop` in scope → option A, real
   PTY. Also the precondition for the in-app K8s shell.

## Decisions to close (original)

1. **Browser mode in scope?** If Tauri-only, Phase 3 drops the WebSocket path and
   Phase 5 drops WS authentication — roughly half the remaining risk.
2. **Windows in scope?** `colima` is macOS/Linux. If Windows is out, ConPTY
   handling and BSD-vs-GNU divergence both stop mattering.
3. **Sessions per profile:** one, or many tabs?
4. **Is `k9s`/`vim` actually required?** If not, option C is far cheaper and this
   plan collapses to "stream logs properly".

## Related Code Files

- Read: `src-tauri/src/terminal_session.rs`
- Read: `src/pages/Terminal.svelte`, `src/pages/TerminalInstance.svelte`
- Read: `src-tauri/src/api_server.rs` (routes 238-242)

## Implementation Steps

1. Confirm the four decisions above with the user.
2. Record them in this file under a `## Decisions` heading.
3. Prune the later phases that the decisions make unnecessary.

## Success Criteria

- [ ] All four questions answered and written down.
- [ ] Phase list adjusted to match.

## Risk Assessment

Skipping this phase means building a WebSocket auth path and Windows ConPTY
support that may be dead code on day one.
