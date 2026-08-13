# Phase 3 — Notification store core: implementation review

Date: 2026-08-12
Reviewer: code-reviewer
Verdict: **DONE_WITH_CONCERNS** — no blocker, but four correctness defects should land before Phase 4/5 build UI on this store.

## Verification run

| Command | Result |
|---|---|
| `pnpm vitest run` | 17 files / **204 passed** |
| `pnpm check` (svelte-check) | 360 files, **0 errors, 0 warnings** |
| `pnpm lint` (eslint --max-warnings=0) | **clean** |

No new type/lint/build error. (g) is clean on the tooling side.

---

## (a) Svelte 5 runes correctness — clean

`notificationState` is `$state({...})` in a `.svelte.ts` module (`src/store/notifications.svelte.ts:82`). Svelte 5 `$state` is a **deep** proxy, so all four of the questioned patterns are correct:

- `entry.status = ...` (`:229`), `entry.job.bytes = ...` (`:193`, `:266`) — the object was `unshift`ed into a proxied array, so it is itself proxied on write; nested mutation notifies. Correct.
- `notificationState.entries = [...]` in `trim` (`:107`), `clearFinished` (`:312`), `clearErrorLog` (`:349`) — reassignment of a property on a `$state` object is tracked. Components hold no reference to the *array*; `ErrorDetailPanel` re-derives per read, so no stale reference. Correct.
- `const entries = $derived(errorEntries())` (`ErrorDetailPanel.svelte:17`) — `$derived` tracks every state read during synchronous evaluation, including inside a called function. `errorEntries()` reads `entries` (length + indices) and each `e.error`. Not a mistake in Svelte 5.
- `unreadCount()` (`:89`) is a plain function, correct **only** when called inside a reactive context. The JSDoc at `:88` claims "Derived, so trimming cannot desync it" — it is not `$derived`; it is a plain filter. Harmless today (no caller yet) but Phase 4 must call it in a template/`$derived`, not assign it to a plain `let`. **Fix the comment**, it will mislead the Phase 4 implementer.

One caveat worth stating: `pushNotification` builds `entry` as a plain object and `unshift`es it (`:161`); the returned value is the id (a number), not the object, so no un-proxied reference escapes. Correct as written — but any future `return entry` here would hand out the raw object and silently break reactivity for that caller.

---

## (b) Double-entry — clean, contract holds

Grep over `src/` for `recordError`: exactly **one** call site outside the store — `errorReporter.ts:34`, immediately followed by `globalToast(..., { record: false })` at `:35`. Grep for `globalToast(` with an `error:` option: **only** `errorReporter.ts:35`. There is no other call site that passes a structured error, and no caller of `recordError` that also emits a default-`record` toast. Both directions verified.

Note (not a defect, but a volume change Phase 4 must expect): every one of the ~130 remaining `globalToast` call sites — including `success`/`info` toasts — now writes an entry by default. The notification list is no longer an error list. That is the stated design; it just means `MAX_ENTRIES=100` will churn far faster than the old error-only log did.

---

## (c) Collapsing semantics

**Medium — the flood scenario the design cites is not reachable, so the collapse window is untested against real traffic.**
`notifications.svelte.ts:70-77` and the phase file both justify `COLLAPSE_WINDOW_MS` with "`dataPoller` retries every 5s while toasts expire in 4-9s". Grep: `src/lib/dataPoller.ts` contains **no** `globalToast` and no `reportError` call. Its SSE `onerror` (`dataPoller.ts:171-183`) closes the stream and reconnects silently; the malformed-frame handlers `catch {}` with a comment and emit nothing. The poller cannot flood the list. The rationale comment is factually wrong about this codebase and should be corrected or dropped — leaving it invites the next reader to trust an unverified premise.

**Key stability — good for the path that matters.** `recordError` (`:326`) builds the title as `` `${action}: ${error.code}` `` — an error *code*, not `errorMessage()`. Codes are a closed set (`errors.ts:178-189`), so the collapse key is perfectly stable across retries; a repeating failure collapses correctly. Entries pushed from `globalToast` (`globalToast.ts:126`) use the full redacted user text, which for varying detail (paths, ids, ports) will **not** collapse — but those are user-initiated one-shots, not retry loops, so the exposure is small.

**Medium — `recordError` titles are unreadable in the notification centre.** The same `${action}: ${error.code}` produces `"Stop container: command_failed"`. `ErrorDetailPanel` never renders `entry.title` (it uses `errorTitle(entry.error)`), so this was invisible before; Phase 4's list will render `entry.title` verbatim and show raw snake_case codes to the user. Either store `errorMessage(err)` as the title and keep `error.code` in the collapse key, or have the Phase 4 renderer special-case `entry.error`.

**Medium — timestamp bump breaks list ordering.** Collapsing sets `existing.timestamp = Date.now()` (`:152`) but does **not** move the entry in the array. The list is array-ordered (newest-first by `unshift`), so a collapsed entry keeps its old position while displaying a newer time than everything above it. `ErrorDetailPanel.svelte:68` already renders `formatTime(entry.timestamp)`, so this is a visible regression today, not just a Phase 4 risk. Either re-position on collapse, or keep `firstSeen` for ordering/display and `lastSeen` separately for the window.

On starvation: a hot repeat refreshing `timestamp` means the entry never rolls over into a new one — count grows unbounded on a single entry. That is the intended anti-flood behaviour, not a defect, but it does mean the user loses the first-occurrence time entirely.

---

## (d) `trim()` correctness

**Medium — running jobs are hoisted to the top on unrelated pushes.** `:107` rebuilds as `[...running, ...rest].slice(0, MAX_ENTRIES)`. Ordering elsewhere is newest-first by `unshift` (`:161`), and `ErrorDetailPanel` renders array order directly (`:62`). Once the list crosses 100 entries, *every subsequent push* re-runs this and permanently pins running jobs above newer entries — the list silently changes sort order mid-session, under the user's cursor. If pinning running jobs is wanted, do it in the renderer, not by mutating stored order.

**Low — the "never evict a running job" invariant inverts at scale.** If `running.length >= MAX_ENTRIES`, `slice(0, 100)` keeps only running jobs and discards the entry that was just pushed — new notifications vanish with no signal. 100 concurrent transfers is implausible, but the code claims an invariant it does not hold; a guard (`if (running.length >= MAX_ENTRIES)` → keep running, drop nothing else) or a comment bounding the assumption would be honest.

---

## (e) `reconcileJobs` vs `TransferSnapshot`

**High — a failed transfer is reported to the user as a success.** `src-tauri/src/transfer_registry.rs:40` sets `RETENTION = 60s` and `prune` (`:113-115`) drops *terminal* entries past that cutoff. `reconcileJobs` (`:277-282`) settles anything the backend no longer lists as `status = "success"`. So: transfer fails → app is backgrounded / SSE is down > 60s → reconcile on reconnect → the entry that actually failed is shown to the user as **succeeded**, with the failure text never delivered. The user believes a `docker save` produced a file that does not exist. The comment at `:242-243` ("has finished and been forgotten") conflates *finished* with *succeeded* — absence carries no outcome. Settle these as a distinct terminal status (e.g. `"unknown"` / "outcome not recorded") rather than asserting success. The test at `notifications.test.ts:125-132` pins the wrong behaviour, so it will need updating with the fix.

**Medium — reconcile clobbers `cancelling`.** `:272`: `if (status !== "running" || isRunning(entry)) entry.status = status;`. `isRunning` returns true for `"cancelling"` (`:94`), so when the user has just clicked cancel and the backend snapshot still says `running` (which it will, for the window between the cancel request and the process dying), the entry flips back to `"running"`. The cancel button un-presses itself. Exclude `cancelling` from the overwrite: only take a snapshot `running` when the entry is literally `running`.

**Medium — reconcile-driven terminal transitions never become unread.** `settleJob` sets `read = false` and bumps `timestamp` (`:230-231`). The reconcile path (`:272`, `:279-280`) sets `status` without touching `read`. A transfer that completes while the SSE channel was down produces **no badge**, which defeats the purpose of the reconcile pass for exactly the case it was written for.

**Low — falsy guard on `totalEstimate`.** `:194` and `:267` use `if (update.totalEstimate)` / `if (snapshot.totalEstimate)`. `TransferSnapshot.totalEstimate` is `number | null`, so `0` is silently discarded and an estimate can never be cleared once set. Use `!== null` / `!== undefined`.

---

## (f) Error-log UI regressions

**Cannot render an entry without `.error` — verified safe.** `errorEntries()` (`:342-346`) narrows with a type predicate on `e.error !== undefined`, and `ErrorDetailPanel.svelte:17` consumes only that. `svelte-check` passes with zero errors, which would not hold if `entry.error` were optional at `:66/:71/:77/:81/:96`.

**Medium — `clearErrorLog` now deletes failed transfers from the notification centre.** `:348-352` filters on `e.error === undefined`. `settleJob(..., "error", { error })` (`:234`) attaches `error` to a **job** entry, so a failed transfer both appears in the error panel and is destroyed when the user presses "Clear" in that panel. Cross-surface side effect: clearing the error view silently removes items from a different list. Scope the filter to `e.kind === "message"` (or add an explicit `source` discriminator). Symmetrically, `errorEntries()` will surface failed transfers in the error panel with no `action` and a job title — probably acceptable, but it is a behaviour change from "all entries had `.error` and came from `reportError`" and is not covered by a test.

`clearErrorLog` does **not** wipe running transfers (a running job never carries `.error`), and `notifications.test.ts:179-186` pins that. Good.

`formatEntryForClipboard` (`:368-383`) handles both shapes correctly — the `else` branch (`:377-380`) covers the no-error entry via `title`/`detail`. Only the error branch is tested (`:188-195`); the job/message branch is not.

---

## (g) Test quality

The 16 tests assert behaviour, not implementation — each one pins a failure mode named in the red-team findings (eviction of a running job, unknown-id settle, collapse surviving toast TTL, terminal-wins-over-stale). `notifications.test.ts:56-67` correctly uses fake timers to prove the window outlives a toast. No phantom tests.

Gaps, in priority order:

1. No test for the `record: false` contract — the phase file's own success criterion #1 ("one error through `errorReporter` yields exactly one entry") is **not** covered. This is the "silent bug that breaks nothing" the risk section flags; it needs a test that calls `reportError` and asserts `entries.length === 1`.
2. `notifications.test.ts:125-132` pins the settle-as-success behaviour that finding (e) says is wrong.
3. No test for `markJobCancelling` followed by `reconcileJobs` (the clobber in (e)).
4. No test for the no-error branch of `formatEntryForClipboard`.
5. `notifications.test.ts:99-105` proves the running job survives trim but does not assert list ordering, so the reorder in (d) is invisible to the suite.

---

## Recommended actions

1. **(High)** `reconcileJobs` — do not settle a forgotten job as `success`; use a non-committal terminal status. Update the test at `:125-132`.
2. **(Medium)** Exclude `cancelling` from the snapshot-`running` overwrite at `:272`.
3. **(Medium)** Set `read = false` on reconcile-driven terminal transitions.
4. **(Medium)** Scope `clearErrorLog` to message entries so it stops deleting failed transfers.
5. **(Medium)** Fix collapse ordering: re-position on collapse, or separate `firstSeen` from `lastSeen`.
6. **(Medium)** Stop mutating list order in `trim`; pin running jobs in the renderer instead.
7. **(Medium)** `recordError` title should be human-readable (`errorMessage`), with the code kept only in the collapse key.
8. **(Low)** `totalEstimate` falsy guards → explicit null checks.
9. **(Docs)** Correct the `unreadCount` "derived" comment and the `dataPoller` flood rationale — both state things the code does not do.
10. **(Tests)** Add the `reportError` single-entry test that success criterion #1 requires.

## Plan status

Success criteria met: #2 (collapse works), #3 (running job survives cap), #4 (unknown id creates entry), #5 (`ErrorDetailPanel` works), #6 (`pnpm test` green, no `store/errorLog` import remains — grep confirms zero). Criterion #1 is implemented but **unproven by test**.

## Unresolved questions

1. Should a transfer that failed and aged out of the 60s backend retention be shown as failed-with-unknown-detail, or as an explicitly unknown outcome? This is a product call.
2. Should `ErrorDetailPanel` continue to show failed transfers, or should the error panel stay message-only now that Phase 4 gives transfers their own surface?
