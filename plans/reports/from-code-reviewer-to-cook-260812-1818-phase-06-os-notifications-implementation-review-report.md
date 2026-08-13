# Phase 6 — OS-native notifications: implementation review

Date: 2026-08-12 · Branch `dev` · Reviewer: code-reviewer

## Verification (run, not assumed)

| Gate | Result |
|---|---|
| `pnpm vitest run` | exit 0 |
| `pnpm check` (svelte-check) | 369 files, 0 errors, 0 warnings |
| `pnpm lint` (eslint --max-warnings=0) | exit 0 |
| `cargo clippy --all-targets --all-features -- -D warnings` | exit 0 |

No new lint/type/build error introduced.

## (a) Privacy constraint — CLEAN

No route found by which a host path reaches an OS notification.

- `src/lib/transferNotifications.ts:137-140` passes `t(...)` + `label` on failure and
  deliberately **not** `e.error`; the raw runtime text goes only to the in-app entry
  (`detail: e.error`, line 133). This is the constraint the phase demanded and it holds.
- `jobLabel()` (`transferNotifications.ts:36-38`) returns the store entry `title`. Title
  sources, traced for all four modes at `src/components/transfer/TransferDialog.svelte:85-92`:
  - `export` → `images.join(", ")` — image refs.
  - `import` → `baseName(tarPath)` (`TransferDialog.svelte:73-75`) — last path segment only.
    The folder is dropped. This is the mode most at risk and it is handled.
  - `copy-in` / `copy-out` → `containerPath` — a path *inside* the container, not the host.
    `hostPath` and `destDir` are never used as the title; `detail` gets `fileName` only.
- Reconcile route: `reconcileJobs` titles from `snapshot.targetLabel`
  (`src/store/notifications.svelte.ts:311`), which the Rust side sets to
  `images.join(", ")`, `path.file_name()`, or `container_path`
  (`src-tauri/src/commands/file_transfer.rs:467,507,564,634`) — never `dest`/host dir.
  `src-tauri/src/transfer_registry.rs:61` documents this invariant.
- Settle-before-start: `settleJob` invents `title: info.title ?? jobId`
  (`notifications.svelte.ts:278`), but `jobLabel()` runs *before* settle and returns
  `undefined` for an unknown job, so the notification body is omitted rather than being a
  raw job id. Correct.

Only residual: `reconcileJobs` falls back to `snapshot.jobId` when `targetLabel` is empty
(`notifications.svelte.ts:311`), so a later failure on such an entry produces the body
`"save-17"`. Cosmetic, not a leak.

## (b) Permission flow — one Medium

**Medium — a concurrent second transfer is silently dropped during the permission prompt.**
`src/lib/osNotify.ts:56-66`. Two transfers finishing while the system dialog is open:
A sets `permissionAsked = true` (line 63) then awaits `requestPermission`; B resumes from
`isPermissionGranted`, sees `permissionAsked === true` at line 62 and returns — even if A's
grant then succeeds. No double prompt (the check and set at lines 62-63 have no `await`
between them, so the interleave is impossible), which was the stated macOS risk, but B's
notification is lost rather than deferred. Fix: keep the in-flight `requestPermission`
promise in a module variable and have concurrent callers await it instead of bailing.

Denial semantics are correct: `permissionAsked` is module-level and not persisted, so a
refusal is not re-asked in the session and *is* retried after restart — matches the phase
requirement (line 26-28 of the phase file).

**Low — a revoked permission stays cached as granted.** Line 56 only re-reads while
`permissionGranted` is false, so revocation mid-session leaves it `true` and
`sendNotification` becomes a silent no-op until restart. The caching direction is the right
trade-off (it re-detects a *grant* made in system settings, which is the recoverable
direction); the un-recoverable direction is not user-visible beyond "no notifications".
Acceptable as-is; the comment at lines 57-58 should say the cache is one-way.

## (c) Focus detection — Low, informational

`document.hasFocus()` (`osNotify.ts:51`) is false when the window is minimised, on another
Space, or when another app is frontmost — i.e. it covers the cases the phase cares about.
The gap is narrow: ColimaUI frontmost but its window fully occluded by another window of the
same app, or off-screen on a disconnected display. `getCurrentWindow().isFocused()` returns
the same answer as `hasFocus()` in those cases; only `isVisible()` + `isMinimized()` would
add anything, and both require new `core:window:allow-is-*` capabilities for a marginal
case. Not worth it. Keep `hasFocus()`.

Note the guard `typeof document !== "undefined" && document.hasFocus()` fails *open* in a
non-DOM context (no document → notification is sent). Irrelevant in practice since
`isRunningInTauri()` already returned.

## (d) Capabilities — CLEAN, minimal

`src-tauri/capabilities/default.json:15-17` grants exactly the three commands the code calls
(`isPermissionGranted`, `requestPermission`, `sendNotification`). `notification:default` —
which additionally allows `register-action-types`, `get-pending`, `cancel`, `get-active`,
`remove-active`, `permission-state`, `batch` — was correctly **not** used. Nothing granted
that is unused, and nothing granted that lets the frontend read back, cancel, or act on
notifications.

One caveat worth recording rather than fixing: `allow-notify` covers the whole
`NotificationOptions` shape, which includes `schedule`, `sound`, `attachments` and `extra`.
That is inherent to the plugin's permission granularity, not a mistake here, and only
matters if untrusted content could execute in the webview.

## (e) Wiring — CLEAN

- `jobLabel()` before `settleJob` in both handlers (`transferNotifications.ts:111,128`) is
  necessary and correct for the unknown-job case; see (a).
- `void osNotify(...)` is fire-and-forget, and `osNotify` cannot reject: everything after
  the three synchronous guards is inside `try/catch` (lines 53-72), and the guards
  (`isRunningInTauri`, `getAppSetting`, `document.hasFocus`) cannot throw. No unhandled
  rejection escapes. `mod.sendNotification` returns void in plugin v2, so the un-awaited
  call at line 68 is not a floating promise either.
- Cancelled suppression matches the rule: `onDone` fires `osNotify` only under
  `if (!e.cancelled)` (lines 120-125); `onFailed` always fires, which is correct since a
  failure is not a user action.

## (f) Settings — one Medium

**Medium — the toggle can show the wrong value if the page mounts before settings load.**
`src/components/settings/NotificationSettings.svelte:19` initialises
`let enabled = $state(getAppSetting(...) !== "false")` **once**, at component creation.
`settingsState` is populated asynchronously by `loadAllSettings()`
(`src/lib/settingsStore.svelte.ts:34`, `appState.isSettingsLoaded` set at line 40). A user
who reaches Settings before that resolves sees the checkbox "on" even though the stored value
is `"false"`, and the next toggle writes back a state they never saw. `osNotifyEnabled()`
reads at call time so behaviour is right — only the UI lies. Fix: gate the section on
`appState.isSettingsLoaded`, or re-sync via `$effect` on `settingsState[OS_NOTIFY_SETTING]`.

Not awaiting `setAppSetting` (line 24) is safe: it mutates `settingsState` synchronously
first and swallows its own persistence failure with `console.error`
(`settingsStore.svelte.ts:48,56-58`), so awaiting would change nothing observable. It does
mean a failed write is invisible to the user — pre-existing repo behaviour, not this phase's.

`icon="Bell"` is real: exported at `src/components/Icons.svelte:12`.

## (g) Tests and locales

Tests assert behaviour, not just execution: each of the 7 cases in
`src/lib/osNotify.test.ts` asserts on `sendNotification`/`requestPermission` call counts and
payload, including the negative cases (focused, disabled, browser, denied) and the
throwing-plugin case (line 99-105). No phantom tests.

Gap: no test covers the concurrent-prompt interleave described in (b), and none asserts
that the failure path never passes `e.error` — the privacy invariant is enforced only by
`transferNotifications.ts:137`. A test in `transferNotifications.test.ts` asserting the
`osNotify` body for `onFailed` is not the error text would make that regression-proof.
Recommended, not blocking.

Locale keys: `notifications.*` are absent from `src/locales/en.json` (0 matches), as
expected for Phase 7. `t()` returns `params.default` when the lookup misses
(`src/lib/i18n.svelte.ts:46-48`), so the English strings render, not raw keys. Harmless.

## Recommended actions

1. (Medium) Share the in-flight `requestPermission` promise in `osNotify.ts` so a second
   transfer settling during the prompt is not dropped.
2. (Medium) Make the `NotificationSettings` checkbox reflect `settingsState` after async load.
3. (Low) Add a `transferNotifications` test pinning "the OS body is the label, never the error".
4. (Low) Correct the comment at `osNotify.ts:57-58` — the cache is one-way; revocation is not
   detected until restart.

None of these block landing.

## Unresolved questions

- Should a notification suppressed by the permission-prompt race be retried after the grant,
  or is dropping it acceptable given the in-app entry already exists?
