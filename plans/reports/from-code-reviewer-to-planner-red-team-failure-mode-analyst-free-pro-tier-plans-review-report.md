# Red-team review — Failure Mode Analyst: Free tier + Pro tier plans

Reviewer: code-reviewer (hostile / failure-mode lens)
Date: 2026-08-11
Plans reviewed: `plans/260811-2241-free-tier-foundation/`, `plans/260811-2245-pro-tier-features/`
Method: flow tracing from claimed entry points through actual call paths in `src-tauri/src/`.

---

## Finding 1: `crash.rs` persists nothing — the diagnostic bundle reads a file that does not exist

- **Severity:** Critical
- **Location:** Plan A, Phase 2, "Implementation Steps" step 1 + "Requirements" (bundle gồm "crash gần nhất từ `crash.rs`")
- **Flaw:** The plan states the crash module "hiện chỉ có ghi" (currently only writes) and asks to add `latest_report()` to read the saved report. Traced path: `crash::install()` (`src-tauri/src/crash.rs:48`) sets a panic hook whose entire sink is `eprintln!("{report}")` (`src-tauri/src/crash.rs:53`). There is no file write, no directory, no rotation. The module doc says transmission is deferred and no sink exists (`src-tauri/src/crash.rs:13-15`, `44-47`).
- **Failure scenario:** Implementer writes `latest_report()`, finds no storage, and improvises one mid-phase: crash file path, rotation policy, size cap, and redaction-at-write all become unplanned work inside a phase budgeted as "rẻ nhất". Worst case the improvised writer stores the *unredacted* payload and Phase 2 then ships that text into a GitHub issue body — the exact leak the module was built to prevent. Alternatively the section silently reports "(không thu thập được)" forever and the phase's stated purpose (feeding real failure signatures to Pro Phase 1) never materialises.
- **Evidence:** `src-tauri/src/crash.rs:35-41` (`redacted_report` returns a `String`), `src-tauri/src/crash.rs:48-55` (hook only `eprintln!`), no `File`/`write`/`fs` reference anywhere in the 87-line module.
- **Suggested fix:** Add an explicit sub-step to Phase 2: "persist redacted crash reports to `~/.colima-ui/crashes/` (ring of N, redact at write, cap per-file size)" with its own success criterion. Until that exists, `latest_report()` has no source.

---

## Finding 2: subscriber counting has no viable mechanism — the collector-stops-at-zero criterion cannot be met as designed

- **Severity:** Critical
- **Location:** Plan A, Phase 3, "Architecture" (`start()/stop() theo số subscriber`) + "Implementation Steps" step 5 ("suy ra từ SSE connection lifecycle — chọn cách sau nếu `sse.rs` đã theo dõi được số connection")
- **Flaw:** `sse.rs` does not track connections. There is one process-global `broadcast::Sender` (`src-tauri/src/sse.rs:17-27`) shared by every event type. `Sender::receiver_count()` counts *all* SSE clients — the docker watcher stream, k8s streams, KB streams — not metrics subscribers. Traced consumer path: `routes/misc.rs:6-17` subscribes on every `/api/events` connection with no registration, no id, no teardown callback. The conditional in step 5 resolves to false, and the plan gives no fallback design beyond `POST /subscribe|/unsubscribe`.
- **Failure scenario:** Implementer falls back to explicit subscribe/unsubscribe HTTP calls. The UI reloads (Vite HMR, window refresh, browser-mode tab close, app crash) without firing `/unsubscribe`. The counter never returns to zero, the tokio tick task runs `docker stats --no-stream` every 2s forever in the background, and the Pro SQLite sink attached to it (Pro Phase 2) keeps writing 24/7 while the user believes the feature is idle. The stated success criterion "Đóng trang Activity → collector dừng hẳn" passes in the manual happy-path test and fails in every abnormal exit.
- **Evidence:** `src-tauri/src/sse.rs:17-27` (single global channel, no registry), `src-tauri/src/routes/misc.rs:7` (`get_sse_tx().subscribe()` with no bookkeeping), `src-tauri/src/routes/k8s.rs:809` and `src-tauri/src/routes/kb.rs:173` (other consumers of the same channel).
- **Suggested fix:** Design the lease explicitly in Phase 3: a keep-alive lease with TTL (client re-asserts every N seconds; collector stops when no lease renewed within 2×TTL), plus a hard idle timeout. Drop the "derive from SSE" option — it is not available. Add a criterion: "kill the UI process without unsubscribing → collector stops within 2×TTL".

---

## Finding 3: everything is bound to one auto-detected docker endpoint — `?instance=` and cross-instance transfer have no plumbing, and metrics have no instance dimension

- **Severity:** Critical
- **Location:** Plan A, Phase 4, step 4 (`GET /api/topology?instance=<name>`); Plan B, Phase 5, "Architecture" (`docker --context <from> save | docker --context <to> load`); Plan B, Phase 2, "Schema"
- **Flaw:** Every docker invocation in the app targets a single implicitly-chosen endpoint. `run_cmd` sets `DOCKER_HOST` from `detect_docker_host()` (`src-tauri/src/helpers.rs:103-107`), which returns the **first running colima profile found while scanning `~/.colima`** (`src-tauri/src/path_util.rs:172-202`). `DockerState` holds one bollard client built the same way (`src-tauri/src/docker_state.rs:26-45`). Nothing in the codebase passes `--context`, and no helper accepts a per-call docker target. Meanwhile the Pro Phase 2 schema (`samples_raw`, `samples_1m`, `samples_1h`, `containers_seen`) has **no instance/endpoint column**.
- **Failure scenario:** (a) Free Phase 4 ships `?instance=X` as a parameter the backend accepts and silently ignores, because `commands/topology.rs` reuses `docker_state.rs` as instructed in step 3 — the acceptance box "Endpoint nhận tham số instance" is checked while the behaviour is a lie, and Pro Phase 5 inherits it. (b) A user runs two instances and switches; the collector begins sampling a different daemon while writing into the same tables. Container short ids collide across instances, `containers_seen` upserts one instance's name over another's, and a 7-day history chart silently interleaves two machines. That data is unrecoverable — there is no column to disambiguate after the fact. (c) Pro Phase 5's `docker --context` pipeline is a new invocation style the codebase has never used; if the user's docker CLI has no context for a colima profile (colima only creates contexts in some configurations), the transfer fails with a raw CLI error and the arch pre-check runs against the wrong daemon.
- **Evidence:** `src-tauri/src/path_util.rs:172-202`, `src-tauri/src/helpers.rs:103-107`, `src-tauri/src/docker_state.rs:26-45`, `src-tauri/src/commands/containers.rs:606-612` (`all_container_stats` → `docker_output`, no target argument).
- **Suggested fix:** Make "per-call docker endpoint targeting" an explicit prerequisite step — either in Free Phase 4 or as a new Phase 0 — covering a `DockerTarget { docker_host, context }` resolved from `instance_reader`, threaded through `run_cmd`/`docker_output`/bollard. Add `instance_id` to every metrics table and to the primary keys/indexes in Pro Phase 2. Retrofitting an instance column after data exists is a migration with no correct backfill.

---

## Finding 4: the SSE transport silently drops lagged samples — no gap signal reaches the UI or the drop counter

- **Severity:** High
- **Location:** Plan A, Phase 3, "Architecture" (`publish_sse_event("metrics.sample", samples)`); Plan B, Phase 2, "Bounded buffer" (drop-and-count) and Phase 3, risk row "bỏ qua mẫu có gap thời gian lớn"
- **Flaw:** The plan's drop accounting covers only the SQLite writer channel. The SSE path has an independent, undocumented drop: the global broadcast channel has capacity **64** (`src-tauri/src/sse.rs:22`), and the consumer maps every error — including `BroadcastStreamRecvError::Lagged(n)` — to `None`, i.e. discards it without emitting anything (`src-tauri/src/routes/misc.rs:9-12`).
- **Failure scenario:** A background/throttled webview tab, or a burst of docker events during `compose up` (the watcher publishes on the same channel, `src-tauri/src/docker_state.rs:118-160`), pushes a slow client past 64 buffered messages. The client loses an arbitrary run of `metrics.sample` events and receives **no gap marker**. The sparkline draws a straight line across the hole, the "Số liệu khớp `docker stats`" criterion silently fails, and the Phase 3 mitigation "bỏ qua mẫu có gap thời gian lớn" cannot fire because the frontend has no way to distinguish "no sample" from "sample lost".
- **Evidence:** `src-tauri/src/sse.rs:22` (`broadcast::channel(64)`), `src-tauri/src/routes/misc.rs:9-12` (`Err(_) => None`), `src-tauri/src/docker_state.rs:122-160` (same channel used by high-rate docker event refreshes).
- **Suggested fix:** In Free Phase 3, either give metrics a dedicated broadcast channel with an explicit capacity derived from tick rate × client count, or convert `Lagged(n)` into an explicit `metrics.gap` event. Add a criterion: "force a slow client; UI shows a gap rather than a fabricated continuous line."

---

## Finding 5: crash-loop detection is traced to a debounced state-refresh path that cannot count restarts

- **Severity:** High
- **Location:** Plan B, Phase 3, "Implementation Steps" step 4 ("subscribe stream sự kiện container qua `docker_state.rs`"); Plan B, Phase 4, step 2 (same claim for `CrashLoop`, `Unhealthy`, `OomKilled`)
- **Flaw:** Flow trace FAILED. `docker_state.rs` exposes no event stream. `start_docker_watcher` (`src-tauri/src/docker_state.rs:62`) consumes the bollard event stream internally and applies a **500 ms trailing-edge debounce**: it keeps draining and resetting the timer while events keep arriving, then performs one cache refresh (`src-tauri/src/docker_state.rs:118-160`). Individual events are consumed and discarded; only the post-debounce aggregate state is published. There is no `pub fn subscribe_events()` anywhere in the module.
- **Failure scenario:** A container in a tight crash loop emits `die`/`start` pairs faster than 500 ms apart. The debounce collapses the entire burst into a single refresh, so a subscriber built on this path observes *one* transition, not five. Pro Phase 3's criterion "Crash-loop rule bắt được container restart 5 lần trong 2 phút" fails precisely in the fast-loop case that matters most. Worse, Pro Phase 4 rule #2 ("crash-loop → dừng hẳn") never fires, so rule #1 ("unhealthy → restart") keeps restarting the same container against its quota — the plan's own "restart lặp vô hạn" mitigation depends on the detector that does not work.
- **Evidence:** `src-tauri/src/docker_state.rs:59-62` (watcher entry), `:118-160` (debounce loop, events consumed in place), `:184-197` (on disconnect: reconnect, emit connection-lost). No exported per-event API in the module's public surface.
- **Suggested fix:** Add an explicit step: fan out raw docker events to a dedicated broadcast channel *before* the debounce, or count restarts from `docker inspect .RestartCount` polled on the collector tick. Either way, name the source of truth for restart counts in the phase file, and add the burst case (5 restarts in 20 s) to the criteria.

---

## Finding 6: self-heal quota and violation state are in-memory only — an app restart resets both, in exactly the situation that causes restarts

- **Severity:** High
- **Location:** Plan B, Phase 3, step 3 ("giữ state vi phạm trong bộ nhớ… app restart thì reset là đúng ngữ nghĩa"); Plan B, Phase 4, "Requirements" (`số lần tối đa/giờ`) and step 3
- **Flaw:** The plans state alert-violation state is deliberately in-memory and never say where the per-hour heal quota counter lives. `HealLog` is persisted, but the plan does not derive quota from the log; step 3 reads as an in-memory check. Nothing in the codebase provides durable counters for this — the only persistent store is the KB SQLite DB.
- **Failure scenario:** Rule "unhealthy > 5 min → restart", mode Auto, quota 3/hour. The user's machine is under memory pressure; the app itself is killed or the user quits and relaunches (common when things are going wrong). On relaunch the quota resets to 0 and the violation timer restarts. Over an hour with three app restarts, the container is restarted 9+ times — the quota, which the plan lists as the primary mitigation for "rủi ro cao nhất cả roadmap", provides no bound across process lifetime. Symmetrically, the in-memory duration window means a rule with `duration_sec: 300` can *never* fire on a machine where the app restarts every 4 minutes, so alerts silently stop working with no log line.
- **Evidence:** No durable counter primitive exists; the only persistence pattern in the repo is `static DB: OnceLock<Mutex<Connection>>` in `src-tauri/src/commands/knowledge_bank.rs:7-14`, and only `ExitRequested` is handled at shutdown (`src-tauri/src/lib.rs:329-331`) — a SIGKILL or force-quit runs no teardown at all.
- **Suggested fix:** Define quota as a query over persisted `heal_log` (`COUNT(*) WHERE rule_id=? AND ts > now-1h`), not an in-memory counter — the log already exists and the plan already requires writing it. Persist the "violating since" timestamp per (rule, container) or explicitly document and test that restarts reset duration windows.

---

## Finding 7: entitlement is dynamic and async, but the sink is registered once at boot — upgrade needs a restart, downgrade keeps writing

- **Severity:** High
- **Location:** Plan B, Phase 2, step 7 ("chỉ đăng ký khi entitlement Pro; đây là điểm gate phía backend") and criterion "Tài khoản Free: sink không được đăng ký, `metrics.db` không tồn tại"
- **Flaw:** `pro_status()` is `async` and re-runs sidecar detection on every call (`src-tauri/src/pro/mod.rs:56-58`, doc at `:50-53`), and the subscription cache expires on a wall-clock timestamp (`src-tauri/src/subscription/cache.rs:42-62`). Entitlement is therefore a value that changes *during* a session. The plan's registration is a single boot-time decision in `lib.rs`.
- **Failure scenario:** (a) A user upgrades to Pro in-app; the sink was not registered at boot, so metrics history stays empty with no error and no prompt — first-day churn for the exact user who just paid. (b) A subscription lapses mid-session; the already-registered sink keeps writing to `metrics.db` because nothing re-evaluates. (c) Calling `pro_status()` per tick to avoid this puts an await on sidecar socket detection inside the collector loop, coupling the sampling cadence to sidecar responsiveness — a hang there stalls the tick the plan promises is unblockable. (d) The criterion "`metrics.db` không tồn tại" is untestable after any downgrade, since Phase 2's own risk table says the DB is deliberately never deleted.
- **Evidence:** `src-tauri/src/pro/mod.rs:45-58`, `src-tauri/src/subscription/cache.rs:49-62` (missing/unparseable expiry fails closed → status can flip at any moment), `src-tauri/src/lib.rs:325-332` (only lifecycle hook is ExitRequested).
- **Suggested fix:** Register the sink unconditionally and gate at *write time* with a cached entitlement snapshot refreshed by an existing status-change signal (not per-tick detection). Restate the criterion as "Free account: no rows are written and no query endpoint returns data", which is testable in both directions.

---

## Finding 8: `metrics.db` conflates retention-managed telemetry with user configuration that must never be deleted

- **Severity:** High
- **Location:** Plan B, Phase 2 ("xoá theo tuổi và theo dung lượng trần", step 5); Phase 3 (`alert_rules`, `alert_events` in `metrics.db`); Phase 4 (`heal_rules`, `heal_log` in `metrics.db`)
- **Flaw:** Phase 2 correctly argues KB and metrics must be separate DBs because "vòng đời, tần suất ghi và chính sách xoá hoàn toàn khác nhau" — then Phases 3 and 4 put alert rules, heal rules, and the heal audit log into that same retention-managed, size-capped, entitlement-gated file. Additionally, the WAL claim in step 1 ("đọc và ghi chạy song song") does not hold if the implementer copies the only SQLite pattern in the repo: a single `Mutex<Connection>` (`src-tauri/src/commands/knowledge_bank.rs:7-14`), under which readers serialise behind the writer regardless of journal mode.
- **Failure scenario:** The 200 MB ceiling is hit. The retention task "xoá thêm từ cũ nhất" — an implementer writing a size-driven cleanup in a DB that also contains `heal_log` and `alert_events` has an obvious oldest-rows target that includes the audit trail the plan requires be complete ("Mọi hành động tự động đều có dòng log tương ứng"). Separately, a user whose Pro lapses finds their configured self-healing rules inside a DB that Phase 2 declares off-limits for queries — the global kill switch's state is now in an unreadable file. And with a single mutexed connection, a 7-day query holds the lock while the 5-second batch writer waits, so the "collector tick không bao giờ bị chặn" criterion depends on the bounded channel absorbing a multi-hundred-millisecond stall every time the user opens a chart.
- **Evidence:** `src-tauri/src/commands/knowledge_bank.rs:7-14` and `:25` (single global `Mutex<Connection>`, `Connection::open` with no `journal_mode` pragma — the pattern Phase 2 will be copied from), Plan B Phase 2 "Non-functional" vs Phase 3 file list line "Modify: `metrics_store.rs` (bảng `alert_rules`, `alert_events`)" and Phase 4 line "(bảng `heal_rules`, `heal_log`)".
- **Suggested fix:** Split config/audit (`rules.db`, never retention-pruned, never entitlement-gated for read) from samples (`metrics.db`, prunable). State explicitly that retention DELETEs are scoped to `samples_*` tables only. Specify a reader connection pool or a dedicated read connection — one mutexed connection makes the WAL rationale false.

---

## Finding 9: `docker save | docker load` has no defined partial-failure contract

- **Severity:** High
- **Location:** Plan B, Phase 5, "Architecture" (stream qua pipe, KHÔNG ghi file tạm) and criterion "Huỷ giữa chừng: không để lại image dở dang ở đích"
- **Flaw:** The plan asserts pipe streaming avoids temp files and that cancellation leaves no partial image, with the mitigation "kiểm tra exit code cả hai đầu pipe; dọn image dở ở đích khi fail". There is no defined way to identify what to clean up: a truncated `docker load` stream may import some layers and no image reference, or import a subset of tags for a multi-tag save. Nothing in the repo supports a two-process pipe with independent cancellation — every docker call goes through one-shot `Command::output()` (`src-tauri/src/helpers.rs:99-125`, `src-tauri/src/commands/containers.rs:51`), which buffers the entire output in memory. Naively reusing that helper for `docker save` of a 500 MB image loads it into RAM.
- **Failure scenario:** User cancels a 500 MB transfer at 60%. The `save` side is killed; `load` reads EOF mid-stream and exits non-zero. The destination now holds dangling layers consuming disk that no `docker image rm` targets — the "không để lại image dở dang" criterion is checked by looking at `docker images`, which shows nothing, while the disk stays consumed. Worse: the plan's own risk table lists "Instance dừng giữa lúc transfer" with mitigation "kiểm tra trạng thái trước" — a pre-check that cannot cover a stop occurring *during* a multi-minute transfer, and the app's own stop path sets `DockerState.suppressed` and clears state (`src-tauri/src/docker_state.rs:30-33`), so the transfer's error surface during that window is undefined.
- **Evidence:** `src-tauri/src/helpers.rs:99-125` (`Command::output()`, full buffering, no streaming/stdin plumbing), `src-tauri/src/commands/containers.rs:51` (`docker_output` same shape), `src-tauri/src/docker_state.rs:29-33` (`suppressed` flag set by stop/delete paths).
- **Suggested fix:** Specify the streaming primitive explicitly (`Stdio::piped()` + `tokio::io::copy` with a cancellation token, plus a byte counter for progress) — it does not exist today and is not a "reuse Free Phase 1 pattern" item. Define post-failure cleanup as `docker image prune --filter dangling=true` scoped to the destination *with user confirmation*, or accept and document that cancellation may leave dangling layers, and change the criterion to match.

---

## Finding 10: Free Phase 1 asserts a Tauri dialog capability that is not installed

- **Severity:** Medium
- **Location:** Plan A, Phase 1, "Architecture" — "dialog chọn path (tauri-plugin-dialog, đã có trong capabilities)"
- **Flaw:** False premise. `src-tauri/capabilities/default.json` grants `core:default`, `opener:default`, `updater:default`, `deep-link:default`, event and window permissions, and `http:default` — there is no `dialog:` entry. `src-tauri/Cargo.toml:21-52` lists no `tauri-plugin-dialog`.
- **Failure scenario:** The phase is scoped as GUI-over-existing-commands, and the entire path-selection UX rests on a plugin that must be added: new Cargo dependency, plugin registration in `lib.rs`, new capability permissions, and a re-review of the capability surface (this app deliberately keeps a minimal permission set — adding filesystem dialog scope is a trust-boundary change, not a chore). Discovered at implementation time, this either blocks the phase or gets worked around with a raw text path input, which routes untrusted user-typed host paths straight into the command builder — the precise risk the phase's own security note calls "điểm dễ sai nhất".
- **Evidence:** `src-tauri/capabilities/default.json` (full permission list, no dialog), `src-tauri/Cargo.toml:21-52` (no dialog plugin). Related smaller inaccuracy: Plan B Phase 1 refers to `serde_yaml`; the repo uses `serde_yml = "0.0.12"` (`src-tauri/Cargo.toml:30`) — a different crate with different comment-preservation behaviour, which matters for that phase's "giữ nguyên comment" requirement.
- **Suggested fix:** Add an explicit step 0 to Free Phase 1: add `tauri-plugin-dialog`, register it, add the narrowest dialog permission, and note the capability change in the phase's risk table. Correct the crate name in Plan B Phase 1 and verify `serde_yml` round-trip behaviour before relying on the stated mitigation.

---

## Cross-cutting note on dependency ordering

Plan A's `plan.md` states Phases 1, 2, 4 are independent and parallelisable. Findings 1, 3, and 10 each add unplanned foundational work inside those phases (crash persistence, per-instance docker targeting, dialog plugin + capability change). Findings 2, 3, 4, and 5 all land on the Free Phase 3 → Pro Phase 2/3 seam, which the plan itself identifies as the highest-leverage design decision ("nếu collector thiết kế sai, Pro phase 2 sẽ phải viết lại"). The seam as currently specified rests on three mechanisms that do not exist in the codebase: SSE subscriber accounting, per-event docker notifications, and per-instance docker targeting.
