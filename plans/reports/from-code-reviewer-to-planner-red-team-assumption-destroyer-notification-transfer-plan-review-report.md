# Red-Team Plan Review — Assumption Destroyer / Scope Auditor

Plan: `plans/260812-1247-notification-center-background-transfers/`
Reviewer role: Assumption Destroyer + Scope Auditor (state lifetime)
Date: 2026-08-12

---

## Finding 1: `pnpm test` does not exist — every phase's acceptance criteria is unrunnable

- **Severity:** Critical
- **Location:** plan.md "Acceptance criteria"; Phase 1 Success Criteria; Phase 2 step 7; Phase 3 Success Criteria; Phase 6 steps 4 and Success Criteria
- **Flaw:** The plan gates five phases on `pnpm test` being green. `package.json` has no `test` script. `vitest` and `@testing-library/svelte` are devDependencies and `vitest.config.ts` exists, but nothing wires `pnpm test`.
- **Failure scenario:** Implementer finishes Phase 1, runs `pnpm test`, gets `ERR_PNPM_NO_SCRIPT`. Either they invent a script (unplanned scope, unreviewed CI contract) or they mark the criterion "done" without running anything — the plan's only verification gate silently evaporates. Phase 6's "sửa hồi quy thay vì nới lỏng test" becomes unenforceable.
- **Evidence:**
  - `package.json:6-12` — scripts are exactly `dev`, `build`, `check`, `preview`, `tauri`. No `test`.
  - `package.json:40` — `"vitest": "^4.1.10"` present as devDep.
  - `vitest.config.ts:1-17` — config exists with jsdom + svelte plugin.
- **Suggested fix:** Add an explicit Phase 0 step creating `"test": "vitest run"` (and decide on `typecheck`/`check` gating), or change every criterion to `pnpm exec vitest run <path>`. Do not leave "pnpm test xanh" as a criterion that cannot be executed.

---

## Finding 2: `globalToast() → pushNotification()` duplicates `errorLog` state and the anti-flood guard does not hold

- **Severity:** Critical
- **Location:** Phase 1, "Architecture" + "Implementation Steps" 3-4 + "Risk Assessment"
- **Flaw:** Two compounding errors.
  (a) **Duplicate state.** Every error toast already produces a structured session entry via `reportError()` → `recordError()` → `errorLogState.entries`. Phase 1 adds a second, parallel, session-lifetime, non-persisted, MAX_ENTRIES-capped list holding the same `AppError` for the same events. That is exactly "new state duplicating existing state under a different name" — the thing Phase 1 claims to be eliminating ("Hai hệ thống song song sẽ lệch nhau").
  (b) **The flood mitigation is wrong.** Step 4 says the collapse branch will prevent poller floods. Collapsing is keyed on `_active`, and `dismissToast()` deletes the entry from `_active` when the timer fires (4s success / 9s error). So collapse only suppresses repeats *while the toast is still on screen*.
- **Failure scenario:** SSE drops. `dataPoller` falls back to a 5000 ms `setInterval` calling `refreshManual()` + `refetchAllResources()`. Docker is down, so each cycle reports an error. The error toast lives 9 s, so repeats within that window collapse — but the SSE reconnect backoff grows to 30 s (`sseRetryDelay = Math.min(sseRetryDelay * 2, 30000)`), and any error whose text varies (container id, exit code, timestamp in `detail`) produces a *different* `toastKey` and therefore a brand-new entry every 5 s. Within ~8 minutes the 100-entry cap is fully consumed by poller noise, the badge shows 100+, and the running transfer job the whole feature exists for is pushed out of the list. Meanwhile `errorLogState` holds the same 50 errors under a different name.
- **Evidence:**
  - `src/lib/errorReporter.ts:32-33` — `recordError(err, ctx.action); globalToast("error", text, { error: err, hint: ... });` — both, for every error.
  - `src/store/errorLog.svelte.ts:15-43` — `ErrorLogEntry { id, error, action, timestamp }`, `MAX_ENTRIES = 50`, `$state`, no persistence. Near-identical shape and lifetime to the proposed `NotificationEntry`.
  - `src/lib/globalToast.ts:59-61` — `toastKey(type, text)` includes full text.
  - `src/lib/globalToast.ts:71-84` — `dismissToast` deletes from `_active`; `src/lib/globalToast.ts:42` — TTL 9000/4000/5000 ms.
  - `src/lib/dataPoller.ts:168-171` — `setInterval(..., 5000)` fallback; `:176` — backoff to 30000.
  - Call-site volume: 130 `globalToast(` invocations in `src/` (66 `error`, 57 `success`, 3 `info`) — the plan's "~100" undercounts.
- **Suggested fix:** Do not auto-push from `globalToast`. Push notification entries explicitly from the small set of events that deserve session-lifetime history (job start/settle, announcement, and `reportError` — once, replacing `recordError`, not alongside it). If `errorLog` is to be folded into the notification store, say so and delete it; if not, state explicitly why two session error lists must coexist.

---

## Finding 3: Hoisting `subscribeTransfer` to app lifetime removes the only reconnect path in browser mode

- **Severity:** High
- **Location:** Phase 2, "Phần A — Thiết kế" bullet 1; plan.md "Kiến trúc" ("chỉ còn đúng một subscription cho cả phiên"); Phase 2 Success Criteria ("mở dialog 5 lần chỉ tạo một EventSource")
- **Flaw:** `transferEvents.ts` has **no** `onerror` handler and no reconnect/backoff. Today the connection is per-dialog, so a dead `EventSource` is implicitly repaired by closing the dialog and opening a new one. Making the subscription app-lifetime turns a self-healing short-lived connection into a single point of failure with no recovery, and the plan does not add one.
- **Failure scenario:** Browser mode. User starts a 20-minute `docker save` and the dialog closes (new behaviour). Two minutes in, the API server restarts or the network blips. The `EventSource` errors; nothing reconnects. No further `transfer.progress`/`done`/`failed` ever arrives. The Notification Center shows the job pinned at "running" with a stale byte count and a Cancel button, forever, for the rest of the session — and every subsequent transfer in that session is equally invisible. The old design at least recovered on the next dialog open. There is also no timeout/staleness detection in the proposed store, so nothing ever settles the orphan entry.
- **Evidence:**
  - `src/lib/transferEvents.ts:60-79` — browser branch creates `new EventSource(url)` and registers three `addEventListener`s. There is no `source.onerror`, no retry, no backoff. Compare `src/lib/dataPoller.ts:165-177` which *does* have `es.onerror` with `clearInterval`/`setInterval` fallback and exponential `sseRetryDelay`.
  - `src/lib/transferEvents.ts:63-66` — `getApiToken()`/`resolveApiBase()` are awaited before the source is created; at app boot this races with token availability, a risk that did not exist when subscription happened after the user had already navigated and authenticated.
  - Note also the Success Criterion "chỉ tạo một EventSource" is unachievable as literally stated: `dataPoller` holds its own independent `EventSource` (`src/lib/dataPoller.ts:165`), so the app will always have two.
- **Suggested fix:** Phase 2 must add `onerror` + reconnect/backoff to `transferEvents.ts` (or move it onto the `dataPoller` bus, which the file's own doc comment already flags as the correct end state), plus a staleness rule in the store that marks a job `error` when no progress has arrived for N minutes. Restate the criterion as "one transfer EventSource".

---

## Finding 4: Sidebar has no reusable "panel toggle" mechanism — it is one hardcoded `id === "ai-chat"` check

- **Severity:** High
- **Location:** Phase 3, "Architecture" ("Sidebar đã có sẵn khái niệm... Notification dùng đúng cơ chế đó — không phát minh kiểu mở thứ hai") and Implementation Step 2 ("dùng cơ chế panel-toggle đã có")
- **Flaw:** The claimed mechanism is a single inline `@const` string comparison inside the nav-item loop, bound to one specific `uiState` boolean. There is no abstraction to reuse. Meanwhile Phase 3's own Risk Assessment says the bell should live in the sidebar header/footer, **not** in `navGroups` — which is where the "mechanism" lives. The two statements are mutually exclusive.
- **Failure scenario:** Implementer follows Step 2, finds nothing generic, and either (a) adds `notifications` to `navGroups` to reuse the branch — reintroducing exactly the 15th-nav-item overflow the plan cites as its reason for not doing that, or (b) writes a second, ad-hoc toggle in the footer — i.e. "phát minh kiểu mở thứ hai", the thing the plan forbids. Either way the plan's stated design is unimplementable as written, and the effort estimate ("M") does not include the generalization work.
- **Evidence:**
  - `src/components/Sidebar.svelte:306` — `{@const isPanelToggle = item.id === "ai-chat"}` — hardcoded, single value.
  - `src/components/Sidebar.svelte:316-319` — `if (isPanelToggle) { uiState.aiPanelOpen = !uiState.aiPanelOpen } else { uiState.currentPage = item.id }` — the boolean is named for AI specifically.
  - `src/components/Sidebar.svelte:324, 331-333` — `aria-pressed` and `nav-panel-hint` both read `uiState.aiPanelOpen` directly.
  - `src/store.svelte.ts:45-49` — `uiState` has exactly one panel flag, `aiPanelOpen`.
  - `src/components/Sidebar.svelte:61-63` — the 15-item overflow comment the plan cites (plan says line 62; the comment spans 61-63) — accurate, but it argues *against* reusing the nav-item path.
- **Suggested fix:** Add an explicit step: generalize the toggle to a `uiState.openPanel: "ai" | "notifications" | null` (which also delivers Phase 3's own "mở panel này đóng panel kia" mitigation for free) and refactor both the AI item and the new bell onto it. Budget it; it is not free reuse.

---

## Finding 5: Client-side tier filtering assumes a tier that is not reliably readable at poll time

- **Severity:** High
- **Location:** plan.md "Dependencies" ("Phase 5 lọc theo tier phía client, dùng entitlement đã resolve offline (`src-tauri/src/subscription/`)"); Phase 5 "Bất biến" and Implementation Step 3; Success Criteria "`audience: 'pro'` không hiện cho người dùng Free"
- **Flaw:** Three separate unstated dependencies break this.
  1. `proState.subscription` (the only thing carrying a `tier` string) is **explicitly documented as display-only** and is set to `null` on any read failure, by design. Filtering announcements on it means a transient IPC failure silently reclassifies a Pro user as Free.
  2. The durable signal is `proState.paid`, a **boolean** — it cannot distinguish `pro` from `pro_teams`, and the schema's `audience` check constraint is `('free','pro')` with no `pro_teams`, so the schema and the entitlement model already disagree.
  3. In browser mode `getSupabase()` returns `null` (`accountAvailability()` → `"browser-mode"`), so Phase 5 is a complete no-op there — never stated, and it contradicts nothing in the plan only because the plan never mentions browser mode at all.
- **Failure scenario:** A paying customer opens the app. `loadEntitlement()` runs before/concurrently with the announcement poll; the local subscription IPC hiccups, the catch block nulls `proState.subscription` while deliberately preserving `paid`. The announcement filter reads `subscription.tier`, gets `null`, and hides a `severity: 'critical'` security advisory targeted at `audience: 'pro'` from exactly the user who needed it. No error, no retry, no log — Step 6 says swallow silently.
- **Evidence:**
  - `src/lib/pro.svelte.ts:22-23` — `/** Tier, seats and renewal date for the Subscription page. Display only. */ subscription: SubscriptionState | null`.
  - `src/lib/pro.svelte.ts:65-76` — catch block: `proState.subscription = null;` while `paid` is deliberately left untouched.
  - `src/lib/pro.svelte.ts:30` and `:96-98` — `paid: boolean`, the only durable entitlement signal.
  - `src/lib/api/subscription.ts:11` — `tier: string | null` inside `SubscriptionState`.
  - `src/lib/api/subscription.ts:82` — checkout tiers are `"pro" | "pro_teams"`; Phase 5's `audience` CHECK allows only `('free','pro')`.
  - `src/lib/supabase.ts:55-57, 109-110` — `accountAvailability()` returns `"browser-mode"`; `getSupabase()` returns `null` unless `"ready"`.
  - `src-tauri/src/subscription/` contains only `cache.rs` and `mod.rs` — there is no tier resolution surface beyond the cached entitlement.
- **Suggested fix:** Filter on `isPaid()` (the durable boolean) and state that explicitly, or drop `audience` from v1 entirely. Add a Phase 5 requirement that a `null`/unknown tier is treated as *show* for `severity: 'critical'`, never as *hide*. Document the browser-mode no-op.

---

## Finding 6: Phase 5 assumes a Supabase migration layout that does not exist anywhere in the repo

- **Severity:** High
- **Location:** Phase 5, "Related Code Files" — `Create: supabase/migrations/<ts>_announcements.sql` "theo layout migration mà `plans/260811-1930-subscription-teams-supabase-schema/phase-03` thiết lập"; Phase 5 Step 1 "áp lên Supabase project bằng tài khoản owner"; Phase 6 "Modify: `docs/setup.md` — bước tạo bảng announcements"
- **Flaw:** There is no `supabase/migrations/` directory and **zero** `.sql` files in the entire repository. The referenced plan is unmerged/unexecuted, so the "layout it establishes" does not exist. Phase 5's dependency section claims independence but its file plan is written as if the other plan already landed. `supabase/config.toml` and `supabase/functions/` exist, so the CLI is in play, but no migration history has ever been created.
- **Failure scenario:** Implementer runs `supabase db push` against a project with no migration baseline. Either the CLI errors on a missing migration table, or it applies the announcements migration as migration #1 — and when the subscription/teams plan later lands, its migrations sort *before* an already-applied timestamp, producing an out-of-order history that `supabase migration repair` has to unpick manually against the production project. Step 1 also says "áp lên Supabase project bằng tài khoản owner" with no mention of a local/staging apply first, so the first run of an unreviewed DDL is against production.
- **Evidence:**
  - `find . -name "*.sql" -not -path "*/node_modules/*" -not -path "*/target/*"` → **no results**.
  - `ls supabase/` → `.branches`, `.env`, `.env.example`, `.temp`, `config.toml`, `functions`, `snippets`. No `migrations/`.
  - `plans/260811-1930-subscription-teams-supabase-schema/` exists but is untracked (`git status` shows `??`), i.e. not landed.
- **Suggested fix:** Make Phase 5 own migration bootstrap explicitly (create `supabase/migrations/`, establish baseline) or declare a hard `blockedBy` on the subscription/teams schema plan. Add a step requiring local `supabase db reset` verification before touching the hosted project. Note: `supabase/.env` is present in the working tree — confirm it is gitignored before any of this work adds files next to it.

---

## Finding 7: Phase 6 depends on an i18n-key-diff script that does not exist and is never created

- **Severity:** Medium
- **Location:** Phase 6, Implementation Step 2 ("chính vì thế phải rà bằng script, không bằng mắt") and Success Criterion "Không khoá dịch nào thiếu ở bất kỳ locale nào"
- **Flaw:** The reasoning is correct — `t()` really does swallow missing keys via `{ default }`, so drift is invisible — but the plan cites a script as the mitigation without checking whether one exists. It does not, and Phase 6 does not list creating it under "Related Code Files". `src/lib/i18n.test.ts` tests language switching and lookup behaviour, not locale parity.
- **Failure scenario:** Phase 6 is the last phase and P2. Implementer hits Step 2, finds no script, eyeballs four JSON files, and misses a key in `ja.json`. `t()` silently renders the English `default`, no test fails, the success criterion is ticked. Japanese users get mixed-language UI in the new panel with no signal anywhere.
- **Evidence:**
  - `package.json:6-12` — no i18n script; `ls scripts/` → `build-compose-corpus.py`, `compose-autofix-gate.py`, `compose-diagnose-benchmark.sh`, `release.sh`, `security-scanner-bakeoff.py`, `split_api.cjs`, `validate-kb-compose.sh`. Nothing for locales.
  - `src/lib/i18n.svelte.ts:32,46-47` — `t(key, params?)`; `let fallback = params && params.default ? String(params.default) : key;` — the `{ default }` claim is **verified true**.
  - `src/lib/i18n.test.ts:1-30` — covers `setLanguage`/`getLanguage`/known-key lookup only.
  - Phase 6 also claims the `notifications.*` namespace "đã có": `src/locales/en.json` has a `transfer` object at line 398, but **no** `notifications` key. Half the claim is wrong.
- **Suggested fix:** Add `src/lib/locales-parity.test.ts` (a vitest that diffs the key sets of the four JSON files) to Phase 6's Related Code Files. A test, not a script, so it runs in the same gate as everything else — and correct the "namespace đã có" claim.

---

## Finding 8: Phase 4 breaks the exact import-direction invariant Phase 1 uses as its cycle mitigation, and both phases cite wrong settings paths

- **Severity:** Medium
- **Location:** Phase 1 "Risk Assessment" ("Giữ chiều phụ thuộc một hướng — `globalToast` import store, store **không** import `globalToast`"; "store hiện không import lib nào ngoài `errors` type") vs Phase 4 "Architecture" + Step 5 ("`store không nên biết gì về Tauri, giữ nó test được ở jsdom`" + "Modify: `src/store/notifications.svelte.ts` — gọi `osNotify` từ `settleJob`")
- **Flaw:** Phase 4 contradicts itself in adjacent sentences: it justifies extracting `osNotify.ts` on the grounds that the store must not know about Tauri, then wires the store to call it. The result is `lib/globalToast → store/notifications → lib/osNotify → lib/settingsStore`, which violates Phase 1's stated one-way rule (the rule is the *only* stated cycle mitigation) and makes the store's jsdom tests depend on mocking a Tauri-adjacent module. Additionally both phases point at directories that do not contain the files named.
- **Failure scenario:** Phase 1's `notifications.test.ts` is written clean in jsdom. Phase 4 lands, `settleJob` now imports `osNotify`, which reads `settingsStore` and dynamic-imports `@tauri-apps/plugin-notification`. The Phase 1 tests start needing setup they were explicitly designed not to need, and the "store is pure" invariant is gone with nothing recording that it was traded away.
- **Evidence:**
  - `src/store/errorLog.svelte.ts:13` — `import type { AppError } from "../lib/errors";` — confirms the current store→lib surface is a type-only import, as Phase 1 claims.
  - `src/lib/globalToast.ts:9-10` — `import { redact } from "./redact"; import type { AppError } from "./errors";` — adding a store import here is new, and the reverse edge Phase 4 adds closes the loop conceptually even if the module graph stays acyclic.
  - `src/lib/redact.ts:74` mentions `globalToast` only in a comment — no real cycle today.
  - Path errors: Phase 4 lists `Modify: src/pages/settings/` for the toggle, and Phase 5 lists `src/pages/settings/PrivacySettings.svelte`. `ls src/pages/settings/` → `Account.svelte`, `ColimaConfig.svelte`, `Subscription.svelte`. `PrivacySettings.svelte` and `SettingsSection.svelte` are in `src/components/settings/`.
  - `src/lib/settingsStore.svelte.ts:61-63` — `getAppSetting(key, defaultValue: string): string` — string-valued. Phase 5's "danh sách id đã đọc" needs a documented JSON-in-string encoding and an unbounded-growth bound; neither is specified.
- **Suggested fix:** Invert the Phase 4 wiring — have the app-level subscriber (the same one Phase 2 registers in `App.svelte`) observe settled jobs and call `osNotify`, keeping the store pure and the import direction one-way as Phase 1 promised. Fix the settings file paths in both phases. Specify the read-id encoding and cap.

---

## Finding 9: Phase 2's `reject_flag_like` warning is a phantom, while the real breaking change to a public HTTP contract is under-specified

- **Severity:** Medium
- **Location:** Phase 2, "Phần B / B3 — Chi tiết cần cẩn thận" and Risk Assessment
- **Flaw:** The plan spends a bullet warning that `reject_flag_like` will block the `-` destination and prescribing a workaround. It will not: `reject_flag_like` is only reached via `require_plain`, which `start_copy_from_container` applies to `container_path` (and `resolve_destination` applies to `dir`/`file_name`). The final argv element is built from `dest.to_string_lossy()` and never passes through either. The bullet is invented caution — an AI-review smell — and it displaces attention from the change that actually needs specifying: `POST /api/containers/cp/from` is a public HTTP endpoint whose output artefact type changes from "whatever docker cp produced" to "always a tar", with a new `require_archive_name` rejection added to a previously-accepting input.
- **Failure scenario:** Any existing script or browser-mode client calling `/api/containers/cp/from` with `file_name: "config.json"` starts receiving a hard error from `require_archive_name` after upgrade. The plan's mitigation is "ghi vào docs/ ở Phase 6" — but Phase 6 is the last phase, P2, and its own Risk Assessment concedes it may be dropped for time. There is no API version bump, no deprecation window, and no acceptance criterion that an old-shaped request produces an actionable error rather than a generic 400.
- **Evidence:**
  - `src-tauri/src/commands/file_transfer.rs:75-80` — `reject_flag_like` rejects only values passed to it.
  - `src-tauri/src/commands/file_transfer.rs:87-95` — `require_plain` is the only caller; it is applied to labelled *inputs*.
  - `src-tauri/src/commands/file_transfer.rs:494` — `require_plain("Container path", &container_path)?;` — the only `require_plain` call in that function besides `resolve_destination`'s internal ones at `:104-105`.
  - `src-tauri/src/commands/file_transfer.rs:503-506` — argv is `["cp", "<id>:<path>", dest.to_string_lossy()]`; the destination is never validated by `reject_flag_like`.
  - `src-tauri/src/routes/file_transfer.rs:55-58` + `src-tauri/src/routes/payloads.rs:177` (`CopyFromContainerBody`) + `src/lib/api/transfer.ts:82-85` — this is a live HTTP endpoint, not a Tauri-only command.
  - Positive control: the plan's claim that the `ToFile` sink is auto-cleaned on cancel **is** correct — `src-tauri/src/streaming_cmd.rs:80` ("On cancellation or a non-zero exit, a partial `ToFile` destination is removed") and `:284-298` (`remove_file(path)` on cancelled and on failure). Dropping `cleanup_on_abort` is safe; the `remove_file`-on-a-directory bug at `file_transfer.rs:511-513` is real.
- **Suggested fix:** Delete the `reject_flag_like` bullet. Add an explicit breaking-change subsection to Phase 2 (not Phase 6) covering `/api/containers/cp/from` and `/api/images/save`: the rejection message text, an acceptance criterion that a non-`.tar` `file_name` returns a message naming the accepted form, and a `docs/api.md` edit inside Phase 2 so it cannot be dropped with Phase 6.

---

## Verification Results

### State lifetime classification

| State | Lifetime | Instantiation sites | Verdict |
|---|---|---|---|
| `notificationState` (proposed, `src/store/notifications.svelte.ts`) | process-global module singleton, session-scoped, non-persisted | 1 (module-level `$state`, per plan) | **FAILED** — duplicates `errorLogState` for the error case (Finding 2a). No isolation-boundary leak (single-user desktop app, one window), but it is new state serving an existing purpose. |
| `errorLogState` (`src/store/errorLog.svelte.ts:28-31`) | process-global module singleton, session-scoped, non-persisted, `MAX_ENTRIES = 50` | 1 | PASS — pre-existing; the plan does not reconcile with it. |
| `_active` / `_timers` / `_nextId` (`src/lib/globalToast.ts:36-53`) | process-global module singletons, entries evicted on TTL | 1 each | PASS as-is, but the plan's Phase 1 step 4 misreads `_active` eviction semantics (Finding 2b). |
| `uiState` (`src/store.svelte.ts:45+`) | process-global module singleton, `currentPage`/`sidebarCollapsed` mirrored to persisted app settings | 1 | PASS — correct home for the panel-exclusivity flag Phase 3 proposes; but `aiPanelOpen` is the only existing panel flag (Finding 4). |
| `proState` (`src/lib/pro.svelte.ts:18-32`) | process-global module singleton; `paid` durable across read failures, `subscription` nulled on failure | 1 | PASS for `paid`; **FAILED** as a tier source for Phase 5 (Finding 5). |
| `settingsState` (`src/lib/settingsStore.svelte.ts`) | process-global, persisted, **string-valued** | 1 | PASS, but Phase 5's read-id list needs an encoding + growth bound that the plan does not specify (Finding 8). |
| `transferEvents` `EventSource` | currently per-call (dialog-scoped); plan promotes to app/process lifetime | 1 per `subscribeTransfer()` call | **FAILED** — promoting the lifetime without adding reconnect converts a self-healing resource into a permanent-failure singleton (Finding 3). |
| `announcements` poll `setInterval` (proposed) | process lifetime, 6 h | 1, in `App.svelte` | PASS provided teardown is real; `App.svelte` never unmounts, so the teardown clause in Phase 5 step 4 is decorative. |

### Failed plan assumptions

| # | Assumption | Verdict |
|---|---|---|
| 1 | "`pnpm test` xanh" is a runnable gate | **FAILED** — no `test` script (`package.json:6-12`) |
| 2 | "~100 call site" for `globalToast` | **FAILED** — 130 calls (66 error / 57 success / 3 info) |
| 3 | Collapsed toasts prevent Notification Center flooding | **FAILED** — `_active` is cleared on TTL (`globalToast.ts:71-84, 42`) |
| 4 | No existing state serves this purpose | **FAILED** — `errorLogState` (`errorLog.svelte.ts:15-43`) |
| 5 | Backend "đã chạy nền rồi", frontend-only redesign | **PARTIAL** — true for job execution (`spawn_job`/`streaming_cmd`), but Phase 2B rewrites `start_copy_from_container` and adds two new Rust validators; the framing understates Rust scope |
| 6 | Sidebar has a reusable panel-toggle mechanism | **FAILED** — `Sidebar.svelte:306` hardcodes `item.id === "ai-chat"` |
| 7 | `tauri-plugin-notification` is absent from Cargo/npm/capabilities | **CONFIRMED TRUE** — `Cargo.toml:23,41,42,45,53`; `package.json:17-22`; `capabilities/default.json` permissions array |
| 8 | Capabilities are declared as string permissions in `capabilities/default.json` | **CONFIRMED TRUE** — that is exactly the repo's mechanism |
| 9 | Tier is resolvable offline client-side at poll time | **FAILED** — display-only, nulled on failure, boolean-only durable signal, null in browser mode |
| 10 | A migration layout exists to follow | **FAILED** — zero `.sql` files repo-wide; no `supabase/migrations/` |
| 11 | An i18n key-diff script exists | **FAILED** — not in `package.json` or `scripts/` |
| 12 | `t()` accepts `{ default }` | **CONFIRMED TRUE** — `i18n.svelte.ts:46-47` |
| 13 | `notifications.*` and `transfer.*` namespaces "đã có" | **PARTIAL** — `transfer` exists (`en.json:398`); `notifications` does not |
| 14 | `streaming_cmd` auto-cleans `ToFile` on cancel, so `cleanup_on_abort` can be dropped | **CONFIRMED TRUE** — `streaming_cmd.rs:80, 284-298` |
| 15 | `reject_flag_like` would block the `-` destination | **FAILED (phantom risk)** — destination never passes through it (`file_transfer.rs:75-95, 503-506`) |
| 16 | Store imports no `lib` module (one-way dependency) | **FAILED by Phase 4's own design** — Phase 4 step 5 adds `store → lib/osNotify` |
