---
phase: 4
title: Frontend xterm rewrite
status: in-progress
priority: P1
dependencies:
  - 3
effort: M
---

# Phase 4: Frontend xterm rewrite

## Overview

Rewire `TerminalInstance.svelte` onto the push transport and fix the byte-level
bugs that the polling design forced.

## Requirements

- Functional: attach xterm to the stream, propagate resize, preserve scrollback
  across tab switches.
- Non-functional: no per-keystroke network call; theme follows app tokens.

## Related Code Files

- Rewrite: `src/pages/TerminalInstance.svelte`
- Modify: `src/pages/Terminal.svelte` (session lifecycle, tab switching)
- Maybe add: `src/lib/terminal-transport.ts` (thin wrapper over the Tauri IPC
  commands, so the component does not call `invoke` inline)
  <!-- Updated: Validation Session 1 - single transport -->

## Implementation Steps

1. Replace the `onData` → `fetch(POST /write)` handler with a single open stream
   write.
2. Delete the 100 ms `setInterval` read loop.
3. **Delete `data.replace(/\r?\n/g, "\r\n")`** (line 111) — it exists to paper
   over `script(1)` and corrupts cursor-addressed output.
4. Feed xterm `Uint8Array` rather than `string`, so multibyte sequences split
   across frames survive.
5. Send `resize` on: `FitAddon.fit()`, tab activation, window resize, AI-panel
   drag, font-size change. Debounce ~50 ms; never send 0 rows/cols.
6. Stop minting a new session id per mount (`${sessionId}-${Date.now()}`) — key
   the session to the tab, so a remount reattaches instead of orphaning.
7. Drive `theme` from the CSS custom properties rather than a literal object.
8. Set a bounded `scrollback`.
9. Add the "Open in Terminal.app" escape hatch button (option B from `plan.md`).

## Success Criteria

- [ ] Typing produces no network request per keypress.
- [ ] Resizing the window reflows `htop` correctly.
- [ ] Switching tabs and back preserves scrollback and does not spawn a session.
- [ ] Box-drawing characters and CJK render without corruption.
- [ ] Terminal colours track a theme change without a reload.

## Risk Assessment

`FitAddon` inside a `ResizeObserver` can feed itself. Guard the same way the AI
composer does — measure, compare, only then act.
