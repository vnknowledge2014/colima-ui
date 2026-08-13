# Phase 4 review — transfers to background

Scope: `src/lib/transferEvents.ts`, `src/lib/transferNotifications.ts`, `src/App.svelte`,
`src/components/transfer/TransferDialog.svelte`, `src/store/notifications.svelte.ts`,
tests, locales. Verification run: `pnpm vitest run` 213/213 pass (18 files),
`pnpm check` 0 errors 0 warnings, `pnpm lint` clean. No new type/lint/build errors.

## High

**H1 — Stale reconcile marks a live transfer "ended" (race).**
`reconcileTransfers()` (`src/lib/transferNotifications.ts:32`) is fire-and-forget with
no in-flight guard and no ordering guard. `reconcileJobs()`
(`src/store/notifications.svelte.ts:314-320`) sweeps *every* running entry not present
in the snapshot and sets `status = "ended"`. A `GET /api/transfers` issued before a job
was registered, but resolving after `startJob()` ran, flips that brand-new running
transfer to "ended" while it is still copying. Window = one round-trip, and `onDesync`
fires on every connect, so an SSE reconnect concurrent with a dialog start hits it.
Two overlapping reconciles interleave the same way (older response wins because it
lands last). Fix: single-flight guard plus a monotonic request seq, and/or skip the
sweep for entries created after the request was issued.

**H2 — Duplicate entries for one job when failure precedes start.**
The phase brief calls this out as handled, but it is not. `settleJob` with an unknown
id pushes a new `kind: "job"` entry (`notifications.svelte.ts:245-255`). `startJob`
then calls `pushNotification` which explicitly **never collapses jobs**
(`notifications.svelte.ts:164`), so the same `jobId` gets a second entry. `findJob`
(`:210`) returns only the first match, so no later reconcile merges them: the user is
left with one "error" entry and one entry that later becomes "ended"/terminal for a
single transfer. Fix: in `pushNotification`/`startJob`, if `input.job.jobId` already
exists, update that entry instead of unshifting.

**H3 — Host paths in notification title/detail, contradicting the redaction contract.**
`TransferDialog.svelte:73` sets `jobLabel = tarPath` for `import`, and `:171` sets
`detail = ${destDir}/${fileName}` for export/copy-out. `TransferSnapshot.targetLabel`
documents "never a host path … keeps the list needing no redaction pass"
(`src/lib/api/transfer.ts:44-50`), and `formatEntryForClipboard`
(`notifications.svelte.ts:406-411`) states its fields are "already redacted upstream,
safe to paste into an issue". They are not: username/home dir now reaches the clipboard
path. Either redact these locally or drop the doc claim — currently the code and the
contract disagree.

## Medium

**M1 — Reconcile amplification on a flapping connection.**
`onopen` resets `retryDelay` to 2 s **and** fires `onDesync`
(`transferEvents.ts:118-125`). A server/proxy that accepts then immediately drops the
stream produces open→error→2 s→open forever: ~30 `GET /api/transfers` per minute
indefinitely, because backoff never grows past a successful open. Reset `retryDelay`
only after the connection has stayed up (e.g. first message, or a short timer), and
de-dupe `onDesync` with a minimum interval.

**M2 — `refreshImages()` now runs at start, not at completion.**
`src/pages/Images.svelte:400-405` refreshes the image list inside `onClose`, which now
fires the instant the job is handed off. An import's new images are therefore not in
the list the refresh returns; the user waits for the background poller. The comment
there ("refreshing after both is simpler") is now stale — it describes the old
dialog-stays-open behaviour. Hook the refresh to job settle instead.
`src/pages/Containers.svelte:782` only nulls state — no regression there.

**M3 — Subscription can leak past teardown in `App.svelte`.**
`cleanupTransfers` is assigned at `src/App.svelte:110`, after `await
loadAllSettings()` (`:105`). If `onDestroy` (`:127`) runs before that await resolves
(HMR remount, fast teardown), `cleanupTransfers` is still undefined and the
subscription created afterwards is never torn down — a second EventSource on remount.
Same pre-existing shape as `cleanupPoller`. Guard with a `destroyed` flag checked
before assigning, or call cleanup immediately if already destroyed.

**M4 — New i18n keys missing from all four locales.**
`transfer.start_background`, `transfer.copy_out_is_archive` are absent from
`src/locales/{en,ja,vi,zh}.json` (`transfer.job_unknown` is not referenced anywhere in
`src/` — dead key in the brief). `t()`'s `default` param (`src/lib/i18n.svelte.ts:48`)
makes absence non-breaking, but ja/vi/zh users get English strings for the primary
button. Orphaned after this change: `transfer.start`, `transfer.finished`,
`transfer.cancelled`, `transfer.estimated` — zero references outside locales.
`transfer.cancel_transfer` still has one reference, so keep it.

## Clean / no finding

- **(a) Reconnect loop.** No double-connection: `source.close()` in `onerror`
  (`transferEvents.ts:128`) aborts the browser's own retry, so exactly one pending
  reconnect exists. `cancelled` is re-checked after both awaits (`:99`), so a teardown
  during `getApiToken()`/`resolveApiBase()` cannot leave a live EventSource;
  `retryTimeout` is cleared on unsubscribe. `retryDelay` placement is the only issue
  (M1). Permanent failures (bad token) retry forever at 30 s — acceptable.
- **(g) Test coverage.** Dropped progress/cancel dialog tests are legitimately gone
  with the feature; equivalent behaviour is covered by
  `transferNotifications.test.ts` (progress routing, cancelled vs done, failure shape,
  reconcile-on-gap, adopt-unknown-job, swallow-backend-error). Not covered, and
  matching the findings above: duplicate-jobId settle-then-start (H2) and stale-snapshot
  sweep (H1).

## Recommended order

1. H2 (dedupe by `jobId` in `pushNotification`) — smallest fix, largest visible defect.
2. H1 (single-flight + seq guard on reconcile).
3. H3 (decide: redact or amend the contract comment).
4. M1, M3, M2, M4.
