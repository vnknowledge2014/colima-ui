---
phase: 7
title: K8s exec sessions and shell history
status: in-progress
priority: P2
dependencies:
  - 4
effort: M
---

# Phase 7: K8s exec sessions and shell history

## Overview

Two follow-on requirements: clicking a container's shell button should open a
session in the app's own Terminal tab rather than launching Terminal.app, and
sessions should carry usable history.

## Part A — K8s exec as an in-app session

### Current behaviour

`src-tauri/src/routes/k8s.rs:509` (`api_k8s_exec`) builds
`kubectl exec -it -n <ns> <pod> -c <container> -- /bin/sh` and hands it to
`osascript` → `tell application "Terminal" to do script`. The user lands in
Terminal.app, outside the app. `src/pages/Kubernetes.svelte:310` then toasts
"Shell opened in Terminal", which is accurate today and should stop being true.

### Target behaviour

The shell button opens a Terminal tab in-app, already attached to the container.

### Design

Generalise the session, rather than adding a parallel code path:

```
SessionKind::Colima { profile }
SessionKind::Lima   { instance }
SessionKind::K8sExec { namespace, pod, container, shell }
```

All three spawn onto the same PTY from Phase 2; only the argv differs. Everything
downstream — transport, resize, reaping, history — is shared.

### Implementation Steps

1. Add `SessionKind` to `terminal_session.rs`; `create` takes it instead of
   `(profile, vm_type)`.
2. Build the K8s argv as a **vector**, never a shell string — no quoting, no
   AppleScript escaping, no injection surface.
3. Add a store action (`openTerminalSession(kind)`) that creates the tab and
   navigates: `uiState.currentPage = "terminal"`. Follow the existing one-shot
   deep-link convention already used by `openHelpArticle` / `openSettingsSection`
   in `src/store.svelte.ts` rather than inventing a third mechanism.
4. Point `Kubernetes.svelte`'s shell button at that action; drop the
   `k8sApi.exec` call and its toast.
5. Fall back gracefully: try `/bin/bash`, then `/bin/sh`. Many images have
   neither — surface that as a clear message, not a blank terminal.
6. Keep "Open in Terminal.app" as an explicit secondary action if the user wants
   it; do not make it the default.
7. Label the tab `pod/container`, not a random session id.

### Caveats specific to exec

- `kubectl exec -it` needs a TTY on **both** ends; without Phase 2 this cannot work.
- Distroless / scratch images have no shell at all. Detect and say so.
- The session dies when the pod restarts — surface the exit, do not hang.
- Namespace/pod/container must pass `is_valid_k8s_name` before use.

## Part B — Shell history

### The honest constraint

With a raw PTY the app sees **keystrokes**, not commands. Reconstructing "what
command did the user run" means reimplementing line editing — arrow keys, `Ctrl+R`,
multi-line continuations, vi-mode. That is unreliable and should not be attempted.

So there are three separable features people mean by "history":

| What | How | Viability |
|------|-----|-----------|
| **1. Shell's own history** (`↑` inside the session) | Give the shell a stable `HISTFILE` | Works for Colima/Lima; usually impossible for exec |
| **2. Session history** (which shells were opened, when) | App-level record | Easy, genuinely useful |
| **3. Command history UI** (searchable list of commands run) | Requires parsing keystrokes | Do not build |

### Implementation Steps

<!-- Updated: Validation Session 1 - HISTFILE-in-VM confirmed; option 1 and 2 in, option 3 out -->

Validation confirmed writing a history file **inside the VM** is acceptable, so
rows 1 and 2 of the table above are both in scope and row 3 stays out.

1. **Per-target `HISTFILE`.** For Colima/Lima, export a stable
   `HISTFILE=~/.colima-ui/history-<profile>` when the shell starts, so `↑` works
   across sessions and profiles do not share a history. Note that multiple tabs
   on one profile now share that file — append-on-exit is the shell default and
   loses history from all but the last tab to close, so set
   `shopt -s histappend` and `PROMPT_COMMAND='history -a'` for bash.
2. Set `HISTCONTROL=ignorespace` so a leading space keeps secrets out of the file.
3. **Do not** try to persist history for `K8sExec`: container filesystems are
   ephemeral, frequently read-only, and often run `sh` with no history support.
   Say this in the UI instead of silently doing nothing.
4. **Session history** in the existing SQLite store (`knowledge.db`), mirroring
   the `chat_conversations` pattern: kind, target, started_at, ended_at, exit
   code. Gives a "reopen last session" affordance.
5. Never store session *content*, only metadata. Same reasoning as the
   no-logging rule in Phase 5.

## Related Code Files

- Modify: `src-tauri/src/terminal_session.rs` (SessionKind)
- Modify: `src-tauri/src/routes/k8s.rs` (retire the `osascript` default)
- Modify: `src/store.svelte.ts` (deep-link action)
- Modify: `src/pages/Kubernetes.svelte`, `src/pages/Terminal.svelte`
- Modify: `src-tauri/src/commands/knowledge_bank.rs` (session history table)

## Success Criteria

<!-- Updated: first real run against a live cluster -->

- [x] Container shell button opens an in-app tab attached to that container.
- [x] No `osascript` runs on the default path.
- [x] A distroless pod produces a clear no-shell message.

### What the first live run proved

Running the shell button against `coredns-75d478d48f-6c4v2` returned:

```
OCI runtime exec failed: exec failed: unable to start container process:
exec: "sh": executable file not found in $PATH
command terminated with exit code 127
● Session ended (exit code: 127)
```

That output is itself the evidence that the chain works end to end: the tab
opened in-app, `kubectl exec -it` got a real TTY, container stderr streamed
through the pty to xterm, and the exit code surfaced instead of the UI hanging
on a dead prompt. Only the last step was missing — saying what 127 means.

The shell fallback was also wrong in a way worth recording: it was written as
`sh -c 'command -v bash … || exec sh'`, so the chain could only run on an image
that already had `sh`. A `FROM scratch` image has none, so the interpreter
chosen to evaluate the fallback was the missing thing. No argv can fix that;
it is now detected by exit code and explained.
- [ ] `↑` recalls commands from a previous Colima session in the same profile.
- [ ] Two profiles do not share history.
- [ ] Session metadata is recorded; session content is not.

## Risk Assessment

Part A is blocked on Phase 2 — `kubectl exec -it` without a real TTY reproduces
exactly the bug this plan exists to fix. Part B's only real trap is scope creep
into keystroke parsing; the table above is the guard against it.
