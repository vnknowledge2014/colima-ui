# Red-team review — Assumption Destroyer / Scope Auditor

Plans reviewed:
- Plan A: `/Users/longnd/Desktop/vnknowledge/colima-ui/plans/260811-2241-free-tier-foundation/`
- Plan B: `/Users/longnd/Desktop/vnknowledge/colima-ui/plans/260811-2245-pro-tier-features/`

Method: every "already exists / can be reused" claim was grepped against `src-tauri/src/` and `src/`.

---

## Finding 1: Backend Pro gating has no trustworthy identity to gate on

- **Severity:** Critical
- **Location:** Plan B, `plan.md` section "Quy ước gating (áp dụng mọi phase)" ("Gate ở UI + backend"); Plan B Phase 2, step 7 ("chỉ đăng ký khi entitlement Pro; đây là điểm gate phía backend"); Phase 5, step 9.
- **Flaw:** There is no server-side session. Entitlement is client-asserted at three separate points.
  - `subscription_state(user_id: Option<String>)` derives entitlement from whatever `user_id` the *caller* passes (`src-tauri/src/subscription/mod.rs:116`), and the HTTP variant reads it from a query string (`src-tauri/src/subscription/mod.rs:164-172`).
  - `subscription_store` accepts an arbitrary `EntitlementPayload { entitled: bool, expires_at }` from the caller and writes it straight to the cache (`src-tauri/src/subscription/mod.rs:126-143`); the HTTP route `POST /api/subscription/store` is exposed (`src-tauri/src/api_server.rs:142`). No JWT verification exists anywhere in Rust — `account_session.rs` only stores/reads opaque keychain strings (`src-tauri/src/account_session.rs:40-62`).
  - The API token that protects every other route is handed out by an **unauthenticated** endpoint: `/api/auth/token` sits in `public_routes` (`src-tauri/src/api_server.rs:256`) and returns the token verbatim (`src-tauri/src/routes/system.rs:263-265`).
- **Failure scenario:** Any local process (or the user) does `GET /api/auth/token`, then `POST /api/subscription/store {"user_id":"x","entitled":true,"tier":"pro","expires_at":"2099-01-01T00:00:00Z"}`, and every "backend gate" in Plan B phases 1–5 now returns Pro. The plan's acceptance criterion "gate ở backend, không chỉ UI" is satisfied on paper and worthless in practice.
- **Suggested fix:** Either (a) declare explicitly in Plan B `plan.md` that local gating is honour-based anti-friction, not anti-tamper, and drop the "backend gate" acceptance line; or (b) add a prerequisite phase that verifies the Supabase JWT in Rust and derives entitlement from a server-held session, and route `subscription_store` behind it. Do not ship phases 1–5 claiming enforcement that the current state model cannot deliver.

## Finding 2: The capability that Plan B phase 1 is built on is resolved by a sidecar binary that does not exist

- **Severity:** Critical
- **Location:** Plan B, `plan.md` Overview table row 1 and Phase 1 Overview ("Capability `compose.autofix` đã khai sẵn trong `ProGate.svelte`"); Plan B, `plan.md` Acceptance ("Mỗi capability mới xuất hiện đủ ở: `ProGate.svelte` GATED_ENUM, enum Rust").
- **Flaw:** The `ProGate.svelte` half of the claim is true (`src/components/ProGate.svelte:13-14`), but the plan mistakes a *telemetry* enum for a *capability* enum, and the actual capability source is unbuilt.
  - The Rust `GatedCapability` enum has exactly three variants and exists only to type a telemetry event — `ComposeAutofix, ComposeDiagnose, DockerfileOptimize` (`src-tauri/src/telemetry/events.rs:30-34`), used at `events.rs:55` / `telemetry/mod.rs:103`. Adding `metrics.history`, `metrics.alerts`, `heal.rules`, `cluster.transfer` there grants nothing; it only records funnel events.
  - The real answer to "may this run" is `ProStatus::Active { capabilities }` (`src-tauri/src/pro/mod.rs:28-29`, `has_capability` at `mod.rs:45`), populated exclusively from a **sidecar handshake** (`src-tauri/src/pro/bridge.rs:87`, `detect()` at `bridge.rs:99`). The sidecar socket path is a self-described placeholder: "The production path here is a documented placeholder. The final location is settled together with the bundled sidecar and its signing setup (Phase 3, once the binary exists)" (`src-tauri/src/pro/bridge.rs:32-41`).
- **Failure scenario:** Phase 1 is implemented and wrapped in `ProGate capability="compose.autofix"`. `hasCapability()` (`src/lib/pro.svelte.ts:111-113`) returns false for every user including paying ones, because `pro_status` always resolves to `Free` with no sidecar present. The feature is unreachable for 100% of users and the failure is silent by design ("absence is normal, not an error", `bridge.rs:6`).
- **Suggested fix:** Add an explicit prerequisite to Plan B `plan.md` Dependencies: the Pro sidecar binary, its socket path, and its capability list must exist and be bundled before *any* phase gates on `hasCapability`. Or change the gate source to `proState.paid` (the Supabase path, which does work) and state that decision — but note `paid` and `capabilities` are two independent systems (`src/lib/pro.svelte.ts:54` comments them as different questions) and the plan never says which one wins.

## Finding 3: `crash.rs` writes nothing to disk — the planned `latest_report()` has nothing to read

- **Severity:** Critical (blocks the stated phase-2 deliverable)
- **Location:** Plan A, Phase 2, Requirements ("crash gần nhất từ `crash.rs`"), Related Code Files ("Modify `crash.rs` (thêm hàm đọc crash report gần nhất, **hiện chỉ có ghi**)"), Implementation step 1.
- **Flaw:** The claim "hiện chỉ có ghi" is false. `install()` sets a panic hook whose entire body is `eprintln!("{report}")` (`src-tauri/src/crash.rs:48-54`). `redacted_report()` only formats a string (`crash.rs:35-40`). There is no file, no path, no rotation. The module doc states transmission is deferred and the value is "it stops a secret from reaching the terminal" (`crash.rs:12-15`).
- **Failure scenario:** Implementer writes `latest_report()`, finds no storage, and either (a) silently ships a bundle section that is always "(không thu thập được)" — quietly deleting a headline requirement, or (b) invents crash persistence mid-phase: file location, size cap, rotation, redaction-at-rest, and cleanup, none of which is estimated, planned, or in the success criteria.
- **Suggested fix:** Split crash persistence into its own numbered step with its own acceptance (where the file lives, rotation policy, max size, who deletes it), or drop the crash section from the bundle scope for phase 2.

## Finding 4: `redact()` does not cover three of the four secrets the phase-2 success criteria demand

- **Severity:** Critical
- **Location:** Plan A, `plan.md` "Nền tảng tái dùng" (lists `redact.rs` as reusable foundation); Phase 2, step 9 and Success Criteria ("AWS key, token bearer, password trong env, đường dẫn `/Users/<tên>` → khẳng định không lọt"); Risk table row 1.
- **Flaw:** `redact()` applies exactly four rule families (`src-tauri/src/redact.rs:79-94`):
  1. URL query params (`redact.rs:26`) — matches `?password=` in a URL only, **not** `PASSWORD=hunter2` in an env dump or a log line.
  2. `Bearer <token>` (`redact.rs:33`) — the one criterion that passes.
  3. Header-shaped `x-api-key/authorization: value` (`redact.rs:40-44`).
  4. Provider key shapes: `sk-`, `AIza`, `gsk_`, `hf_`, `gh[pousr]_`, `colima-<64hex>` (`redact.rs:56-67`).
  There is **no AWS pattern** (`AKIA…`/`ASIA…` and no secret-access-key rule), **no `KEY=VALUE` rule**, and **no home-path rule**. Worse, the module deliberately refuses generic high-entropy matching and documents why: "Notably absent are bare hex/alphanumeric runs… A 64-char hex pattern would also match Docker image digests" (`redact.rs:49-53`). Container logs — the single largest content source in the bundle — are exactly where secrets appear as `KEY=VALUE` and bare tokens.
- **Failure scenario:** A user hits "Report a problem". The bundle includes the last N log lines of a failing app container; those lines contain `DATABASE_URL=postgres://user:pw@host/db`, `AWS_SECRET_ACCESS_KEY=…`, and `/Users/realname/...`. All pass `redact()` untouched. The preview UI shows them, the user does not read 500 lines of log, and pastes it into a public GitHub issue. Plan A's own risk table calls this "rủi ro lớn nhất của phase" and then lists `redact.rs` as already-solved infrastructure.
- **Suggested fix:** Promote "extend `redact.rs`" from a conditional Modify ("bổ sung pattern nếu test lộ thiếu sót") to a mandatory first step with named patterns (AWS key ids, `(?i)(pass|secret|token|key)[A-Z_]*\s*=\s*\S+`, `/Users/[^/]+` → `/Users/<user>`), and write the fixture tests *before* wiring the bundle. Also decide explicitly whether container logs are in scope at all — the requirement says "không env" for the container list but says nothing about env leaking through log text.

## Finding 5: Phase A-4 plans to delete two Kubernetes views that have nothing to do with the graph it is building

- **Severity:** Critical
- **Location:** Plan A, Phase 4, Overview ("đã có hai trang chồng lấn — ClusterTopology.svelte (114 dòng) và XRay.svelte (358 dòng). Phase này hợp nhất chúng"), Related Code Files ("Delete: …"), Implementation step 8, Success Criteria row 1, and `Modify src/App.svelte, Sidebar.svelte (gỡ 2 mục cũ)`.
- **Flaw:** Three factual errors compound.
  - `XRay.svelte` is a **Kubernetes** resource graph: it takes `namespace` as a prop, imports `k8sApi`, and renders ingresses/services/deployments/pods (`src/pages/XRay.svelte:3-15`). Plan A phase 4's node set is container/network/volume/compose/image, and its own risk table says "k8s resources KHÔNG thuộc phase này".
  - `ClusterTopology.svelte` renders **colima instances** split into masters/workers by name substring (`src/pages/ClusterTopology.svelte:3-11`) — an instance-level view, not a container graph. It is arguably closer to Plan B phase 5 than to Plan A phase 4.
  - Neither is a route. Both are child components of the Kubernetes page: `src/pages/Kubernetes.svelte:13-14` (imports) and `:577`, `:579` (usage). Grep for these names across `src/` returns only `Kubernetes.svelte`. There are no sidebar entries or `App.svelte` routes to remove.
- **Failure scenario:** Implementer follows step 8, deletes both files, and the Kubernetes page fails to compile; or, having fixed the imports, ships a release where the k8s X-Ray tab and cluster topology tab are simply gone — a functional regression in an area the phase explicitly declared out of scope, with the deletion justified by a "merge" that never absorbed the deleted behaviour.
- **Suggested fix:** Remove all delete/merge language from phase 4. The new `Topology.svelte` is a **third, docker-scoped** page; state that plainly and accept it. If de-duplication is genuinely wanted, that is a separate k8s-scoped decision.

## Finding 6: `tauri-plugin-dialog` is claimed as present; it does not exist in the project

- **Severity:** High
- **Location:** Plan A, Phase 1, Architecture ("dialog chọn path (tauri-plugin-dialog, **đã có trong capabilities**)").
- **Flaw:** `src-tauri/capabilities/default.json` lists only `core:default`, `opener`, `updater`, `deep-link`, `core:event/window` permissions and an `http:default` allowlist — no `dialog:` permission. `src-tauri/Cargo.toml` has only `tauri-plugin-opener` (:23), `-http` (:41), `-updater` (:42), `-deep-link` (:45). `package.json:17-21` has the matching four JS packages and no `@tauri-apps/plugin-dialog`.
- **Failure scenario:** Phase 1 begins, the native file picker does not exist, and the implementer falls back to a free-text path input. That directly amplifies the phase's own stated worst risk ("đường dẫn phía container do người dùng gõ tay"), and the host-side path is now hand-typed too. The fallback also breaks the success criterion "Copy được file 100 MB vào và ra container" via an unplanned UX.
- **Suggested fix:** Add "install `tauri-plugin-dialog` (Cargo + npm + `capabilities/default.json` permission entry)" as step 0 of phase 1, and note the same for `tauri-plugin-notification` in Plan B phase 3 — phase 3 step 1 says "check capabilities" but the Related Code Files row already pre-commits to `notify-rust`, and macOS notification delivery also needs the signed/notarized bundle to be considered.

## Finding 7: Collector subscriber-counting and Pro sink registration both assume state lifetimes the app does not have

- **Severity:** High
- **Location:** Plan A, Phase 3, Architecture + Implementation steps 3 and 5, Success Criteria row 1; Plan B, Phase 2, step 7 and Success Criteria "Tài khoản Free: sink không được đăng ký, `metrics.db` không tồn tại".
- **Flaw (Scope Auditor lifetime classification):**
  - `MetricsCollector` as designed is **process-global** (instantiated once in `lib.rs` setup, per phase 3's Related Code Files). Its sinks are therefore process-lifetime.
  - Entitlement is **per-call and disk-backed**, not process-global: `subscription_state` re-reads `cache::load()` on every invocation (`src-tauri/src/subscription/mod.rs:116-118`), and `pro::detect()` re-handshakes the sidecar on every `pro_status` call (`src-tauri/src/pro/mod.rs:56`, `pro/bridge.rs:99`). Registering the SQLite sink once at startup based on entitlement bakes a request-scoped answer into process-scoped state: a user who buys Pro mid-session records nothing until restart, and a user whose cache expires mid-session keeps writing forever.
  - Step 5's fallback ("suy ra từ SSE connection lifecycle nếu `sse.rs` đã theo dõi được số connection") is not available: `sse.rs` holds a single `OnceLock<broadcast::Sender<SseMessage>>` (`src-tauri/src/sse.rs:17`) with `get_sse_tx()`/`publish_sse_event()` (`:19`, `:29`) and no connection registry.
  - Dual-mode leak: the app also serves the same routes over HTTP on 11420 (`src-tauri/src/api_server.rs:279-282`). A browser tab subscribed to SSE is a consumer with no relation to the Tauri window's "rời trang" event, so the success criterion "Đóng trang Activity → collector dừng hẳn" is untestable as written and false in browser mode.
- **Failure scenario:** User closes the Activity tab in the Tauri window while a stale browser tab (or a dropped-but-not-yet-reaped broadcast receiver) still holds a subscription; the collector runs `docker stats` every 2 s indefinitely and the app burns CPU in the background — the exact outcome the phase says is a hard acceptance gate.
- **Suggested fix:** Specify the subscriber guard as an explicit `subscribe()/Drop` registry owned by the collector (not inferred from SSE), covering both transports; and make the Pro sink a **runtime-toggleable** sink whose write path consults current entitlement per batch, rather than a startup-time registration decision.

## Finding 8: Crash-loop detection has no event source — `docker_state.rs` debounces events away

- **Severity:** High
- **Location:** Plan B, Phase 3, Implementation step 4 ("cần subscribe stream sự kiện container qua `docker_state.rs`"); Plan B, Phase 4, step 2 (same claim for `Unhealthy`, `CrashLoop`, `OomKilled`, `VmUnresponsive`); Phase 3 Success Criteria "Crash-loop rule bắt được container restart 5 lần trong 2 phút".
- **Flaw:** `start_docker_watcher` consumes the bollard event stream **internally** and exposes nothing subscribable (`src-tauri/src/docker_state.rs:62`, stream at `:122`). It applies a 500 ms trailing-edge debounce that deliberately collapses bursts and then refetches state — "when events arrive in rapid bursts (e.g., docker stop…) …keep resetting the timer while events keep coming" (`docker_state.rs:118-155`). Individual events, their `Action` (`die`, `start`, `oom`), and their timestamps are discarded; only a post-debounce state snapshot survives.
- **Failure scenario:** A container in a tight crash loop restarts 5 times in 8 seconds. The watcher coalesces all of it into one debounced refresh. The `CrashLoop` trigger observes one state change, never fires, and phase 4's rule 2 ("crash-loop → dừng hẳn") never engages — which is precisely the case where the automatic action matters most. `OomKilled` is worse: OOM is an event action with no lasting state, so it is unobservable through the current path entirely.
- **Suggested fix:** Add an explicit step to Plan B phase 3 (before the evaluator work): extend `docker_state.rs` with a *pre-debounce* event fan-out (`broadcast::Sender<DockerEvent>` carrying action + actor id + timestamp) and state that phases 3 and 4 depend on it. Also state which container `restart_count` source is authoritative (`docker inspect .RestartCount` is the cheaper alternative and should be named if chosen).

## Finding 9: The docker invocation layer assumed by phase B-5 (`docker --context`) does not exist

- **Severity:** Medium
- **Location:** Plan B, Phase 5, Overview ("đã có `instance_reader.rs` + `adapters/colima.rs`"), Architecture (`docker --context <from> save … | docker --context <to> load`), step 1 and step 3; also Plan A Phase 1's assumption that `adapters/docker.rs` can spawn transfers.
- **Flaw:** There is no shared docker command layer to add a `--context` flag to. `docker_output` is duplicated as a **private** helper in four separate modules: `commands/containers.rs:51`, `commands/volumes.rs:37`, `commands/networks.rs:25`, `commands/compose.rs:16`. Nothing under `src-tauri/src/adapters/` mentions `context` (grep for `context` across `adapters/*.rs` returns nothing). `instance_reader.rs` does carry `arch` (`:34`, `:178-181`) — that part of the claim holds — but the "docker context" half is asserted, not verified; the file's only context handling is a **kubectl** context for a k3s check (`instance_reader.rs:204-213`), which is a different concept.
- **Failure scenario:** Phase 5 begins, finds no context-aware invocation path, and either bolts `--context` onto a fifth private copy of `docker_output` (a fifth duplicate, and the "đừng viết bản thứ hai" instruction in step 5 is violated in the same phase that states it) or opens an unplanned refactor of four modules mid-phase. Additionally, `save | load` piped between two processes gives no byte counter without an interposed reader, and "huỷ giữa chừng: không để lại image dở dang ở đích" requires defining who kills which half of the pipe — neither is specified.
- **Suggested fix:** Add step 0 to phase 5: consolidate `docker_output` into one context-aware helper (`docker_output_ctx(context: Option<&str>, args)`), and record it as a shared prerequisite with Plan A phase 1. Specify the progress/cancel mechanism for the pipe explicitly (interposed counting reader, kill order, destination cleanup on non-zero exit at either end).

## Finding 10: Plan B declares a dependency that is not satisfied, and the rusqlite "no new dependency" claim understates the work

- **Severity:** Medium
- **Location:** Plan B, `plan.md` frontmatter `blockedBy: [… 260811-1930-subscription-teams-supabase-schema]` and Dependencies ("Không phase nào ở đây bắt đầu trước khi entitlement chạy được end-to-end"); Plan B, Phase 2, Overview ("`rusqlite` đã có trong `Cargo.toml`… Không thêm dependency").
- **Flaw:** The blocking plan is `status: in-progress` (`plans/260811-1930-subscription-teams-supabase-schema/plan.md:4`) with phase 5 (`phase-05-client-entitlement-rewrite.md`) and phase 7 (`verify-harden`) still outstanding — consistent with Findings 1 and 2. Sequencing Plan B behind it is correct, but the Plan B phases were written as if resolution already works.
  On rusqlite: the dependency is indeed present (`src-tauri/Cargo.toml:38`, `version = "0.32", features = ["bundled"]`), so that literal claim holds. What does not hold is the implication that a usable DB pattern exists. The only precedent opens a connection with `Connection::open(&db_path).expect("Failed to open knowledge.db")` (`src-tauri/src/commands/knowledge_bank.rs:23-25`) — a panic on failure, no WAL, no pooling, no async story. Phase 2 wants WAL, a second database file, a long-lived writer task, a retention task, and concurrent UI reads. That is a new connection-management design (blocking rusqlite calls must not run on the async runtime without `spawn_blocking`), not a free ride on an existing dependency.
- **Failure scenario:** Implementer copies the `knowledge_bank.rs` pattern, calls rusqlite directly from the writer task on the tokio runtime, and every batch write stalls a runtime worker thread — reintroducing exactly the "writer chậm làm đứng UI live" failure the phase says the bounded channel prevents. Separately, a first-run disk error panics the app instead of degrading, because `.expect()` is the house pattern being copied.
- **Suggested fix:** Reword phase 2's dependency note to "rusqlite is present; the connection/threading pattern is new" and add explicit steps for WAL setup, `spawn_blocking` (or a dedicated OS thread) for all DB access, and non-panicking open failure. Keep the `blockedBy` edge and add a concrete gate: Plan B starts only when a paying account resolves a capability end-to-end on a clean machine.

---

## Cross-cutting observations

- Plan A `plan.md` "Nền tảng tái dùng" table is the root of Findings 3, 4, and 5: it lists four files as ready-to-reuse foundation, and three of them do not do what the table implies (`crash.rs` persists nothing, `redact.rs` covers one of four required secret classes, the two topology pages are k8s/instance views). A plan that opens with a false inventory produces phases that under-scope by whole features.
- Both plans repeatedly encode "check X before implementing" as an implementation step (A-4 step 1, B-3 step 1, B-5 step 1). Those checks were cheap and are done above; folding the answers into the plans removes the risk that an implementer resolves them by guessing.
- The Free/Pro boundary rule in Plan A ("không phase nào được import `pro.svelte.ts`") is sound and verifiable, and phase A-3's `MetricSink` seam is the right shape — but see Finding 7: the seam must be runtime-toggleable, not startup-bound, or it will not survive the entitlement lifecycle.

## Unresolved questions for the planner

1. Which entitlement source is authoritative for gating — the sidecar `capabilities` list or the Supabase `paid` flag? `src/lib/pro.svelte.ts` maintains both and documents them as different questions; no plan states the precedence.
2. Are container logs in scope for the diagnostic bundle at all? Requirements exclude env from the container list but say nothing about env leaking through log text.
3. Does deleting the k8s X-Ray / cluster topology tabs have any product intent behind it, or was the overlap inferred from filenames?
