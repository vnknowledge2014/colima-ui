# Detonation (phase 6) fix verification — 2026-08-13

Re-review of the fixes for `plans/reports/from-code-reviewer-to-cook-260813-0823-detonation-phase6-review.md`.
No code was modified.

Verdict: **C1 and C2 are genuinely fixed.** Four of the remaining findings are only
partially fixed, and three of the "replaced" tests are still phantom tests — including one
that passes for the wrong reason.

## Finding-by-finding

| # | Status |
|---|---|
| C1 context custody inert under `DOCKER_HOST` | **Fixed** |
| C2 `detect_docker_host` hijack | **Fixed** (one residual fallback path, see N2) |
| H1 concurrent sessions | **Fixed** |
| H2 no deadline / cancel before loop | **Partially fixed** — see N1 |
| H3 UI loses the session | **Fixed**, with a new race — see N3 |
| H4 restore to a deleted context | **Partially fixed** — see N4 |
| M1 duplicate `{#each}` keys | Fixed |
| M2 image-ref argv injection | Fixed; no false rejections found (see "validate_image_ref") |
| M3 arbitrary export directory | **Not fixed** — only the filename half — see N5 |
| M4 kill-before-log | Fixed (`detonation.rs:711-716`) |
| M5 unbounded `SESSIONS` | Fixed, no deadlock (see "prune") |
| M6 `cancel()` on terminal session | Fixed (`detonation.rs:496-498`) |

### C1 — verified fixed on every path
`docker_context_cmd` (`src-tauri/src/commands/detonation.rs:316-326`) builds the `Command`
directly with `.env_remove("DOCKER_HOST")`, and it is the only route to
`current_docker_context` / `restore_docker_context`. Both `teardown_profile`
(`detonation.rs:378, 391`) and `sweep_orphans` → `teardown_profile` (`detonation.rs:406`)
go through it, so the out-of-session paths are covered.

Checked and **not** a regression: `docker_context_cmd` uses a bare `Command::new("docker")`
rather than `helpers::build_cmd`, which would normally be an ENOENT risk in a `.app` bundle.
`path_util::fix_path_env` (`path_util.rs:82-152`) `set_var`s the merged login-shell PATH into
the process env from `main()`, and `build_cmd` also uses a bare `Command::new(program)`
(`helpers.rs:105`) — so both resolve identically. No defect.

### C2 — guard is in the right place
`path_util.rs:231-233` skips detonation profiles inside the `read_dir` loop, before the
`ha.sock`/`docker.sock` check, so no enumerated path can return one. All callers of
`detect_docker_host` route through that single function (`docker_state.rs:21,81`,
`helpers.rs:110`, `adapters/docker.rs:25`, `commands/runtime.rs:10`,
`commands/containers.rs:904`, `commands/metrics_collector.rs:398`) — no parallel scanner, no
cached host. `connect_bollard` falling through to `connect_with_defaults()` cannot reach the
detonation VM either. `DETONATION_PROFILE_PREFIX` has exactly one definition
(`path_util.rs:195`), re-imported at `detonation.rs:69`.

## Open defects

### N1 (High) — neither setup step is bounded; a hung `colima start` locks the feature out permanently
`detonation.rs:618-633, 657`. `run_cmd` is still `Command::output()` with no timeout. The
new cancel checks (`648`, `664`) only run *between* steps, so during a hung `colima start` or a
stalled pull: the session stays `Preparing`, `cancel()` flips a flag nobody reads, the UI's
Stop button does nothing, and `has_active_session()` (`518-523`) stays true — so `start()`
(`781`) rejects every future session for the life of the process. The same trap fires if
`run_session_blocking` ever panics: `spawn_blocking`'s `JoinError` is discarded at
`detonation.rs:814` (`let _ = …await`), the session is never moved to a terminal status, and
the VM is not torn down. Bound both commands, and mark the session `Failed` when the blocking
task returns `Err`.

### N2 (Medium) — the `~/.colima/docker.sock` fallback is still unguarded
`path_util.rs:253-257`. When no non-detonation profile qualifies — i.e. the user's own Colima
is stopped while a detonation runs — the loop finds nothing and the fallback returns the
top-level socket, labelled `"default"`. On this machine that path is a real socket
(`srw------- ~/.colima/docker.sock`), not a symlink, and I could not determine without
starting an instance whether `colima start -p <detonation>` re-points it. If it does, C2's
hole reopens for exactly the "user stopped their own VM" case. Cheap mitigation: resolve the
socket's owning profile, or skip the fallback while a detonation session is active.

### N3 (Medium) — `onMount` adoption races `start()`
`src/components/security/DetonationPanel.svelte:105-119`. The `await detonationApi.list()` is
in flight while the component is interactive. If the user clicks Start before it resolves,
the response (taken *before* the new session existed) overwrites `session`:
either `session = live` + `startPolling(live.id)` at `109-111`, which `stopPolling()`s the
poll `start()` just created, or `session = all[0]` at `113`, which replaces the live session
with an old finished one and leaves no poll and no Stop button while a VM runs. Narrow
(human click vs. local IPC) but it is the same "visible and unreachable" state the adoption
was added to prevent. Guard the assignment with `if (session) return;`, or await the mount
promise in `start()`.

### N4 (Medium) — H4's fallback was never implemented
`detonation.rs:340-361`. The guard refuses to restore *to* a detonation context and then
returns — it does not fall back to a real one. On the crash-recovery path
(`sweep_orphans` → `teardown_profile`), the ambient context at entry **is** the detonation
context, so `previous` is a detonation name, the guard trips, and the instance is deleted with
`~/.docker/config.json` `currentContext` still naming a context that no longer exists. Whether
the user's `docker` CLI is left broken then depends on whether `colima delete` itself resets
the setting, which I could not confirm from the repo. Manual check: start a detonation, `kill -9`
the app, restart it, then run `docker context show` and `docker ps` in a fresh shell. The
docstring at `338-339` claims the existence check covers this case; it does not — the
existence check is never reached, because the detonation-name guard returns first.
Fix: when `previous` is a detonation context, restore to the first existing non-detonation
context instead of giving up.

### N5 (Medium, unchanged) — `export_session` still writes to any directory the caller names
`detonation.rs:824-838` and `routes/detonation.rs:57-67`. Only the filename half of M3 was
addressed. `dir` still arrives from the HTTP body and is used as `assert_path_within`'s own
base, so the confinement is vacuous: an authenticated API caller picks any writable directory
on the machine and overwrites any file there with session JSON. Confine to an app export
directory (`path_util::app_data_dir`) or to a directory the user picked in a native dialog.

## Test quality — three tests still cannot fail

- `export_refuses_a_filename_that_is_really_a_path` (`detonation.rs:1019-1025`) **passes for
  the wrong reason.** `export_session` resolves `get_session(id)` first (`825`) and the test
  passes `"nope"`, so every case returns `Unknown session nope` and never reaches the filename
  check at `829`. Deleting the separator guard entirely leaves this test green. Insert a real
  session into `SESSIONS` first, or move the filename validation above the session lookup.
- `timeouts_are_clamped_to_the_ceiling` (`973-979`) still re-implements the clamp as a local
  closure. This was on the list of four to replace and was not. Extract the clamp
  (`fn clamp_timeout(asked: Option<u64>) -> u64`) and call it from both.
- `a_detonation_context_is_never_restored_to` (`999-1016`) re-implements the guard as a local
  closure; removing the guard from `restore_docker_context` leaves it green. Extract
  `fn is_detonation_context(name: &str) -> bool` and have both call it.

Genuinely constraining now: `build_run_args` (`928-954`), `stale_profiles` (`915-925`),
`parse_top_line` (`982-996`), `validate_image_ref` (`957-970`), `prune_sessions` (`1028-1056`).

## Specifically checked, no defect found

**`validate_image_ref` false rejections** (`detonation.rs:212-230`): the allowed set
`[alnum . - _ / : @ +]` accepts `localhost:5000/img`, `registry:5000/ns/img:tag`,
`alpine@sha256:<hex>`, `ghcr.io/o/n:1.2.3+build`, and multi-segment paths. Docker reference
grammar permits nothing outside this set (`~` and `%` are not legal in a reference), so no
legitimate ref is rejected. One cosmetic gap: validation trims a *copy*, so
`" alpine:latest"` validates and then reaches argv untrimmed (`detonation.rs:656, 683`) and
fails at pull. Trim once in `start()` and store the trimmed value.

**No deadlock from `prune_sessions`**: it takes `&mut HashMap` (`533`) and never touches
`SESSIONS`, so the call under the guard at `806` is sound. `has_active_session()` (`781`)
releases its lock before the second acquisition at `799`, and the recheck at `803` closes that
window. No lock is held across a blocking command or an `.await` anywhere in the module.

**Vitest breakage — your reading is right.** `npx vitest run src/lib/*.test.ts` fails during
*reporter module load*, before any test file is resolved (`ERR_LOAD_URL` out of
`loadCustomReporterModule`). It is a vitest 4.1.10 / vite 8.2.1 / jsdom 30 / undici 8.10 vs.
Node 20.19.1 toolchain problem, not attributable to phase 6. Consequence worth stating
plainly: `DetonationPanel.svelte` and `DetonationTimeline.svelte` currently have **no**
executable test coverage of any kind, so N3 would not have been caught by CI either.

## Recommended order

1. N1 (permanent feature lockout + leaked VM).
2. N4, then N2 — both are the remaining edges of the custody guarantee.
3. N5, N3.
4. The three phantom tests; `export_refuses_a_filename_that_is_really_a_path` first, since it
   currently certifies a guard it never executes.

## Unresolved questions

- Does `colima delete -p <profile> -f` reset `currentContext` when the deleted instance owned
  it? Answering this decides whether N4 is user-visible breakage or only latent.
- Does `colima start -p <profile>` re-point `~/.colima/docker.sock`? Decides N2's severity.
