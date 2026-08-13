# Multi-instance API port shadowing — bug report

- **From:** Terminal Integration Phase 6 verification (Session 2, 2026-08-12)
- **To:** Owner
- **Severity:** Low — silent misdirection, no data loss
- **Status:** Open (TODO marked in code, see below)

## Symptom

While verifying the terminal lifecycle matrix, probing
`http://127.0.0.1:11420/api/instances` after a colima stop returned `HTTP 000`
even though the app process was alive — initially diagnosed as "the API server
died". That diagnosis was **wrong** (see Correction).

The real behavior: with **two app instances running**, the second instance does
not fail — it binds the first free port in `11420-11429` (Fix #12,
`src-tauri/src/api_server.rs:285-309`). `11420` was held by a stale instance
from before this session; the probed instance served `11421`, which was never
queried.

## Root cause chain

1. `api_server.rs` binds the first free port in `11420-11429` once, at startup,
   and never re-scans afterwards.
2. The **frontend** port-scans `11420-11429` (`src/lib/api/client.ts:20`), so
   the webview always reaches its own instance's server — the bug is invisible
   in normal app use.
3. The **`cui` CLI** (`src-tauri/src/bin/cui.rs:116`) hardcodes
   `http://127.0.0.1:11420/api/cli/chat`. With any stale instance holding
   11420, the CLI silently talks to the *wrong* server (the old one), and there
   is no user-facing notice about which instance owns which port.

## Impact

- External tools hardcoding 11420 (the `cui` CLI, health-check scripts) can
  query a stale instance while the active one runs on 11421+.
- A newer instance never reclaims 11420 when the holder exits; the mismatch
  persists until the newer instance restarts.
- Nothing warns the user that two instances are running.

## Evidence

- `lsof -iTCP:11420` → old instance PID held the port while a newer instance
  was alive.
- Unified log: old instance exited `09:54:25` (before the colima stop at
  `09:59`), freeing 11420; the newer instance's API was never probed on 11421.
- Code: port-scan fallback at `api_server.rs:285-309`; hardcoded CLI port at
  `cui.rs:116`; frontend scan at `client.ts:20`.

## Suggested fix (whenever touched)

- When `bound_port != 11420`, surface a notice (log + UI) that another instance
  holds the primary port.
- Have the `cui` CLI resolve the active server the same way the frontend does
  (scan 11420-11429), or expose the bound port via IPC/health endpoint.
- Optionally re-scan for 11420 when the holder exits (the holder is an app
  process, not arbitrary software, so re-binding is low-risk).

## Correction note

The first draft of this finding (in
`plans/260811-0919-terminal-integration/phase-06-verification.md`) claimed the
second instance "silently runs without an API server". That was a measurement
artifact: only 11420 was probed while the fallback instance served 11421. The
claim is retracted; this report is the corrected record.

## Location

- TODO marker: `src-tauri/src/api_server.rs` (bind site, `// TODO(port-shadowing)`)
- Phase 6 findings section updated accordingly.
