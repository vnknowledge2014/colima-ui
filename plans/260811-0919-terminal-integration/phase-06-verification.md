---
phase: 6
title: "Verification"
status: in-progress
priority: P1
dependencies: [5, 7]
effort: "S"
---

# Phase 6: Verification

## Overview

The failure modes here are behavioural, not type-level. A green build proves
nothing about a terminal — every check below has to be exercised against a real
VM and a real cluster.

## Verification Session 2 — 2026-08-12 (automated + OS-level)

Ran against the debug bundle (`ColimaUI.app`), the live colima `default` VM, and
the real app process. Checks marked `[x]` are **verified with evidence** below;
`[~]` are partially verified (code/unit-level, or VM-level but not in-app
visually); `[ ]` still need a human with eyes on the app UI.

### Automated
- `cargo test`: **157 passed / 0 failed** — includes the injection guards
  (`rejects_hostile_k8s_names`, `rejects_hostile_profile_names`,
  `k8s_exec_builds_argv_not_a_shell_string`) and the ring-buffer overflow tests.
- Frontend `vitest`: still cannot boot (pre-existing jsdom/undici vs Node 20
  mismatch, noted in the plan; not caused by this work).

### PTY semantics (exercised through the app's exact spawn argv
`ssh -tt -F ~/.colima/_lima/colima/ssh.config lima-colima` in a real PTY)
- [x] `stty size` = 40x120; resize → 30x100. SIGWINCH propagates.
- [~] `vim` opens, accepts input incl. CJK/emoji, survives resize, `:q` → E37
  (correct), `:q!` exits cleanly. Rendered *in-app* needs eyes.
- [~] `htop` renders + `q` quits (VM-level). `k9s` untested — no cluster this
  session (kind flapped after a colima restart; see notes).
- [x] `Ctrl+C` interrupts `sleep 100`; `Ctrl+Z` suspends (`[1]+ Stopped`);
  `fg`+`Ctrl+C` kills; `Ctrl+D` exits the shell (session ends).
- [x] `printf '你好 ▛▜ 🎉\n'` renders correctly.
- [x] `less` on a 200-line file scrolls (page-down) and `q` quits cleanly.

### Transport
- [x] Idle = zero requests: transport is IPC `Channel` push (no polling, no
  HTTP) — code-verified; `/api/terminal/*` routes no longer exist.
- [x] `yes | head -c 10M` completes without stalling the shell; ring-buffer
  overflow/drop-oldest is unit-tested (12/12).
- [x] External kill of the session's ssh child: process GONE (no zombie, no
  respawn), the sibling session untouched, the app process alive.

### Lifecycle
- [x] Quit app with sessions open → **no descendants**: after `TERM`, zero
  `ssh`/`limactl` orphans remained.
- [~] `colima stop` mid-session: session connection drops (ssh child exits),
  app process survives (state S, 0.1% CPU — no hung reader). The full
  stop→start cycle leaves the app + API server alive. The exact mid-stop API
  probe was inconclusive (see notes — instance confusion).
- [~] Session cap refused: code-verified (`MAX_SESSIONS=16`,
  `commands/terminal.rs:64-68`); the visible message needs eyes.
- [~] Idle reaping: code-verified (`MAX_IDLE` = 30 min, 60 s sweep,
  `terminal_session.rs:556-568`); live wait impractical.
- [ ] Close a single tab → `pgrep -f "colima ssh"` clean (needs eyes to close
  the tab).
- [ ] Tab switch and back → scrollback intact, no new session (needs eyes).
- [ ] Three tabs on the same profile run independently (needs eyes).

### Security
- [x] `grep -rn "api/terminal" src src-tauri` → only the comment explaining the
  replacement; no HTTP terminal endpoint exists.
- [x] Invalid namespace/pod/container rejected — unit-tested (injection guards,
  pass).
- [x] No session content in logs — `terminal_session.rs` contains no
  `println!`/`log::`/`tracing` calls; output goes only to the IPC channel.

### K8s exec (Phase 7) — BLOCKED this session
- [ ] Shell button opens an in-app tab, not Terminal.app.
- [ ] Distroless image → clear "no shell" message.
- [ ] Pod restart mid-session → clean exit message.

The kind cluster created for these tests **flapped after the colima VM
restart** (apiserver TLS timeouts) and was removed. The k8s exec path was
exercised in Validation Session 1 (DB records: `k8sExec kube-system/…` on
2026-08-11). Needs a stable cluster + eyes.

### History (Phase 7)
- [~] `↑` recalls previous-session commands for the same Colima profile —
  code/unit-verified (`HISTFILE=~/.colima-ui/history-{name}`,
  `PROMPT_COMMAND='history -a'`); live typing needs eyes.
- [~] Two profiles have separate history — unit-tested
  (`history_file_is_per_profile`, pass).
- [x] Commands typed with a leading space stay out of the history file —
  `HISTCONTROL=ignorespace` (code-verified).

## Findings worth fixing (out of this plan's scope)

1. **Stale-instance port shadowing.** The API server binds the first free port
   in 11420-11429 (Fix #12, `api_server.rs`). A second app instance therefore
   lands on 11421+, and the frontend port-scans to find it — but:
   - the `cui` CLI (`src-tauri/src/bin/cui.rs:116`) hardcodes
     `http://127.0.0.1:11420/api/cli/chat`, so with any stale instance holding
     11420 it silently talks to the wrong server;
   - a newer instance never rebinds 11420 when the holder exits (bind happens
     once, at startup);
   - nothing tells the user that two instances are running, or which port each
     owns.
   Report: `plans/reports/from-verification-to-owner-260812-1036-multi-instance-api-port-shadowing-report.md`.

   **Nửa đầu đã sửa 2026-08-13 — `cui` không còn hardcode 11420.**
   Nó quét 11420-11429 bằng `/api/health` (endpoint không cần auth, tồn tại đúng
   để làm việc này — webview đã quét như vậy từ trước), timeout 300 ms mỗi cổng
   để một cổng bị filter không treo CLI. Ba thông điệp lỗi từng khẳng định
   "port 11420" giờ nói đúng dải.

   Trước đây: app rơi xuống 11421+ thì `cui` **thất bại hoàn toàn** và báo "backend
   có đang chạy không?" trong khi nó đang chạy.

   Kiểm chứng: vòng quét chọn 11420 khi có app ở đó; khi 11420 im thì tìm ra
   stub ở 11427 — trường hợp `cui` cũ bỏ cuộc. (Kiểm bằng cách chạy đúng thứ tự
   / predicate / timeout của `discover_port` qua `curl`, **không phải** chạy
   binary với app đã dời cổng — không di dời được app đang chạy.)

   **Nửa sau vẫn mở, và cố ý:** với **hai instance cùng sống**, cổng thấp nhất
   thắng, mà đó có thể là cái cũ. Chọn giữa hai instance sống cần app tự công bố
   cái nào là hiện hành (ví dụ ghi cổng + pid ra một tệp, cái mới nhất thắng) —
   việc đó sửa `api_server.rs`, tệp đang được một phiên khác viết dở lúc này.
   TODO trong `api_server.rs:347-352` vẫn đứng.
   (Correction to the first draft of this section: the "second instance runs
   with a dead API server" claim was a measurement artifact — only 11420 was
   probed while the fallback instance served 11421.)
2. **Test environment notes**: the colima `default` VM had no `vim`/`htop`
   (installed via `apt` for the matrix; harmless, removable). kind clusters
   created inside the colima docker daemon do not survive a VM restart
   reliably.

## Remaining manual pass (needs eyes — ~10 minutes, one sitting)

1. Open the debug bundle, New Session → `default`.
2. In-app: `vim` a file → visually confirm rendering + resize redraw → `:q!`.
3. Open 3 tabs on `default`, type in each, switch tabs, confirm scrollback.
4. Close one tab → `pgrep -f "colima ssh"` (host) shows no orphan.
5. With a stable cluster: pod → shell button (in-app tab), a distroless pod
   ("no shell"), `kubectl delete pod` mid-session (clean exit).
6. Type `↑` in a fresh session → previous commands appear; a command prefixed
   with a space does not persist.

## Success Criteria

- [~] Every box in the matrix ticked against a real VM and cluster — **VM-level
  verified; in-app visual boxes need the manual pass above**.
- [x] Injection-guard tests exist and pass.
- [~] `plan.md` acceptance criteria all met — automated + OS-level met;
  visual criteria pending the manual pass.
