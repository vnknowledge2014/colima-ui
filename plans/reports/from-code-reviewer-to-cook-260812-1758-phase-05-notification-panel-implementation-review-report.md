# Phase 5 — Notification Center UI: implementation review

Reviewer: code-reviewer · 2026-08-12 · branch `dev`

## Verification (run)

| Command | Result |
|---|---|
| `pnpm vitest run` | 19 files, 226 tests passed |
| `pnpm check` | 365 files, 0 errors, 0 warnings |
| `pnpm lint` | clean (`--max-warnings=0`) |

No new type/lint/build errors. Everything below is behaviour the gates do not cover.

---

## Critical

### C1 — `alreadyFinished` is reported to the user as "success", and it is provably wrong

`src/components/notifications/NotificationItem.svelte:62-63` maps `alreadyFinished` → `settleJob(job.jobId, "success")`.

The backend returns that variant for *any* terminal state:

`src-tauri/src/transfer_registry.rs:280`
```rust
Some(entry) if entry.snapshot.status.is_terminal() => CancelOutcome::AlreadyFinished,
```
`is_terminal()` covers `Success`, `Failed` **and** `Cancelled`. So: an export fails, the user presses Cancel a moment later, and the row flips to a green ✓ "success". The panel then reports a failed export as a completed one.

This is exactly the guess Phase 3 introduced the `ended` status to prevent — the store says so in its own words at `src/store/notifications.svelte.ts:28-35` and again at `:337-340` ("guessing 'success' would report a failed export as a finished one"). The UI reintroduces the guess the store spent a status variant avoiding.

Worse, the correct answer is free: `RETENTION` is 60s (`transfer_registry.rs:40`) and `list()` returns terminal entries until pruned (`:206-214`), so the job that just answered `AlreadyFinished` is *guaranteed* still present in the snapshot list. `void reconcileTransfers()` resolves it to the true status via `statusFromSnapshot`.

Fix: treat `alreadyFinished` the same as `unknownJob` — `void reconcileTransfers()`. If a synchronous fallback is wanted, `entry.status = "ended"`, never `"success"`.

The test at `NotificationPanel.test.ts:81-94` asserts `.toBe("success")`, i.e. it locks the defect in. That assertion must change with the fix.

---

## High

### H1 — A failed cancel request strands the row at "Cancelling…" permanently, with the button disabled

`NotificationItem.svelte:57` calls `markJobCancelling` *before* the request. On throw, the catch at `:68-70` only toasts. The row keeps `status === "cancelling"`, the button stays `disabled` (`:124`), and nothing else will ever move it: `reconcileJobs` explicitly refuses to downgrade `cancelling` back to `running` (`notifications.svelte.ts:325-327`), and `clearFinished` refuses to remove it (`:390`). The user is left with an undismissable, uncancellable row for the rest of the session.

Same outcome via a quieter path: on `unknownJob`, `reconcileTransfers()` swallows every error (`transferNotifications.ts:58-61`) — backend unreachable means the row silently stays at `cancelling` forever.

Phase 5's own risk section asked for this ("Trạng thái 'cancelling' treo … đặt timeout đối soát qua `transferApi.list()` thay vì tin tưởng vô hạn", phase-05:101-103). No timeout exists anywhere in the diff.

Fix: revert to the prior status on throw, and/or add a bounded reconcile retry for entries sitting at `cancelling` past N seconds.

### H2 — `aria-live="polite"` on the whole list turns a transfer into a screen-reader flood

`NotificationPanel.svelte:113` puts the live region on the `<ul>` that contains every row, including the progress `<p class="meta">` at `NotificationItem.svelte:112-115`. `updateJob` mutates `entry.job.bytes` (`notifications.svelte.ts:245`) on every progress event — roughly 5/s per transfer. Each mutation is a text change inside the live region, so AT queues an announcement each time. Polite only means "do not interrupt"; it does not mean "coalesce". The queue grows faster than speech drains and the panel becomes unusable with a screen reader while any transfer runs.

Fix: remove `aria-live` from the list. Add a single visually-hidden `aria-live="polite"` region that emits one short string only when an entry is *added* or reaches a terminal status. Give the bar `role="progressbar"` + `aria-valuenow`/`aria-valuemin`/`aria-valuemax` (or `aria-valuetext` when indeterminate) — progressbar values are read on demand, not streamed.

### H3 — Focus never enters the panel, so the focus trap does not trap

There is no `.focus()` anywhere in `NotificationPanel.svelte`; `panel` (`:41`) is bound only for the `querySelectorAll` in `trapFocus`. When the panel opens, focus is still on the bell button in the sidebar — outside the panel, behind the backdrop.

`trapFocus` (`:49-64`) only acts when `document.activeElement` is the panel's first or last focusable. The bell is neither, so the first Tab falls through to whatever follows the bell in DOM order: the sidebar nav, underneath an opaque overlay. The trap only starts working after the user has blindly tabbed into the panel by luck.

There is also no focus restore: on Escape or Clear-then-close the panel unmounts and focus drops to `<body>`, losing the keyboard user's position.

Fix: `$effect` on `panelOpen` → focus the close button (or the panel with `tabindex="-1"`); store `document.activeElement` on open and restore it on close.

### H4 — Half-modal: focus is confined, but the panel is not a dialog

The container is `<aside aria-label>` (`:85-89`) — a `complementary` landmark, not a dialog. Combined with H3's trap, a screen-reader user gets: keyboard focus confined to a region that the virtual cursor can still browse *out of*, into content that is visually covered and `inert`-less, with no announcement that a modal opened and no documented way out. That mismatch is worse than either choice alone.

Given the backdrop (`:79-83`) is modal in appearance and behaviour (click-outside closes), make it modal in semantics: `role="dialog"` + `aria-modal="true"` + `aria-labelledby` pointing at the `<h2>` (`:91`). Alternatively drop the trap and let it be a genuinely non-modal aside. Do not keep both halves.

---

## Medium

### M1 — Reverse panel coordination is not implemented (success criterion unmet)

Phase 5 success criteria: "Mở panel notification đóng panel AI **và ngược lại**" (phase-05:96). Only one direction exists. `openNotifications()` (`Sidebar.svelte:34-41`) calls `closeAiPanel()`, but the two places that open the AI panel — `Sidebar.svelte:395` (`uiState.aiPanelOpen = !uiState.aiPanelOpen`) and `AiChatPanel.svelte:179` (`showPanel()`, used by the floating bubble) — never call `closeNotificationPanel()`. Open notifications, then click the AI bubble: the AI panel opens *behind* the fixed notification panel and is unreachable.

Note the comments at `store.svelte.ts:100-101` and `Sidebar.svelte:28-32` both assert "both are fixed to the same edge … two Escape handlers listening". Both claims are wrong: `.ai-panel` is `position: relative` in the flex row (`AiChatPanel.svelte:627-636`), and `AiChatPanel` registers no window Escape handler at all. The stated rationale for the coordination does not match the code it justifies.

Fix: add `openAiPanel()` to `store.svelte.ts` that closes the notification panel, and route all three call sites through it.

### M2 — The badge lights up behind the open panel and stays lit

`openNotificationPanel()` marks everything read once, at open (`notifications.svelte.ts:449-452`). Nothing keeps that true afterwards. An entry arriving while the panel is open sets `read: false` (`:165`), so the badge the user just cleared re-appears — counting a row they are currently looking at — and remains until they close and reopen the panel. The `.unread` left rule (`NotificationItem.svelte:154-158`) has the same problem in reverse.

The blanket mark-at-open is otherwise defensible: a running job marked read is not silently lost, because `settleJob` re-sets `read = false` (`:282`) and so does `reconcileJobs` on a terminal transition (`:331`), so a later failure does relight the badge. That part is sound.

Fix: an `$effect` that marks entries read while `panelOpen` is true, matching the pattern `AiChatPanel.svelte:185-187` already uses for its own badge.

### M3 — Display sort fights the store's ordering contract; rows jump on settle

`ordered` (`NotificationPanel.svelte:26-33`) is running-first, then *insertion order* — not timestamp order, despite the comment "Running transfers first, then newest" (`:20-21`).

Two visible consequences:
1. When a job settles, it leaves the running partition and drops to wherever it was inserted — potentially far down the list — while `settleJob` has just set `timestamp = Date.now()` (`:283`) without moving it. The row jumps *and* then displays a newer timestamp than the rows above it.
2. `pushNotification` moves a collapsed repeat to the front (`:189-193`) while `trim()` deliberately preserves position (`:126-138`); the panel's own sort then re-partitions on top of both. Three ordering rules, no single owner.

Fix: sort the non-running partition by `timestamp` descending so the comment and the rendered order agree, and settling only moves a row down by one partition.

### M4 — No progress semantics for assistive tech

The bar is bare `<div>`s (`NotificationItem.svelte:104-111`). Even after fixing H2, there is no `role="progressbar"`, so the only accessible representation of a transfer's progress is the text line inside the (currently) live region. Also, progress is gated on `running` (`:102`), so a finished transfer shows no final byte count at all.

### M5 — Every new user-facing string is English-only

`grep -c notifications src/locales/{en,vi,zh,ja}.json` → 0 in all four. `notifications.title`, `.clear`, `.empty`, `.cancel`, `.cancelling`, `.unread_label` exist only as `default:` fallbacks. `t()` falls back to the default and still interpolates (`i18n.svelte.ts:46-57`), so nothing breaks — but the app ships four locales and this panel is English in all of them, including the aria-label the badge depends on.

### M6 — `unknownJob` → `reconcileTransfers()` can no-op against an in-flight request

`reconcileTransfers` shares a request already in flight (`transferNotifications.ts:51`). If a reconcile started *before* the cancel, its snapshot may still list the job as running, so `reconcileJobs` leaves the row at `cancelling` and no further reconcile is scheduled. Narrow window, but the failure mode is the permanent-`cancelling` row of H1. Same fix (bounded retry) covers it.

---

## Low

- Opening the notification panel does not close `ErrorDetailPanel`. Both can be open at once (a toast's Details while the panel is open), and a single Escape then fires both window handlers (`ErrorDetailPanel.svelte:42`, `NotificationPanel.svelte:66-70`), closing both. The Details button inside the row avoids this by closing the panel first (`NotificationItem.svelte:134-138`); the toast path does not.
- `human()` (`NotificationItem.svelte:73-82`) is a fourth byte-formatter in the codebase pattern; not blocking, but worth checking against an existing helper before it becomes a fifth.

---

## Test quality

Nine tests, mostly behavioural (rendered DOM, roles, real store) — the cancel-targets-the-right-row test (`:66-79`) and the clear-keeps-running test (`:96-107`) are genuinely load-bearing. Gaps:

- `:81-94` asserts the C1 defect as intended behaviour.
- No test for the `unknownJob` branch — the one branch with non-obvious resolution logic.
- No test for a rejected `transferApi.cancel` (H1's stranding path).
- `:127-135` ("clears the badge when opened") never renders the component; it is a store test living in a component file.
- `:137-144` calls `settleJob` and then wipes `entries` — the setup is dead code.
- Nothing covers focus, Tab containment, or badge-while-open.

---

## Clean

Reactivity (item e) is correct: `unreadCount()` reads `entries[].read` through the `$state` proxy inside Sidebar's `$derived` (`Sidebar.svelte:25`), so the badge tracks nested `read` changes; `ordered`/`hasFinished` read `e.status` per element and re-run on status change. Type safety, lint, and the existing suite are unaffected — no regressions found in item (g) beyond the Escape-overlap note above.

---

## Recommended order

1. C1 — stop claiming success (and fix the test that pins it).
2. H1 — no permanently stranded `cancelling` row.
3. H2 — remove the list-level `aria-live`.
4. H3/H4 — move focus in, restore on close, and pick dialog *or* non-modal.
5. M1 — reverse coordination; M2 — read-while-open.
6. M3/M5 — ordering, locale keys.

## Unresolved questions

- Should `alreadyFinished` show `ended` immediately and let reconcile refine it, or wait for reconcile and briefly keep `cancelling`? The former never lies; the latter has one fewer visual state change.
- Is the notification panel intended to be modal? That answer decides H4 in one direction or the other.
