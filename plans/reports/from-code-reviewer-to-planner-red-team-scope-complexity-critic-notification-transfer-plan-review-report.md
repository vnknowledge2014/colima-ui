# Red-Team Plan Review — Scope & Complexity Critic

Plan: `plans/260812-1247-notification-center-background-transfers/`
Role: Contract Verifier + YAGNI enforcer. Evidence-based, no praise.

---

## Finding 1: Phase 2's "always tar" copy-out silently invalidates the existing Docker integration test, and the plan never says so

- **Severity:** Critical
- **Location:** Phase 2, "Phần B — B3. Copy-out ra thư mục" + "Related Code Files"
- **Flaw:** The plan lists `src-tauri/tests/file_transfer_against_docker.rs` as "Modify: **thêm case** cho B1-B3" (add cases). It does not acknowledge that the existing round-trip test asserts a byte-identical copy-out — an assertion that `docker cp <id>:<path> -` into a tar sink makes impossible.
- **Failure scenario:** Implementer follows Phase 2 steps 1-3, runs `cargo test`, and the pre-existing test fails in two independent ways: (a) `file_size(&returned) == 100 * 1024 * 1024` never becomes true (tar adds a 512-byte header plus padding to a 512-byte boundary), so `wait_until` burns its full 300-second timeout and panics; (b) even if the size check were relaxed, `sha256(&returned)` no longer equals the source hash. Worse, if step 2 ("nối validate vào ... `start_copy_from_container`") applies `require_archive_name` as Phase 2 line 116 requires, the file name `"returned.bin"` is rejected before spawn and `.expect("copy out should start")` panics immediately. Under `catch_unwind`, the test container cleanup path is the only thing that saves the environment. The plan's acceptance criterion "`cargo test` xanh" is unreachable without a rewrite the plan never budgets.
- **Evidence:**
  - `src-tauri/tests/file_transfer_against_docker.rs:199-213` — `start_copy_from_container(None, container_id.clone(), "/tmp/payload.bin", dir.str(), "returned.bin", false)`, then `wait_until(... file_size(&returned) == 100 * 1024 * 1024)`, then `sha256(&returned)`
  - `phase-02-transfer-jobs-to-background.md:127` — "Modify: `src-tauri/tests/file_transfer_against_docker.rs` — thêm case cho B1-B3"
  - `phase-02-transfer-jobs-to-background.md:116` — "`file_name` của copy-out cũng đi qua `require_archive_name`"
- **Suggested fix:** Add an explicit step: "rewrite the copy-out round-trip assertion to untar and compare the inner payload; rename `returned.bin` → `returned.tar`". If that rewrite is not wanted, drop the always-tar decision (see Finding 2).

---

## Finding 2: "Always tar copy-out" is a public-contract break bought to avoid a five-line fix

- **Severity:** Critical
- **Location:** Phase 2, "Phần B — B3", and `plan.md:47` ("Quyết định đã chốt")
- **Flaw:** Two real bugs are identified (progress watches a dirent size; `remove_file` fails on a directory). Both are local. The plan's remedy is to change the output contract of a Tauri command, an HTTP endpoint, a documented API row, and every UI path that uses them — for all users, including the majority single-file case. That is gold plating dressed as a bugfix. The plan itself admits the cost at line 107-109 ("copy một file đơn lẻ ra cũng cho `.tar`, người dùng phải `tar -xf`") and at line 164-165 ("Thay đổi phá vỡ kỳ vọng cũ").
- **Failure scenario:** A user copies `/etc/nginx/nginx.conf` out of a container to inspect it. Today they get `nginx.conf`. After Phase 2 they get `nginx.conf.tar`, must run `tar -xf`, and the extracted file lands with the in-container path prefix. The two bugs that motivated the change (dir progress, dir cleanup) never applied to this case at all — `fs::metadata(file).len()` is correct for files and `remove_file` works on files. The regression is 100% of single-file users paying for a directory-only defect.
- **Evidence:**
  - `src-tauri/src/commands/file_transfer.rs:483-521` — `start_copy_from_container` with `JobPaths { watch: Some(dest.clone()), cleanup_on_abort: Some(dest), .. }`; the existing doc comment already explains why there is no sink
  - `src-tauri/src/commands/file_transfer.rs:194-206` — `JobPaths` fields; `cleanup_on_abort` doc already states `ToFile` sinks are cleaned by `streaming_cmd`
  - `docs/api.md:178` — documented as "Copy a container **file** out"
  - Full consumer list in Verification Results below (10 sites)
- **Suggested fix:** MVP cut. Keep the current destination semantics. Fix `cleanup_on_abort` to fall back to `remove_dir_all` when `remove_file` returns `ErrorKind::IsADirectory`, and make the `watch` progress reporter walk a directory total (or emit indeterminate). If tar-on-directory is still wanted, gate it: only when the caller opts in via a new `asArchive` flag, defaulting off. Ship the breaking version behind a decision the user actually confirmed, not one asserted in a "Quyết định đã chốt" table.

---

## Finding 3: Phase 2 fuses two independent workstreams and serializes a Rust-only fix behind a Svelte store

- **Severity:** High
- **Location:** Phase 2 frontmatter `dependencies: [1]`, and "Overview" ("Hai việc trong cùng một phase vì chúng đụng đúng những file đó")
- **Flaw:** The stated justification is false for Part B. `require_archive_name` and `sniff_tar` touch only `src-tauri/src/commands/file_transfer.rs`; they share zero files with the notification store. The only genuine overlap is `TransferDialog.svelte` (dialog filters vs. removing the progress block). So a P1 correctness fix (import validation, export extension) is blocked on the entire notification store landing first.
- **Failure scenario:** Phase 1 stalls on the `globalToast` integration (Finding 4). The `.tar` bugs — which are real user-visible defects today — sit unshipped behind it. Meanwhile the single fused branch touches `file_transfer.rs`, `TransferDialog.svelte`, `App.svelte`, `transferEvents.ts`, two test files, and the Docker integration test; a review of that diff cannot separate "did the tar sniff regress anything" from "did the store wiring regress anything".
- **Evidence:**
  - `phase-02-transfer-jobs-to-background.md:6` — `dependencies: [1]`
  - `phase-02-transfer-jobs-to-background.md:119-128` — Related Code Files spans 6 files across both stacks
  - Part B touches `src-tauri/src/commands/file_transfer.rs` only; `src/store/notifications.svelte.ts` appears nowhere in Part B's steps 1-4
- **Suggested fix:** Split into `Phase 2a: tar contract (Rust + dialog filters), dependencies: []` and `Phase 2b: background jobs, dependencies: [1]`. 2a ships independently and immediately.

---

## Finding 4: Phase 1 does not unify toasts — it creates a third parallel system, and deliberately drops errors on the floor

- **Severity:** High
- **Location:** Phase 1, "Architecture" (relationship with `globalToast`) and Implementation Step 4
- **Flaw:** `plan.md:45` claims "Hai hệ thống song song sẽ lệch nhau", then Phase 1 explicitly keeps both: `globalToast.ts` retains `_active`, `_timers`, `MAX_ACTIVE_TOASTS`, `toastKey` collapsing, and `_errorListeners`; the store adds a second independent lifecycle with its own `MAX_ENTRIES` and unread state. That is the exact divergence the plan says it prevents. It is also a *third* system: `src/store/errorLog.svelte.ts` already keeps a capped, session-scoped, non-persisted error history rendered by `ErrorDetailPanel.svelte` — the plan never mentions reconciling it, so an error will exist in three stores at once.
- **Failure scenario:** Step 4 says a collapsed repeat pushes no entry. The `dataPoller` fails, one toast appears and collapses 40 repeats, and the toast expires after 9 s. The user, away from the screen, opens the Notification Center — the whole outage is represented by one entry timestamped at the first failure with no indication it recurred, because `count` lives on the toast object and is never mirrored to the entry. A persistent notification list whose main job is catching what you missed is the one place collapsing should *not* discard information.
- **Evidence:**
  - `src/lib/globalToast.ts:44-58` (`_timers`, `_active`, `MAX_ACTIVE_TOASTS = 3`), `:104-113` (collapse branch returns early), `:173-181` (`_errorListeners` for AI diagnostics)
  - `src/store/errorLog.svelte.ts:1-40` — "Session error log ... Nothing is persisted", `MAX_ENTRIES = 50`, `errorLogState.isOpen` — a store with the same shape Phase 1 proposes to build
  - `src/lib/errorReporter.ts:11,32` — `recordError(err, ctx.action)` is the funnel; consumers: `ErrorDetailPanel.svelte`, `App.svelte`
  - `phase-01-notification-store-core.md:89-91` — collapse branch does not push an entry
- **Suggested fix:** Either (a) mirror `count` onto the existing entry in the collapse branch, or (b) drop the store entirely for `kind: "message"` and reuse `errorLog.svelte.ts` — it already is the durable-history store. Building `notifications.svelte.ts` as a near-clone of `errorLog.svelte.ts` is parallel reimplementation of an existing utility.

---

## Finding 5: Phase 5 solves a problem nobody stated and lands a DB schema in a repo with no migrations directory

- **Severity:** High
- **Location:** Phase 5 in its entirety; `plan.md` "Overview" states the problem as a blocking modal
- **Flaw:** The plan's own framing is "Cái chặn người dùng nằm ở frontend — `TransferDialog.svelte` giữ modal mở". Phase 5 contributes to none of acceptance criteria 1-5. It adds: a Postgres table, an RLS policy, a migration, an i18n `jsonb` scheme, semver range filtering, a 6-hour poller, persisted read-state (an explicit exception to Phase 1's own no-persist rule, per `phase-05:69-71`), a Settings toggle, and a `docs/telemetry.md` edit. That is a product feature (vendor announcements) smuggled into a UX bugfix plan.
- **Failure scenario:** The plan says to create `supabase/migrations/<ts>_announcements.sql` "theo layout migration mà `plans/260811-1930-...` thiết lập". That directory does not exist — `supabase/` contains only `config.toml`, `functions/`, `snippets/`. The implementer must invent a migration convention, apply it manually against the live project as owner (step 1), and there is no rollback path in the phase. A hand-applied schema change in a plan whose stated goal is "stop the modal blocking" is unreviewable scope drift.
- **Evidence:**
  - `ls supabase/` → `config.toml`, `functions`, `snippets` (no `migrations/`)
  - `phase-05-cloud-broadcast-supabase.md:75-77` — depends on another plan's not-yet-created layout
  - `plan.md:89-101` — acceptance criteria 1-5 are all local transfer/UI; only criterion 6 ("Không có đường dẫn ... rời máy") touches cloud, and it is satisfied trivially by *not* doing Phase 5
- **Suggested fix:** Remove Phase 5 from this plan and file it as its own plan. It has different reviewers (schema, privacy), a different risk profile, and no dependency on the transfer work.

---

## Finding 6: Phases 4 and 5 both build on a settings store and settings pages that do not exist

- **Severity:** High
- **Location:** Phase 4 Implementation Step 4 / Related Code Files; Phase 5 "Chống hiện lại" and Related Code Files
- **Flaw:** Phase 4 says "Thêm setting `notifications.osEnabled` vào **store settings hiện có**". Phase 5 says "lưu `lastSeenAnnouncementId` + danh sách id đã đọc trong **settings store (persist)**". There is no settings store. There is no persistence layer beyond a single `localStorage` key. Phase 5 also names `src/pages/settings/PrivacySettings.svelte`, which does not exist.
- **Failure scenario:** The implementer opens `src/store/` looking for the settings store, finds `account-dialog`, `ai`, `capabilities`, `confirm`, `errorLog`, `k8s`, `upgrade`, and none of them persist anything. They now must design a persisted-settings abstraction — schema, versioning, migration on key rename — inside a phase estimated "S" (Phase 4) that budgets none of it. Effort estimates are wrong by a phase.
- **Evidence:**
  - `ls src/store/` → `account-dialog.svelte.ts`, `ai.svelte.ts`, `capabilities.svelte.ts`, `confirm.svelte.ts`, `errorLog.svelte.ts`, `k8s.svelte.ts`, `upgrade.svelte.ts` — no settings store
  - Only persistence write in the app: `src/App.svelte:80` — `localStorage.setItem("colima_active_page", ...)`
  - `ls src/pages/settings/` → `Account.svelte`, `ColimaConfig.svelte`, `Subscription.svelte` — no `PrivacySettings.svelte`
  - `phase-04-os-native-notifications.md:63` and `phase-05-cloud-broadcast-supabase.md:69-71,82-83`
- **Suggested fix:** Either add an explicit "create persisted settings store" step with its own effort estimate, or drop both toggles and hard-code defaults. Also note Phase 5's client-side tier filter has the same problem: no frontend store exposes a tier. `grep -rn "tier" src/store src/lib` returns only `upgrade.svelte.ts:54` (checkout arg) and `src/lib/api/subscription.ts:11,22,82` — nothing resolved and readable at announcement-filter time.

---

## Finding 7: `NotificationEntry` is over-specified — a closure in reactive state and a third `kind` with no consumer

- **Severity:** Medium
- **Location:** Phase 1, "Architecture" (the `NotificationEntry` interface)
- **Flaw:** Two YAGNI violations in one type. (a) `job.cancel: () => void` stores a function in `$state`. The entry already carries `jobId`, and cancelling is `transferApi.cancel(jobId)` — one line, already implemented. (b) `kind: "announcement"` exists only for Phase 5, a P3 phase this review recommends cutting; it forces `NotificationItem.svelte` to carry a dead branch (`phase-03:46`).
- **Failure scenario:** The closure captures the `TransferDialog` component scope that created it (`phase-02:31`), keeping the unmounted dialog's reactive graph alive for the entry's lifetime — up to `MAX_ENTRIES = 100` entries. It also makes the entry non-serializable, which directly blocks the persistence Phase 5 wants to add to the same store, and makes any structural-clone/JSON assertion in `notifications.test.ts` throw. Compare `errorLog.svelte.ts:15-21`, where `ErrorLogEntry` is pure data.
- **Evidence:**
  - `phase-01-notification-store-core.md:49-55` — `job: { jobId, bytes, totalEstimate, cancel: () => void }`
  - `src/lib/api/transfer.ts:91-94` — `cancel: (jobId) => call<boolean>("cancel_transfer", { jobId }, ...)` already exists
  - `src/components/transfer/TransferDialog.svelte:160-167` — existing `cancel()` needs only `jobId`
  - `src/store/errorLog.svelte.ts:15-21` — precedent: data-only entries
- **Suggested fix:** Drop `cancel` from the entry; have `NotificationItem` call `transferApi.cancel(entry.job.jobId)` directly. Drop `"announcement"` until Phase 5 exists.

---

## Finding 8: The progress-bar CSS duplication that Phase 3 promises to fix does not exist

- **Severity:** Medium
- **Location:** Phase 3, "Architecture" ("Chuyển sang `src/styles/components.css` thay vì copy") and Success Criteria ("Không có CSS progress bar bị nhân bản")
- **Flaw:** `.bar`, `.fill`, and `.fill.indeterminate` are defined in exactly one file. There is no duplication to eliminate, and Phase 2 deletes the only existing consumer ("Bỏ toàn bộ khối progress bar ... khỏi dialog", `phase-02:41`). So after Phase 2 there is one consumer (the panel), and Phase 3 proposes promoting component-scoped CSS to a global stylesheet to serve it. This is DRY applied before duplication exists — the plan's own priority order is YAGNI > KISS > DRY.
- **Failure scenario:** Implementation Step 6 says "cập nhật **cả hai nơi** dùng". There is no second place: Phase 2 removed the dialog's bar. The implementer either re-adds a dialog progress bar to satisfy the step, or globalizes three selectors with names as generic as `.bar` and `.fill` into `components.css`, where they will collide with any future component using those class names. The success criterion "no duplicated progress-bar CSS" is vacuously true before any work is done.
- **Evidence:**
  - `grep -n "\.bar\b|\.fill\b|indeterminate" src/styles/components.css src/components/transfer/TransferDialog.svelte` → matches only in `TransferDialog.svelte:277,279,373,380,386`; zero matches in `src/styles/components.css`
  - `phase-02-transfer-jobs-to-background.md:41` — dialog progress block is deleted
  - `phase-03-notification-center-ui.md:50-53,77,86`
- **Suggested fix:** Delete this from Phase 3. Write the bar's CSS scoped inside `NotificationItem.svelte`. Extract to `components.css` only when a second component actually needs it.

---

## Finding 9: Call-site count in the plan is understated by ~60%

- **Severity:** Medium
- **Location:** `plan.md:45` and Phase 1 Non-functional requirements ("~100 call site")
- **Flaw:** The figure used to justify "don't rewrite `globalToast`" is wrong. It is 161 references across 32 files. The argument still holds directionally, but a plan that quotes a verified-sounding number it did not verify invites the same error elsewhere (see Findings 1, 6, 8, where unverified claims are load-bearing).
- **Failure scenario:** Phase 1 Success Criterion "Không file nào ngoài `globalToast.ts` phải sửa" is asserted against a number nobody counted. If even one of those 32 files calls `globalToast` before `ToastContainer` mounts (a scenario `globalToast.ts:60-67` documents as a past bug), the new `pushNotification` call inherits the same ordering hazard, and the criterion gives false confidence.
- **Evidence:** `grep -rn "globalToast" src | wc -l` → 161; `grep -rln "globalToast" src | wc -l` → 32. `src/lib/globalToast.ts:60-67` documents the prior mount-ordering bug.
- **Suggested fix:** Correct the number and add a Phase 1 test for `pushNotification` invoked before any subscriber exists.

---

### Verification Results

**Interface: `copy_from_container` / `start_copy_from_container` / `POST /api/containers/cp/from` — 10 consumers, all found; Phase 2 lists 4.**

| # | Site | Role | Listed in Phase 2? |
|---|---|---|---|
| 1 | `src-tauri/src/commands/file_transfer.rs:465` | `#[tauri::command] copy_from_container` | yes (file listed) |
| 2 | `src-tauri/src/commands/file_transfer.rs:483` | `start_copy_from_container` impl | yes |
| 3 | `src-tauri/src/lib.rs:208` | Tauri invoke handler registration | **no** |
| 4 | `src-tauri/src/routes/file_transfer.rs:55-58` | `api_copy_from_container` HTTP handler | **no** |
| 5 | `src-tauri/src/routes/payloads.rs:177` | `CopyFromContainerBody` request struct | **no** |
| 6 | `src-tauri/src/api_server.rs:134` | route table `/api/containers/cp/from` | **no** |
| 7 | `src-tauri/tests/file_transfer_against_docker.rs:15,199` | round-trip integration test (breaks — Finding 1) | listed, understated |
| 8 | `src/lib/api/transfer.ts:74-88` | `copyFromContainer` TS wrapper | **no** |
| 9 | `src/components/transfer/TransferDialog.svelte:142` | only UI caller | yes |
| 10 | `src/components/transfer/TransferDialog.test.ts:19` | mocked in dialog test | yes |
| 11 | `docs/api.md:178` | documented as "Copy a container **file** out" | Phase 6 only |

Sites 3-6 and 8 need no signature change *if* the parameter list is unchanged, but 5, 6 and 11 all describe the response contract, and 11 is factually wrong the moment Phase 2 lands. Phase 2's Related Code Files omits `docs/api.md` entirely, deferring it to Phase 6 — which is `dependencies: [1,2,3,4,5]`, i.e. docs stay wrong until the cloud phase ships.

**Interface: `TransferDialog.svelte` (progress/subscribe removal, button relabel) — 4 consumers.**
- `src/pages/Containers.svelte:18,736`
- `src/pages/Images.svelte:12,395`
- `src/components/transfer/TransferDialog.test.ts` (listed)
- Both page-level mounts pass an `onClose` that Phase 2 now fires on start rather than on completion — neither page is listed in Phase 2's Related Code Files, and neither has been checked for state it resets on close (e.g. selected images).

**Interface: `subscribeTransfer` — 4 sites; Phase 2 lists 3.**
- `src/lib/transferEvents.ts:32` (definition, listed)
- `src/components/transfer/TransferDialog.svelte:15,71` (listed — to be removed)
- `src/components/transfer/TransferDialog.test.ts:11,23,36` — the test **mocks** `../../lib/transferEvents` and drives handlers directly; once the dialog stops subscribing, this mock is dead and the test's progress assertions must move to the store test. Phase 2 line 128 says only "dialog đóng sau start".
- `src/lib/api/transfer.ts:7` (doc comment reference)
- `src/App.svelte` — new subscriber (listed)

**Interface: `globalToast` — 161 references / 32 files** (plan says ~100). Public signature unchanged, so no call-site edits needed; the risk is behavioral, not structural (Finding 4).

**Upstream/config checks:**
- `src-tauri/Cargo.toml:23,41,42,45,53` — plugins present: opener, http, updater, deep-link, dialog. Phase 4's claim that `tauri-plugin-notification` is absent is **correct**.
- `supabase/` has no `migrations/` directory — Phase 5's migration path is fictional.
- `src/pages/settings/` contains `Account.svelte`, `ColimaConfig.svelte`, `Subscription.svelte` — Phase 5's `PrivacySettings.svelte` does not exist.
- `src/store/` contains no settings store — Phases 4 and 5 both assume one.
- `src-tauri/src/subscription/` contains only `cache.rs`, `mod.rs`; no frontend store exposes a resolved tier, so Phase 5's client-side `audience` filter has no data source.

**Verified-correct plan claims (stated for adjudication, not praise):**
- `TransferDialog.svelte:114` — `dialog.save({ defaultPath: fileName || "images.tar" })` has no `filters`. B1 is real.
- `file_transfer.rs:99-124` — `resolve_destination` does `require_plain` + `assert_path_within` + exists check, no extension check. B1 is real.
- `file_transfer.rs:483-521` — `watch`/`cleanup_on_abort` on a path `docker cp` may create as a directory. B3's two bugs are real (the proposed remedy is not proportionate — Finding 2).
- `Sidebar.svelte:62-64` — the 15th-nav-item comment exists; `nav-panel-hint` at `:133,332` exists.

---

### Unresolved Questions

1. Was "copy-out always produces `.tar`" actually accepted by the user, or asserted by the planner? `plan.md:47` files it under "Quyết định đã chốt" with no source. Reversing a user decision and inventing one are different failures; this needs an answer before Phase 2 starts.
2. If Phase 5 is cut, does the `notifications` store still justify existing separately from `errorLog.svelte.ts`, or should running jobs be added to the existing store?
3. Who owns applying the Supabase migration, and what is the rollback if `announcements` ships with the wrong `audience` values?
