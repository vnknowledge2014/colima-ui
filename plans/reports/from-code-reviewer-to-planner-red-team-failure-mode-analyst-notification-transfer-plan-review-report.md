# Red Team Review — Failure Mode Analyst
Plan: `plans/260812-1247-notification-center-background-transfers/`
Role: Flow Tracer. Reviewed 2026-08-12 against `dev` working tree.

---

## Finding 1: Lost-event window between `jobId` returning and the store registering the job

- **Severity:** Critical
- **Location:** Phase 2, "Thiết kế" (bullet 2) + Phase 1, Implementation Step 5 (`settleJob` on unknown `jobId` = silent no-op)
- **Flaw:** The plan makes the subscription app-global (good) but keeps *registration* of the job entry on the **await-resolution** of `transferApi.*`. Events emitted by Rust between `spawn_job` firing and the JS promise resolving hit a store with no matching entry. Phase 1 explicitly specifies that path as a no-op that does not throw — so the event is swallowed with zero trace.
- **Failure scenario:** `start_image_load(None, path)` → `spawn_job` → `tokio::task::spawn_blocking`. If the runtime binary is missing or the image ref is bad, `stream_to_file`'s `spawn` fails and `transfer.failed` is emitted within microseconds — before the IPC/HTTP response is even serialized back to the webview. The dialog then pushes a `status: "running"` entry for a job that already died. That entry never settles: it shows a spinner and a Cancel button forever, and Cancel returns `false` because the job was never registered. In browser mode this is not even a race but two independent transports (HTTP response vs. a separate `EventSource`), with no ordering guarantee at all.
- **Evidence:**
  - `src-tauri/src/commands/file_transfer.rs:224` — `tokio::task::spawn_blocking` starts before `Ok(TransferStarted)` is returned at `:373` / `:406`.
  - `src-tauri/src/streaming_cmd.rs:262-269` — spawn failure returns `Err` immediately, which becomes `transfer.failed` at `file_transfer.rs:299-308`.
  - `src/lib/transferEvents.ts:61-78` — browser mode opens a *separate* `EventSource`; the fetch response and the SSE frame have no mutual ordering.
  - `src-tauri/src/routes/file_transfer.rs:17-38` — HTTP route returns `TransferStarted` on a different connection than SSE.
  - Today this is masked because `TransferDialog.svelte:71` subscribes at component init and drops non-matching events by design (`:73`, `:79`, `:90`); the new design removes the only place that used to hold the modal open.
- **Suggested fix:** Reserve the entry **before** the call, not after. Have the dialog `pushNotification({kind:"job", jobId: null, status:"pending"})`, then bind the returned `jobId` on resolution. Additionally add a bounded pending-event buffer in the store: events for an unknown `jobId` are parked for ~5s and replayed when that `jobId` registers. Make `settleJob`-on-unknown-id increment a counter that tests assert on, instead of a silent no-op.

---

## Finding 2: No recovery path for jobs that outlive the webview — orphaned, invisible, uncancellable

- **Severity:** Critical
- **Location:** Plan-level "Kiến trúc"; Phase 1 Non-functional ("Không persist ra disk"); Phase 2 Part A
- **Flaw:** The plan moves the *only* UI that tracks a running job into an in-memory JS store, while the job itself lives in a process-global Rust registry. There is no command or route to enumerate active jobs, and `kill_all_streams` is wired only to `ExitRequested` — not to a webview reload, not to a dev-server HMR reload, not to a crashed renderer.
- **Failure scenario:** A 40 GB `docker save` is running. The user hits Cmd-R (or the webview crashes and Tauri reloads it, or `pnpm dev` HMR reloads the module graph). The Svelte store is reconstructed empty. The `spawn_blocking` thread keeps running, keeps writing to the destination file, keeps publishing `transfer.progress` into the void. The user has no entry, no progress, no Cancel button, and no way to recover the `jobId` — it is a monotonic in-process counter with no listing API. The only way to stop it is to quit the app. Under the old design the modal disappearing was at least a visible signal; now the whole feature is "background", so an orphan is indistinguishable from normal operation.
- **Evidence:**
  - `src-tauri/src/lib.rs:362-368` — `kill_all_streams()` fires only on `tauri::RunEvent::ExitRequested`.
  - `src-tauri/src/streaming_cmd.rs:188` `active_stream_count()` — referenced only from `streaming_cmd.rs:634`, `:676` and `file_transfer.rs:674`, `:685` (tests/shutdown). No `#[tauri::command]` and no route exposes it; `src-tauri/src/routes/file_transfer.rs` has save/load/cp/cancel only.
  - `src-tauri/src/commands/file_transfer.rs:32-36` — `JOB_SEQ` is an in-process `AtomicU64`; ids are not recoverable from outside.
- **Suggested fix:** Add `list_transfers()` (command + `GET /api/transfers`) returning `{jobId, kind, startedAt, bytes, destLabel}` from the streaming registry, and have `App.svelte` rehydrate the store from it on mount. This is a Phase 2 blocker, not a nice-to-have — without it "background" means "unrecoverable".

---

## Finding 3: SSE broadcast lag silently drops `transfer.done`, stranding entries as permanently "running"

- **Severity:** High
- **Location:** Phase 2, "Thiết kế" (single app-level `subscribeTransfer`); Phase 3 Success Criteria ("Job đang chạy nhìn thấy được từ mọi page")
- **Flaw:** The plan treats the SSE stream as a reliable delivery channel for terminal events. It is explicitly documented as the opposite. `transferEvents.ts` subscribes to three event names and ignores `stream-lagged`, so a dropped `transfer.done` produces no error, no retry, and no reconciliation — just an entry that spins forever.
- **Failure scenario:** Browser mode, backgrounded tab. Two transfers run concurrently at 5 progress events/sec each (`POLL_INTERVAL = 200ms`), sharing a 64-slot broadcast channel with the metrics collector and docker-events publishers. The consumer falls behind, `BroadcastStreamRecvError::Lagged(n)` fires, the skipped window contains `transfer.done`. The Notification Center shows a running job with a Cancel button; pressing Cancel calls `cancel_stream` on a job that already unregistered and returns `false`. The entry is now immortal and, per Phase 3, "Clear all" refuses to remove running jobs — so the user cannot even dismiss it.
- **Evidence:**
  - `src-tauri/src/sse.rs:6-10` — "This is a display channel, not a durable one… A client slower than the producer" is dropped.
  - `src-tauri/src/sse.rs:47` — `broadcast::channel(64)`.
  - `src-tauri/src/streaming_cmd.rs:33` — `POLL_INTERVAL = 200ms` per job.
  - `src-tauri/src/routes/misc.rs:60`, `:89` — lag surfaces as `stream-lagged`; `src/lib/transferEvents.ts:24` subscribes only to `transfer.progress|done|failed`, so the gap is invisible to this consumer.
  - `src-tauri/src/streaming_cmd.rs:282` — `unregister(job_id)` runs before the done event, so a later `cancel_stream` returns `false`.
- **Suggested fix:** Handle `stream-lagged` in `transferEvents.ts` by triggering a reconcile against the `list_transfers()` endpoint from Finding 2. Add a client-side watchdog: a running entry with no progress event for N× `POLL_INTERVAL` re-queries job state rather than assuming liveness. Allow "Clear all" to force-remove an entry that the backend no longer knows about.

---

## Finding 4: Cancel has a registration window where it silently does nothing; Phase 3's optimistic "cancelling" state never resolves

- **Severity:** High
- **Location:** Phase 3, Implementation Step 5 ("đặt trạng thái optimistic sang 'cancelling' nhưng chỉ chuyển sang `cancelled` khi `transfer.done` với `cancelled: true` về")
- **Flaw:** The plan builds an optimistic state whose only exit is a `transfer.done{cancelled:true}` event, and discards the boolean that `cancel_transfer` actually returns. Registration in the streaming registry happens *after* `File::create` and *after* `spawn` — a real window during which `cancel_stream` returns `false` and does nothing at all.
- **Failure scenario:** Background transfers make the Cancel button reachable within one frame of Start (that is the whole point of Phase 2). User clicks Start, then immediately Cancel. `cancel_stream` finds no registry entry, returns `false`, no signal is sent. The job proceeds to completion and eventually emits `transfer.done{cancelled:false}`. Per the plan's rule ("backend là nguồn sự thật", only `cancelled:true` exits the state) the entry either sticks in "cancelling" or flips to `success` for a transfer the user explicitly aborted — and a multi-GB file the user did not want is now on disk.
- **Evidence:**
  - `src-tauri/src/streaming_cmd.rs:259-279` — `File::create` → `spawn` → `drain_stderr` → *then* `register(job_id, child)`.
  - `src-tauri/src/streaming_cmd.rs:155-165` — `cancel_stream` looks the entry up and returns false when absent.
  - `src-tauri/src/commands/file_transfer.rs:699-701` — test `cancelling_an_unknown_job_reports_false` confirms the silent-false contract.
  - `src-tauri/src/commands/file_transfer.rs:530-532` — `cancel_transfer` returns that bool; nothing in Phase 3 consumes it.
- **Suggested fix:** Register the job in the streaming registry **before** spawning (reserve the id, attach the child after), so a cancel arriving early is honoured. In the UI, branch on the returned bool: `false` + still-running entry → retry once, then surface "could not cancel". Define an explicit timeout out of the `cancelling` state.

---

## Finding 5: Phase 2's copy-out redesign writes an unbounded TAR stream to disk with no free-space guard

- **Severity:** High
- **Location:** Phase 2, "B3. Copy-out ra thư mục", the `JobPaths { sink: Some(dest_tar) }` change
- **Flaw:** The plan changes copy-out from `docker cp <id>:<path> <dest>` to a stdout TAR stream redirected into a sink file, explicitly to enable copying whole **directories**. It carries over none of the disk-space protection that the existing sink path (export) has, and it has no size estimate to check against.
- **Failure scenario:** User copies `/var/lib/postgresql/data` (or `/`) out of a container to `~/Downloads`. `docker cp <id>:/ -` streams the entire container filesystem. `stream_to_file` happily writes until the host disk is full. On macOS a full boot volume is a system-level failure, not an app-level one. When the write finally errors, the partial file is removed — so the user loses the disk *and* gets nothing. The existing export path refuses this case; the new copy-out path does not.
- **Evidence:**
  - `src-tauri/src/commands/file_transfer.rs:348-357` — the free-space refusal exists only in `start_image_save`, gated on `estimate_image_bytes`.
  - `src-tauri/src/commands/file_transfer.rs:169-182` — `estimate_image_bytes` is image-specific; there is no equivalent for a container path, and the plan proposes none.
  - `src-tauri/src/commands/file_transfer.rs:483-522` — `start_copy_from_container` currently passes `total_estimate: None` and performs no space check.
  - `src-tauri/src/streaming_cmd.rs:281` — `wait_with_progress` writes until the child exits; nothing bounds the sink.
- **Suggested fix:** Before spawning copy-out, get a size via `docker exec <id> du -sb <path>` (best-effort, with a timeout) and apply the same `available_bytes` refusal. Failing that, add a running ceiling in the progress tick: if `file_len(path)` exceeds available free space minus a safety margin, cancel the job and report it. Do not ship an unbounded host-disk writer.

---

## Finding 6: Cancelling a transfer deletes the user's pre-existing destination file — and the plan makes it an acceptance criterion

- **Severity:** High
- **Location:** Phase 2, "Lợi ích" bullet 3 (`streaming_cmd` đã tự dọn `ToFile` sink khi huỷ); Plan-level Acceptance criteria ("Huỷ giữa chừng không để lại file rác ở thư mục đích")
- **Flaw:** The plan's claim that `streaming_cmd` already cleans up a `ToFile` sink is **VERIFIED TRUE** — but "cleans up" means `remove_file` on a path that `File::create` may have truncated over a file the user already owned. Routing copy-out through the sink extends this data-destroying behaviour to a second code path, and the acceptance criterion as written cannot distinguish "removed our partial output" from "deleted the user's file".
- **Failure scenario:** User has `~/Downloads/backup.tar` from last week. They export again to the same name and tick "overwrite". `File::create` truncates `backup.tar` to zero at spawn. Thirty seconds in they realise they picked the wrong image and hit Cancel. `stream_to_file` removes the path. Last week's archive is gone, with no warning and no undo. Identical for the new copy-out path.
- **Evidence:**
  - `src-tauri/src/streaming_cmd.rs:259-260` — `File::create(path)` truncates the destination up front.
  - `src-tauri/src/streaming_cmd.rs:284-293` — cancel removes `path` unconditionally.
  - `src-tauri/src/streaming_cmd.rs:296-299` — non-zero exit removes it too.
  - `src-tauri/src/commands/file_transfer.rs:118-123` — `resolve_destination` permits this whenever `overwrite` is true.
- **Suggested fix:** Write to a sibling temp path (`<dest>.partial`) and `rename` into place only on success. Cancel/failure then removes only bytes this app produced. Restate the acceptance criterion as "cancel removes the partial output and leaves any pre-existing file at the destination untouched", and test exactly that.

---

## Finding 7: `MAX_ENTRIES = 100` FIFO can evict a *running* job entry, contradicting Phase 3's own invariant

- **Severity:** Medium
- **Location:** Phase 1, Implementation Step 1 (`MAX_ENTRIES = 100`) vs. Phase 3, Requirements ("'Clear all' chỉ xoá entry đã kết thúc; job đang chạy không bị xoá khỏi tầm mắt")
- **Flaw:** Phase 1 wires every `globalToast()` call into the same bounded FIFO that holds running jobs, and specifies no exclusion for `status: "running"`. Phase 3 then declares running jobs protected from manual clearing. The two rules disagree: the store will silently do what the UI forbids.
- **Failure scenario:** A long `docker save` runs. Meanwhile something generates a burst of *distinct* error messages — the collapse mitigation in Phase 1 Step 4 keys on `toastKey(type, text)`, so 100 errors that differ by container/image name (the common case when a runtime goes down mid-poll) do not collapse. The running job's entry is pushed past index 100 and dropped. Its `cancel` closure goes with it, `settleJob` becomes a no-op, and the job is now in exactly the orphan state of Finding 2 — without any restart involved.
- **Evidence:**
  - `src/lib/globalToast.ts:59`, `:102`, `:105-113` — collapsing is keyed on redacted text, so distinct texts never collapse.
  - 131 `globalToast(` call sites across 29 files (`grep -rn "globalToast(" src/`) — the plan's "~100" is accurate; all of them now feed the store.
  - `src/store/errorLog.svelte.ts:24`, `:40-41` — the cited precedent truncates by hard `length =` assignment with no per-entry exemption.
- **Suggested fix:** Evict oldest **settled** entry only; if all entries are running, refuse the push (or drop the incoming message entry). State the invariant once, in Phase 1, and assert it in `notifications.test.ts`.

---

## Finding 8: Phase 5's read-tracking scheme cannot work — UUID watermark and lexicographic version gates

- **Severity:** Medium
- **Location:** Phase 5, "Chống hiện lại" and Implementation Step 3
- **Flaw:** Two independent defects in the same phase. (a) The plan stores `lastSeenAnnouncementId` as a watermark, but the schema in the same file defines `id uuid primary key default gen_random_uuid()` — random UUIDs have no ordering, so a watermark is meaningless; only the parallel "danh sách id đã đọc" does any work, and it grows without bound. (b) `min_version`/`max_version` are `text` and the plan says only "so với version app hiện tại", with no semver comparison specified.
- **Failure scenario:** (a) The watermark logic prunes or short-circuits the read-id list based on a random UUID comparison and either re-shows every announcement on each launch (spam, the exact thing the exception to the no-persist rule was made to prevent) or permanently suppresses new ones. (b) A `critical` security advisory published with `min_version: "1.9.0"` is hidden from every user on 1.10.x, because `"1.10.0" < "1.9.0"` under string comparison. The failure is silent by construction: Phase 5 Step 6 specifies "Lỗi mạng → nuốt im lặng", so nobody ever learns the advisory did not land.
- **Evidence:**
  - Phase 5 schema, `id uuid primary key default gen_random_uuid()` vs. Phase 5 "Chống hiện lại", `lastSeenAnnouncementId`.
  - `src/lib/settingsStore.svelte.ts:61` — `getAppSetting(key, defaultValue: string): string` is a flat string KV; an unbounded read-id list has no home here and no pruning story.
  - Phase 5 "Related Code Files" cites `src/pages/settings/PrivacySettings.svelte` — `src/pages/settings/` contains only `Account.svelte`, `ColimaConfig.svelte`, `Subscription.svelte`. The target does not exist.
- **Suggested fix:** Drop the watermark; keep a bounded set of read ids (cap + prune by `published_at`), or add a monotonic `seq bigserial` column if a watermark is genuinely wanted. Specify a semver comparator explicitly and unit-test `1.10.0 >= 1.9.0`. Fix the settings file path.

---

## Finding 9: Phase 4 requests notification permission precisely when the user cannot grant it, then never asks again

- **Severity:** Medium
- **Location:** Phase 4, Requirements ("Chỉ bắn khi cửa sổ không focus") + Risk Assessment ("chỉ xin lần đầu có job kết thúc, không xin lúc mở app") + Success Criteria ("Từ chối quyền → … không lặp lại lời xin quyền")
- **Flaw:** These three rules compose into a permanently disabled feature. The permission request is deferred to the first job completion, and job completions only reach `osNotify` when the window is unfocused — so the very first permission prompt is raised while the app is in the background.
- **Failure scenario:** macOS shows the authorization alert attached to a backgrounded app. The user is in another window and never sees it; it times out or is dismissed as noise. The plan treats a non-grant as "từ chối quyền → không lặp lại lời xin quyền", so the feature is off forever, with the Settings toggle still reading "on". The user's mental model ("I turned OS notifications on") and reality diverge with no diagnostic anywhere.
- **Evidence:**
  - Phase 4 Implementation Steps 2-3 chain `isRunningInTauri()` → permission check → `document.hasFocus()` → send, all inside the `settleJob` path (Step 5).
  - `src-tauri/src/commands/file_transfer.rs:53-58` — `emit` is the only producer of terminal events; there is no other trigger that could run while focused.
  - `src-tauri/Cargo.toml` — `tauri-plugin-notification` is absent, as the plan states; permission plumbing in `src-tauri/capabilities/default.json` is new surface with no rollback note.
- **Suggested fix:** Request permission from the Settings toggle (a foreground, user-initiated action), not from the completion path. Persist a tri-state (`unknown | granted | denied`) and reflect `denied` in the Settings UI with a link to system settings. Keep the "don't nag" rule, but only after a prompt the user actually saw.

---

### Verification Results

**VERIFIED — Phase 2 claim: "`streaming_cmd` đã tự dọn `ToFile` sink khi huỷ"**
Traced `spawn_job` (`file_transfer.rs:271`) → `run_streaming` (`streaming_cmd.rs:101-113`) → `stream_to_file` (`:252`). `remove_file` is called on spawn failure (`:267`), on register failure (`:276`), on cancel (`:288`), and on non-zero exit (`:297`). Removing the manual `cleanup_on_abort` for the sink variant is safe. **Caveat in Finding 6:** cleanup is unconditional on a path `File::create` may have truncated over a user file.

**VERIFIED — Phase 2 claim: "`sink` đo progress bằng độ lớn file đang ghi — số thật"**
Traced the `emit_progress` closure at `file_transfer.rs:232-242`: it captures `measured = watch_path.clone()` (`:231`). With `sink: Some(_)` and `watch: None`, `measured` is `None`, so the closure passes its `bytes` argument through unchanged. That argument originates at `streaming_cmd.rs:281` — `wait_with_progress(&handle, || on_progress(file_len(path)))` — i.e. the sink file's length. The claim holds. Terminal `bytes` at `file_transfer.rs:289-292` falls back to `outcome.bytes` = `file_len(path)` (`:301`). No defect here.

**VERIFIED — Phase 2 claim: "ở browser mode mỗi dialog mở một `EventSource` mới"**
`TransferDialog.svelte:71` calls `subscribeTransfer` at component init; `transferEvents.ts:68` constructs `new EventSource(url)` per call; `onDestroy(unsubscribe)` at `TransferDialog.svelte:97`. One connection per dialog instance, as claimed.

**VERIFIED — Phase 2 claim B3: `watch` on a directory measures the dirent, and `cleanup_on_abort` fails on a directory**
`file_transfer.rs:511-514` sets both to `dest`; `:234` and `:290` use `fs::metadata(p).len()`; `:279` / `:301` use `std::fs::remove_file` with the result discarded via `let _ =`. Confirmed.

**FAILED — Plan-level claim: "Backend transfer đã chạy nền rồi … Đây là redesign frontend, không phải viết lại backend."**
Actual flow: the backend runs jobs in the background but exposes **no way to observe or address them except the fire-and-forget event stream**. There is no job-listing command or route (`active_stream_count` at `streaming_cmd.rs:188` is used only by tests and shutdown), the id space is a process-local `AtomicU64` (`file_transfer.rs:32`), the event channel is documented as lossy (`sse.rs:6-10`), and cancellation is unavailable during the pre-`register` window (`streaming_cmd.rs:259-279`). The moment the frontend stops holding a modal open for the job's whole lifetime, these become user-visible failure modes. Phases 2-3 therefore require backend work (a listing/reconcile endpoint, earlier registration) that the plan does not budget for.

**FAILED — Phase 3 claim: "Sidebar.svelte:62 đã có comment cảnh báo nav list vượt viewport"**
The comment exists but at `src/components/Sidebar.svelte:61-63`, and the `nav-panel-hint` mechanism cited in Phase 3's Architecture lives at `:133`, `:139`, `:332`. Minor, but the plan's other line citations should be re-checked before implementation.

**Additional path errors found:** `src/pages/settings/PrivacySettings.svelte` (Phase 5) does not exist; `src/pages/settings/` holds only `Account.svelte`, `ColimaConfig.svelte`, `Subscription.svelte`. Phase 4 Step 4's "store settings hiện có" is `src/lib/settingsStore.svelte.ts` (a flat string KV, `getAppSetting` at `:61`), not a store under `src/store/`. `App.svelte` mounts `ToastContainer` **twice** (`:203` and `:244`) — Phase 3's "mount panel cạnh `ToastContainer`" must pick one deliberately or the panel will be double-mounted.

---

### Blocking summary

Findings 1, 2 must be resolved in the plan before Phase 2 starts: both are consequences of removing the modal that currently serves as the job's only lifetime anchor. Findings 3, 4, 5, 6 are High and should each get an explicit design decision in their phase file rather than being discovered during implementation. Findings 7, 8, 9 are Medium and internal to their phases.
