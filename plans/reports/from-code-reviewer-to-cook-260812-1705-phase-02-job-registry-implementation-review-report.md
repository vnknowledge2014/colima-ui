# Phase 2 — Job registry and recovery: implementation review

Reviewer: code-reviewer · Date: 2026-08-12 · Branch `dev`
Scope: `src-tauri/src/transfer_registry.rs` (new), `commands/file_transfer.rs`,
`routes/file_transfer.rs`, `api_server.rs`, `lib.rs`, `src/lib/api/transfer.ts`,
`src/components/transfer/TransferDialog.svelte`, `tests/file_transfer_against_docker.rs`,
`docs/api.md`.

## Verification run

| Check | Result |
|---|---|
| `cargo test --lib` | 304 passed, 0 failed |
| `cargo test --test file_transfer_against_docker -- --ignored --test-threads=1` | 8 passed (Docker live) |
| `pnpm lint:rust` (clippy `-D warnings`, all targets/features) | clean |
| `pnpm vitest run` | 188 passed / 16 files |
| `pnpm check` (svelte-check) | 359 files, 0 errors, 0 warnings |
| `pnpm lint` (eslint `--max-warnings=0`) | clean |

No new lint/type/build error. (g) regressions: none found in existing behaviour;
the only behavioural change to an existing path is the destination-conflict refusal
(see M4).

---

## (a) Lock discipline and deadlock — clean, with one poisoning concern

**No deadlock and no lock-order inversion.** Traced every path:

- `transfer_registry` never calls out of the module while holding `REGISTRY`.
  Inside the guard there is only `prune`, `HashMap` work, `format!`, `PathBuf::display`
  and `now_ms` (`transfer_registry.rs:124-159, 189-202`).
- `cancel_transfer_job` (`commands/file_transfer.rs:662-667`) takes `REGISTRY` in
  `request_cancel`, and that guard is dropped at the end of `request_cancel` before
  `cancel_stream` takes `RUNNING`. Sequential, not nested.
- The reverse order (`RUNNING` held while taking `REGISTRY`) does not occur either:
  `update_bytes` is called from `emit_progress`, which reaches
  `wait_with_progress`'s `tick()` (`streaming_cmd.rs:441-450`) — the child guard is
  scoped and released before `tick()`, and `RUNNING` itself is not held during the
  wait loop at all.

**H3 — poisoned-mutex defaults are not all safe.**
`REGISTRY` poisoning is permanent for the process, and the defaults diverge:

| Path | Default on poison | Consequence |
|---|---|---|
| `register` `:124-126` | `Err` | Safe: transfer refused. But every future transfer is refused for the session. |
| `set_running` `:164-166` | `true` | Job proceeds even if cancelled during startup. |
| `update_bytes` `:178` | no-op | Harmless. |
| `settle` `:190` | no-op | **Entry stuck non-terminal forever**, destination claim leaked. |
| `list` `:206-208` | `Vec::new()` | **Silently reports "no transfers" while jobs are running.** A reconciling client concludes everything finished. |
| `request_cancel` `:235-237` | `UnknownJob` | Client told the job does not exist while it still runs. |

The map has no invariant a panic could break, so `unwrap_or_else(|e| e.into_inner())`
is the correct policy here — the test helper already does exactly that
(`transfer_registry.rs:275`) while production code does not. Recommend a single
`fn lock() -> MutexGuard<...>` that recovers from poisoning, so `list`/`settle`
cannot silently lose jobs. Rating: **High** (silent data loss on the reconciliation
path is precisely what this phase exists to prevent), low likelihood.

---

## (b) The cancel race — a real window remains; the claim is overstated

Registering intent before spawn closes the window between `start_*` returning and the
`spawn_blocking` task being scheduled. It does **not** close the window between
`set_running` and `streaming_cmd::register`.

**H1 — a cancel issued in that window is silently dropped, and the caller is told it
was honoured.** Concrete interleaving:

```
T1 (blocking task)                          T2 (cancel from UI/HTTP)
------------------------------------------- -------------------------------------
file_transfer.rs:302  set_running() -> true
                                            request_cancel()  registry.rs:238-244
                                              status == Running (not terminal)
                                              cancel_requested = true
                                              -> CancelOutcome::Cancelled
                                            cancel_stream()   streaming_cmd.rs:158-170
                                              RUNNING has no entry yet -> false
                                            file_transfer.rs:665 discards that false
streaming_cmd.rs:303  File::create(.part)
streaming_cmd.rs:306  spawn(child)
streaming_cmd.rs:317  register(job_id)  <-- nothing ever re-reads cancel_requested
... runs to completion, rename onto destination ...
file_transfer.rs:368  settle(Success) + emit transfer.done { cancelled: false }
```

Result: the user is told `cancelled`, `TransferDialog.svelte:202-207` keeps the
"cancelling…" state waiting for the terminal event, the event arrives as
`cancelled: false`, and the file is published to the destination the user asked not
to write. `cancel_requested` is write-only after `set_running` has passed.

Window size: `File::create` + `fork/exec` of the runtime. Milliseconds locally;
seconds on a network volume, an SMB/NFS destination, or a cold Docker CLI — and a
user cancelling *immediately* after clicking start is exactly the case the phase
targets.

A second, wider instance of the same class: after `run_streaming` returns and before
`settle` (`file_transfer.rs:350-399`), and inside `stream_to_file` between
`unregister` (`streaming_cmd.rs:326`) and the `rename` (`:345`) — `request_cancel`
returns `Cancelled` and `cancel_stream` returns false; the rename publishes the file
anyway.

Minimal fix: make the intent flag shared rather than checked once — store the
`Arc<AtomicBool>` in the registry `Entry` and hand it to `streaming_cmd::register`
so `cancel_stream` and `request_cancel` set the same bit, and `stream_to_file`
re-checks it after `register` and again before `rename`. Cheaper interim fix:
re-check `registry::cancel_requested(job_id)` immediately after
`streaming_cmd::register` succeeds. Either way, `cancel_transfer_job` should stop
discarding `cancel_stream`'s boolean (`file_transfer.rs:665`) — a `Cancelled` that
killed nothing is the untruth this enum was introduced to remove.

Rating: **High**.

---

## (c) Destination conflict — the `start_*` ordering is correct; two non-settling paths remain

**Verified clean (the one you flagged as mattering most):** in all four `start_*`
functions `registry::register` is the last fallible operation before `spawn_job`;
nothing between them can `?`-return.

- `start_image_save` — `register` `:452-459`, then infallible arg building, then
  `spawn_job` `:465`.
- `start_image_load` — `register` `:493-499`, then `spawn_job` `:500`.
- `start_copy_to_container` — `register` `:550`, then `spawn_job` `:551`.
- `start_copy_from_container` — `register` `:620`, then `spawn_job` `:621`.

All validation, `resolve_destination`, `estimate_image_bytes` and the disk-space
checks run **before** `register`. No leak on that path.

`spawn_job`'s early return on cancel does call `settle` (`:303`), which clears
`destination` (`registry.rs:198`). Clean.

**M1 — a panic (or a never-scheduled task) inside `spawn_blocking` leaks the claim
permanently.** If the closure at `file_transfer.rs:294-400` unwinds, or the runtime
is shutting down when the blocking task would be scheduled, no `settle` runs. The
entry keeps `status = Starting|Running` and `destination = Some(path)` forever:
`prune` only drops entries with `finished_at` (`registry.rs:106-109`), so the path is
refused for the remainder of the process and `transfer_list` shows a phantom running
job that can never be cancelled (`cancel_stream` finds nothing). Panic sources are
narrow (Tauri `emit` serialisation, an allocation failure) but not zero. Recommend an
RAII guard whose `Drop` settles `Failed` if nothing settled it — that also covers
future edits to this closure. Rating: **Medium**.

---

## (d) Retention vs conflict — reachable, same root cause as M1

`prune` is correct for terminal entries. A non-terminal entry is immortal by design.
Reachable ways to get one:

1. The panic / unscheduled-task case above (M1).
2. `settle` on a poisoned registry (H3) — no-op, entry never becomes terminal.

Neither is reachable through normal operation, so this is not a leak in the steady
state; both need the same defensive fix. There is no watchdog for a `Starting` entry
that never becomes `Running`. Rating: **Medium** (folded into M1/H3).

---

## (e) `redact` — applied at the right place, but it does not do what the criterion claims

Coverage is correct where it is applied: `TransferSnapshot.error` is set on exactly
one path (`file_transfer.rs:381-397`) and that path redacts before both `settle` and
`emit`. The `Cancelled`/`Success` branches set `error: None`. HTTP `err()` and the
IPC `ColimaError` both redact on construction (`error.rs:81`, `:248-252`). So there
is no unredacted `error` field anywhere.

**M2 — `redact()` does not remove host paths; it only anonymises the account
segment.** `HOME_PATH` (`redact.rs:200-202`) matches `/(Users|home)/<name>` and
replaces only `<name>`. Concrete stderr that survives intact:

- `Cannot create /Volumes/T7Backup/exports/img.tar.save-3.part: Permission denied`
  — built at `streaming_cmd.rs:304`, no `/Users` or `/home` prefix, passes through
  unchanged.
- `/private/var/folders/qx/T/colimaui/out.tar`, `/opt/data/...`, `/mnt/...` — same.
- `failed to read /Users/longnd/clients/acme-confidential/img.tar` becomes
  `/Users/<user>/clients/acme-confidential/img.tar` — the account name goes, the rest
  of the path (project and client names) does not.

The success criterion "`transfer.failed` không mang đường dẫn host chưa redact" is
therefore only partially met. Given Phase 6 pushes this text into the OS notification
centre, decide explicitly: either accept it (the user chose the path and already
knows it) and correct the criterion, or add a path-shortening step for the transfer
error specifically. Do **not** widen `redact.rs` globally — its negative tests
(`:431-439`, `:559-565`) exist because over-redaction previously destroyed
diagnostics. Rating: **Medium**, mostly a claim/documentation correction.

---

## (f) Contract sweep — complete, with one coverage gap

`cancel_transfer` `bool` → `CancelOutcome` consumers, all checked:

| Consumer | Handling |
|---|---|
| Tauri command `commands/file_transfer.rs:651` | returns `Result<CancelOutcome, ColimaError>`; registered `lib.rs:210` |
| HTTP route `routes/file_transfer.rs:69-75` | `ApiResponse<CancelOutcome>`; `/api/transfers/cancel` behind the auth layer (`api_server.rs:132`, layer at `:269`) |
| TS client `src/lib/api/transfer.ts:128` | `call<CancelOutcome>` |
| `TransferDialog.svelte:199-217` | all three outcomes handled; `alreadyFinished`/`unknownJob` settle the dialog |
| `TransferDialog.test.ts:47,120,136` | all three mocked |
| Rust unit test `transfer_registry.rs:331-340` | all three asserted |

**Serde strings verified empirically** (compiled the same derives in a scratch crate):
`["cancelled","alreadyFinished","unknownJob"]` and
`["starting","running","success","failed","cancelled"]` — exactly the TS unions at
`transfer.ts:42` and `:44`. No drift.

`GET /api/transfers` is inside the token-protected router; documented at
`docs/api.md:179-205`.

**L1 — no end-to-end coverage of the new cancel contract.** The integration test
cancels via `streaming_cmd::cancel_stream` directly
(`tests/file_transfer_against_docker.rs:440`), not `cancel_transfer_job`, so the enum
path — the very thing that changed — is exercised only by the registry-only unit
test. The race in (b) would not be caught by any current test. Rating: **Low**
(coverage), but it is the gap that hides H1.

---

## Other findings

- **M4 — behaviour change not documented and a now-stale comment.** Two jobs aimed
  at one destination are now refused at start (`registry.rs:129-139`), including when
  the user explicitly passed `overwrite: true`. `streaming_cmd.rs:263-266` still
  states that two jobs may legitimately target one path and "the loser of the rename
  is simply overwritten" — that is no longer reachable through `start_*`. Update the
  comment and add the new failure to `docs/api.md`, or the next reader will trust the
  stale one. Rating: **Medium** (correctness of documentation, not of code).
- **L2 — missing i18n key.** `TransferDialog.svelte:206` uses
  `t("transfer.job_unknown", { default: ... })`; the key exists in none of
  `src/locales/{en,ja,vi,zh}.json`, so all four languages show English.
- **L3 — `transferApi.list()` has no caller yet.** Expected (Phase 3), noted so it is
  not mistaken for dead code.

---

## Success criteria status

| Criterion | Status |
|---|---|
| `transfer_list` over IPC and HTTP | met (`lib.rs:211`, `api_server.rs:133`) |
| Failed-spawn job still listed with an error state | met (`spawn_job` settles `Failed`; registered before spawn) |
| Finished job readable ~60s | met, integration-tested (`a_finished_transfer_is_still_readable_afterwards`) |
| `cancel_transfer` distinguishes three outcomes | met at the type level; **not truthful in the H1 window** |
| `transfer.failed` carries no unredacted host path | **partially met** — see M2 |
| `GET /api/transfers` rejects a tokenless request | met (protected router) |

## Recommended actions

1. **H1** — share the cancel flag between the registry and `streaming_cmd`, or
   re-check `cancel_requested` after `streaming_cmd::register` and before `rename`;
   stop discarding `cancel_stream`'s return (`file_transfer.rs:665`).
2. **H3** — recover from registry poisoning (`into_inner()`), so `list` and `settle`
   cannot silently lose jobs.
3. **M1/M4** — settle-on-drop guard in `spawn_job`; update the stale comment at
   `streaming_cmd.rs:263-266` and document the conflict refusal.
4. **M2** — decide and record whether unredacted non-home host paths in
   `transfer.failed` are acceptable; correct the phase criterion either way.
5. **L1/L2** — add an integration test that cancels through `cancel_transfer_job`
   (ideally one that cancels within the startup window); add `transfer.job_unknown`
   to the four locale files.

## Unresolved questions

1. Is the destination-conflict refusal intended to apply even with
   `overwrite: true`? It currently does, and no UI copy explains it.
2. Should `transfer.failed` keep full non-home host paths once Phase 6 pushes it to
   the OS notification centre?

Status: DONE_WITH_CONCERNS
