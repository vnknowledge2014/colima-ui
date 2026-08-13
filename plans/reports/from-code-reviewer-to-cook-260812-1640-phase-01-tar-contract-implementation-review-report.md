# Phase 01 — Test harness and TAR contract: implementation review

Reviewer: code-reviewer · Date: 2026-08-12 · Branch: `dev`
Plan: `plans/260812-1247-notification-center-background-transfers/phase-01-test-harness-and-tar-contract.md`

## Scope

Reviewed in full (see note R0 — there is no diff to review):

- `src-tauri/src/streaming_cmd.rs` (844 lines)
- `src-tauri/src/commands/file_transfer.rs` (881 lines)
- `src-tauri/src/routes/file_transfer.rs`, `src-tauri/src/routes/payloads.rs:153-183`
- `src-tauri/tests/file_transfer_against_docker.rs`
- `src/components/transfer/TransferDialog.svelte`, `TransferDialog.test.ts`
- `src/lib/normalizers.test.ts`, `package.json`, `.nvmrc`, `docs/api.md:170-205`

## Verification run

| Command | Result |
|---|---|
| `pnpm vitest run` | 16 files / 184 tests passed |
| `pnpm check` (svelte-check) | exit 0 |
| `pnpm lint` (eslint --max-warnings=0) | exit 0 |
| `cargo clippy --all-targets --all-features -- -D warnings` | exit 0 |
| `cargo test --lib` | 295 passed, 0 failed |
| `cargo test --test file_transfer_against_docker -- --ignored --test-threads=1` | 6 passed, 0 failed (18.8 s) |

Acceptance criteria 1, 3, 4, 5, 6, 7 are demonstrated by these runs. Criterion 2
is met on the backend and **unverified on the frontend** — see F2.

---

## Findings

### R0 (Process, blocking for landing) — nothing is committed; there is no diff

`git status` reports `?? src-tauri/src/commands/file_transfer.rs`,
`?? src-tauri/src/streaming_cmd.rs`, `?? src-tauri/tests/file_transfer_against_docker.rs`,
`?? src/components/transfer/`, `?? .nvmrc`. All of the phase's primary artefacts are
untracked. Consequences: no before/after comparison was possible (this review is a
whole-file audit, not a delta review), and CI has never compiled or run any of it.
Stage and commit before requesting a landing decision.

### F1 (High) — two concurrent jobs to the same destination silently corrupt each other

`partial_sink_path` derives the scratch path purely from the destination
(`src-tauri/src/streaming_cmd.rs:261-265`), and the only collision guard is
`register`, which dedups on `job_id` (`src-tauri/src/streaming_cmd.rs:193-199`).
`job_id` is a monotonic counter (`src-tauri/src/commands/file_transfer.rs:32,41-43`),
so two jobs targeting the same file **always** get distinct ids, both
`File::create` the same `.part` (`src-tauri/src/streaming_cmd.rs:282`), interleave
their writes, and both `rename` onto the destination
(`src-tauri/src/streaming_cmd.rs:324`). `resolve_destination` checks only
`candidate.exists()` (`src-tauri/src/commands/file_transfer.rs:207`) and never the
`.part` sibling, so with `overwrite: false` neither job is refused — the destination
does not exist yet for either.

The doc comment at `src-tauri/src/streaming_cmd.rs:76-78` asserts the opposite:

> `job_id` must be unique among running streams: two commands writing the same
> destination would corrupt each other's output, so a collision is refused rather
> than resolved.

That protection does not exist for any id this codebase generates. Either key the
refusal on the resolved sink path (a `HashSet<PathBuf>` alongside `RUNNING`), or make
the scratch name unique per job (`<name>.<job_id>.part`) and accept the leftover-file
tradeoff. Correct the comment either way — a false safety claim in a doc comment is
how the next change reintroduces the bug.

### F2 (High) — `withTarSuffix` has no test; acceptance criterion 2's frontend half is unverified

`src/components/transfer/TransferDialog.svelte:115-122` is new behaviour that the
phase explicitly calls for ("tự thêm `.tar` vào ô input"), and
`src/components/transfer/TransferDialog.test.ts` already asserts the exact call
arguments of `saveImages` at `:100-112` — the harness to prove it exists and was not
used. Nothing anywhere exercises: a name typed without an extension, an uppercase
`.TAR`, or the deliberate pass-through of `.tgz`. `withTarSuffix` is not exported, so
this must be a component test (type `images` into the file-name field, click Start,
assert `saveImages` was called with `images.tar`).

The phase's stated point of wiring up `pnpm test` first was so that "test xanh" means
something. A green suite that does not touch the new frontend behaviour does not.

### F3 (Medium) — `overwrite: false` is defeated by a TOCTOU window the rename widens

`resolve_destination` refuses an existing destination at start time
(`src-tauri/src/commands/file_transfer.rs:207-212`), but the final
`std::fs::rename` (`src-tauri/src/streaming_cmd.rs:324`) replaces the destination
unconditionally. For a multi-gigabyte `docker save` that window is minutes long: a
file created at that path in the meantime is destroyed even though the user declined
overwriting. Close it with a re-check immediately before the rename, or
`renameat2(RENAME_NOREPLACE)` on Linux with a checked fallback on macOS. Note this is
strictly the *opposite* failure from the one the phase fixed — worth not trading one
for the other.

### F4 (Medium) — terminal events can arrive before the dialog knows its own job id

`spawn_job` launches the tokio task inside `start_*`
(`src-tauri/src/commands/file_transfer.rs:419,447,497,566`) — i.e. before the IPC
reply is returned to the frontend. Every handler filters on `e.jobId !== jobId`
(`src/components/transfer/TransferDialog.svelte:73,79,90`), and `jobId` is only
assigned after the await at `:188`. A job that fails or completes fast (`docker cp` of
a small file, or an immediate runtime error) emits `transfer.done`/`transfer.failed`
into a dialog that discards it. The dialog is then stuck with `running === true`
(`:52`, since `jobId !== null && !finished`), showing only "Cancel transfer" for a job
that already ended, with no path back to a usable state short of closing the modal.

Pre-existing relative to this phase, but it lives in a file this phase edited and it
undermines the same user-visible flow. Fix by buffering events keyed by job id until
the id is known, or by having `spawn_job` defer the actual start until the caller has
been handed the id.

### F5 (Medium) — `active_stream_count() == 0` no longer implies the artefact is published

`unregister` runs at `src-tauri/src/streaming_cmd.rs:305`, before the rename at
`:324`. Any caller treating "no active streams" as "output is on disk" now has a
window where the destination does not exist. The two new integration tests dodge this
by also polling for the file (`src-tauri/tests/file_transfer_against_docker.rs:214,312`),
which is correct, but the invariant is undocumented and the next test author will not
know. Either move `unregister` after the rename or state the ordering in the
`stream_to_file` doc comment.

### F6 (Medium) — frontend and backend disagree on which names get the "uncompressed" message

`src/components/transfer/TransferDialog.svelte:120` leaves `.tgz|.gz|.zip|.bz2|.xz`
untouched, and the comment at `:110-113` justifies this as "so the backend's clearer
message reaches the user". But `require_archive_name`
(`src-tauri/src/commands/file_transfer.rs:127`) lists only `.tar.bz2` and `.tar.xz`,
not bare `.bz2`/`.xz`. So `data.xz` is passed through by the frontend and then gets
the generic `File name must end in .tar` rather than the explanatory message the
comment promises. Add `.bz2` and `.xz` to the backend list, or narrow the frontend
regex to match it.

### F7 (Low) — save-picker path split assumes a POSIX separator

`src/components/transfer/TransferDialog.svelte:152` splits on `"/"` only. On Windows
`cut` would be `-1`, giving `destDir = "/"` and `fileName` equal to the entire path.
Not a live defect for a Colima UI, but it is an undocumented platform assumption in a
security-relevant split (the folder/name separation is what makes
`assert_path_within` meaningful — `src-tauri/src/commands/file_transfer.rs:203-205`).

### F8 (Low) — symlink destinations change meaning, undocumented

A `.tar` destination that is a symlink pointing inside the chosen base used to be
written *through* by `File::create`; `rename` now replaces the symlink itself with a
regular file. A symlink pointing outside the base is still correctly refused, because
`assert_path_within` canonicalizes an existing candidate
(`src-tauri/src/validation.rs`, `resolved = candidate.canonicalize()`). Behaviour
change only, no data-loss path — but `docs/api.md`'s `.part` paragraph would be the
place to say it.

---

## Categories with no findings

- **(a) Blast radius of the `spawn_job` / `JobPaths` change — clean.**
  `OutputSink::ToFile` has exactly one construction site
  (`src-tauri/src/commands/file_transfer.rs:308`) and `run_streaming` exactly one
  caller (`:334`). `spawn_job` has four callers, all in that file (`:419,447,497,566`),
  all with correct arity and correct `sink_path` (`Some` only for `image_save` and
  `copy_from_container`; `None` for `image_load` and `copy_to_container`, which write
  nothing host-side). `JobPaths` has zero remaining references in `src/` or `tests/`.
  The diagnostics bundle (`src-tauri/src/commands/diagnostics.rs`) does **not** use
  `streaming_cmd` at all — the concern about it in the review brief does not apply.
  The only consumer that watched the file while the command ran was the cancellation
  integration test, and it was correctly migrated to `partial_sink_path`
  (`src-tauri/tests/file_transfer_against_docker.rs:17,350`) rather than re-deriving
  the convention.

- **(b) The rename — safe apart from F1/F3.** `partial_sink_path` uses
  `with_file_name` (`src-tauri/src/streaming_cmd.rs:264`), so the scratch file is
  always a sibling in the same directory and therefore the same filesystem; `EXDEV` is
  unreachable. Destination-is-a-directory yields `EISDIR`/`ENOTDIR` from `rename`,
  which is propagated as `Cannot write …` with the `.part` removed (`:324-327`) — an
  error, not data loss. A leftover `.part` from a crashed run is truncated by
  `File::create`, which is the desired behaviour.

- **(c) `sniff_tar` arithmetic — correct.** `take(512).read_to_end`
  (`src-tauri/src/commands/file_transfer.rs:159`) removes the short-read dependency
  that a bare `read` would have introduced. Every slice is length-guarded first
  (`:162,168,172,177`). Empty file → `head.len() == 0` → falls through to the final
  rejection. Exactly 262 bytes with the magic at 257 → `head[257..262]` is in bounds
  and accepted. Under 512 bytes without magic → rejected, which is the intended
  "merely short" case and is covered at `:848-854`. The `.tar`-named 512-byte fallback
  is deliberate and matches the phase's stated preference for false accepts over false
  rejects.

- **(d) `--` terminator — verified empirically, not assumed.** Against Docker 29.7.2:
  `docker cp -- <id>:/etc/hostname -` exits 0 and produces a tar `tar -tf` lists, and
  `docker save -- alpine:3.19` exits 0 producing a 3.4 MB archive. Docker, nerdctl and
  podman all use cobra/pflag, which honours `--`. `docker load -i <path>`
  (`src-tauri/src/commands/file_transfer.rs:451-454`) correctly has no terminator: the
  path is a flag *value*, not a positional, and `require_plain` → `reject_flag_like`
  (`:94-102`) already refuses a leading `-`.

- **(e) `require_archive_name` — no wrongful rejections found.** `my.tar.backup.tar`,
  `archive.zip.tar`, a mid-string `.zip`, and `Images.TAR` are all accepted (the last
  is covered at `:787`). The `.gz` entry sits last in the list but every branch returns
  the same message, so ordering is immaterial; `.tar.gz` and a bare `.gz` are both
  caught. See F6 for the one real gap (`.xz`/`.bz2` without a `.tar` prefix).

- **(f) Svelte 5 reactivity — no surprise.** `fileName = withTarSuffix(fileName)` at
  `:172,179` is a plain `$state` write bound to an `<input>`; the runes compiler
  propagates it to the DOM with no loop, and `canStart` (`:208-214`) cannot flip
  mid-submit because `starting` is already `true` at that point. The copy-out hint is
  correctly scoped to `{#if mode === "copy-out"}` (`:280-289`), and the open-picker
  `TAR_FILTER` is correctly gated to `mode === "import"` (`:138`) so copy-in can still
  select any host file.

- **(g) Public contracts — HTTP and Tauri cannot diverge.** `routes/file_transfer.rs`
  calls the same `start_*` functions with identical arguments and `None` for the app
  handle (`:20,35,44,58`); there is no second validation path. `docs/api.md:189-200`
  matches the code exactly, including the `.tar` requirement on both write endpoints,
  the pre-runtime rejection on load, the file-or-directory archive clause, and the
  `.part`/rename sentence.

- **(h) New lint/type/build errors — none.** All five gate commands are clean.

## Recommended actions

1. Commit the untracked files so CI and a real diff review exist (R0).
2. Guard the sink path, not just the job id, against concurrent jobs; correct the
   false claim in the `run_cmd_streaming` doc comment (F1).
3. Add a component test proving `withTarSuffix` normalises a typed name and passes
   compressed suffixes through (F2).
4. Close the `overwrite: false` rename window (F3).
5. Buffer transfer events until the dialog knows its job id (F4).
6. Document or fix the `unregister`-before-`rename` ordering (F5).
7. Align the compressed-extension lists across frontend and backend (F6).

## Unresolved questions

- Is concurrent transfer to one destination in scope for Phase 1, or deferred to the
  notification-centre phase where multiple background jobs become the norm? F1 gets
  materially worse once the UI encourages parallel transfers.
- Was F4 (event/job-id race) already known and deferred? It is pre-existing but is
  exactly the class of bug the notification-centre work will amplify.
