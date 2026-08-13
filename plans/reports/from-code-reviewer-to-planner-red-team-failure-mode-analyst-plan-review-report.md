# Red Team Review — Failure Mode Analyst / Flow Tracer

Scope: `plans/260810-2258-colimaui-p0-polish-parity/*` and `plans/260810-2258-colimaui-commercial-foundation/*`.
Method: traced actual data/control flow in `src-tauri/src` and `src/lib/api` against each plan's claims. Advisory only; no plan file modified.

---

## Finding 1: "Chỉ thêm field optional" is impossible — `ColimaError` is a tagged enum with a `String` payload, and the TS client stringifies it

- **Severity:** Critical
- **Plan:** Plan 1 (p0-polish-parity)
- **Location:** Phase 1, "Architecture" + "Implementation Steps" step 1 + "Risk Assessment" row 1
- **Flaw:** The plan describes `ColimaError { kind, message, command, exit_code, stderr, hint }` as a struct and asserts compatibility is preserved by "adding optional fields, not renaming old ones". `ColimaError` is not a struct. It is `#[serde(tag = "type", content = "message")]` over six unit-payload variants, so the wire shape is `{"type":"CommandFailed","message":"<string>"}`. Adding `command`/`exit_code`/`stderr`/`hint` requires converting each variant to a struct variant, which changes `message` from a JSON string to a JSON object. That is a breaking wire change, not an additive one.
- **Failure scenario:**
  1. Dev converts `CommandFailed(String)` → `CommandFailed { message, command, exit_code, stderr, hint }`.
  2. Serialized IPC error becomes `{"type":"CommandFailed","message":{"message":"...","hint":"..."}}`.
  3. `client.ts` hits `if (err.type && err.message) throw \`[${err.type}] ${err.message}\`` — template-literal coercion of an object.
  4. Every existing UI catch block that shows the thrown value now displays `[CommandFailed] [object Object]`. `cargo test` and `pnpm test` stay green because no test asserts the rendered string. It only appears at runtime, in exactly the error paths this phase exists to fix.
- **Evidence:**
  - `src-tauri/src/error.rs:4-13` — `#[derive(Debug, Serialize, Clone)] #[serde(tag = "type", content = "message")] pub enum ColimaError { CommandFailed(String), ... }`
  - `src/lib/api/client.ts:39-43` — `catch (err: any) { if (err && typeof err === 'object' && err.type && err.message) { throw \`[${err.type}] ${err.message}\`; } throw err; }`
  - Plan quote (phase-01, step 1): "giữ tương thích serialize hiện tại (thêm field optional, không đổi tên field cũ) để không phá HTTP route."
- **Suggested fix:** State explicitly that this is a v2 error envelope, not an additive change. Introduce a new struct `AppError { kind, message, command, exit_code, stderr, hint }` returned alongside/instead of the enum, keep the old enum serialization for one release behind a `legacy` field, and make `client.ts` normalization the *first* step of the phase (not step 4) so no window exists where the UI renders `[object Object]`.

---

## Finding 2: Browser mode cannot receive structured errors at all — 115 route handlers flatten them to `e.to_string()`

- **Severity:** Critical
- **Plan:** Plan 1 (p0-polish-parity)
- **Location:** Phase 1, "Quyết định thiết kế" + "Related Code Files" + "Tests / Validation"
- **Flaw:** The phase's core architectural justification is: put classification in Rust because "HTTP browser-mode và Tauri IPC dùng chung backend — làm ở TS sẽ phải viết 2 lần." Traced, this is false. The HTTP layer does not share the error type: every route handler pattern-matches and calls `err(e.to_string())`, collapsing the error into a single `String` via the `Display` impl before it reaches the `ApiResponse` envelope. `client.ts` then reads only `json.error`. The `Related Code Files` list does not mention `src-tauri/src/routes/*` at all, and `Tests/Validation` asserts "browser mode hiện lỗi giống hệt desktop mode" — a criterion the described work cannot satisfy.
- **Failure scenario:**
  1. Phase 1 lands. `error_from_output` correctly produces `kind: NotRunning`, `hint: "Colima chưa chạy — bấm Start"`.
  2. Desktop mode shows the rich toast. Team demos it, ships.
  3. A browser-mode user (`:11420`) triggers the same failure. `api_remove_volume` returns `err(e.to_string())` → `{"success":false,"error":"Command execution failed: Cannot connect to the Docker daemon"}`.
  4. `client.ts` throws `new Error(json.error)`. `normalizeError()` receives a bare string with no `kind`, no `hint`, no `exit_code`. The browser user gets the pre-phase experience.
  5. Worse: the two modes throw *different types* — Tauri mode throws a **string**, HTTP mode throws an **Error**. `withErrorReport` must handle both or one path silently produces `undefined` fields.
- **Evidence:**
  - `grep -rn "err(e.to_string())" src-tauri/src/routes | wc -l` → **115**
  - `src-tauri/src/routes/volumes.rs:10-14, 18-24, 28-35` — `Err(e) => err(e.to_string())` on every handler
  - `src/lib/api/client.ts:71-74` — `if (!json.success) { throw new Error(json.error || "API call failed"); }`
  - `src/lib/api/client.ts:40` throws a template string; line 73 throws an `Error` — divergent throw types across modes.
- **Suggested fix:** Add an explicit sub-task: change `ApiResponse`'s error channel to carry the structured error object, and mechanically rewrite all 115 `err(e.to_string())` sites (or introduce `impl IntoResponse for ColimaError` so routes use `?`). Re-estimate the phase — the plan's 4-6 days assumes 7 command modules; the real blast radius is 7 command modules + 17 route modules + the response envelope + both client throw paths.

---

## Finding 3: Reusing `instance_reader`'s `ColimaConfig` to write `colima.yaml` will silently delete the user's configuration

- **Severity:** Critical
- **Plan:** Plan 1 (p0-polish-parity)
- **Location:** Phase 5, "Architecture" (`read_instance_config` → "tái dùng instance_reader.rs") + step 2 + "Risk Assessment" row 1
- **Flaw:** The plan says round-trip safety will come from `#[serde(flatten)]` on `instance_reader.rs`'s config struct. That struct is `Deserialize`-only, has **no** `Serialize`, no `flatten`, and is documented in-source as *"Partial deserialize of colima.yaml — only the fields we need"* covering exactly six keys: `cpu, memory, disk, arch, runtime, kubernetes, hostname`. Real `colima.yaml` also carries `mounts`, `mountType`, `mountInotify`, `network`, `vmType`, `rosetta`, `provision`, `env`, `docker`, `sshPort`, `autoActivate`, `forwardAgent`. None survive. Additionally `serde_yml::from_str(&content).unwrap_or_default()` swallows parse errors into an all-zero config — so a config the parser trips on becomes `cpu: 0, memory: 0, disk: 0` in the UI, and writing that back is catastrophic.
- **Failure scenario:**
  1. User has `mounts: [{location: ~/work, writable: true}]` and `network: {address: true}`.
  2. Colima ships a new key the struct doesn't know; or the file has a tab, or an anchor `serde_yml` mishandles → `unwrap_or_default()` returns zeros. UI form displays CPU 0 / RAM 0.
  3. User bumps CPU to 4 and clicks Apply. `.bak` is written (of the good file), then the serialized struct is atomically renamed into place.
  4. `colima.yaml` now contains six keys. Mounts gone, network gone, k8s version gone. `colima start` boots without the user's mounts — their containers can't see `~/work`. The atomic write guaranteed the *corruption* was written atomically.
  5. The `.bak` exists, but nothing in the plan surfaces it in UI or offers restore; the validation test ("ghi config hỏng có chủ đích → bị chặn") tests malformed *input*, not field loss, so it passes.
- **Evidence:**
  - `src-tauri/src/instance_reader.rs:24-42` — `/// Partial deserialize of colima.yaml — only the fields we need.` `#[derive(Deserialize, Default)] struct ColimaConfig { cpu, memory, disk, arch, runtime, kubernetes, hostname }` (no `Serialize`, no `flatten`)
  - `src-tauri/src/instance_reader.rs:80-82` — `serde_yml::from_str(&content).unwrap_or_default()` — parse failure silently becomes an all-zero config
  - Plan quote (phase-05, step 2): "giữ **field chưa biết** khi round-trip ... (dùng `#[serde(flatten)]` cho phần dư)."
- **Suggested fix:** Do not reuse the reader struct for writes. Specify: parse to `serde_yml::Value`, mutate only the touched keys, re-serialize the `Value` — no typed round-trip. Make parse failure a hard error that disables the editor (never `unwrap_or_default` into the write path). Add a test asserting an unknown key + `mounts` survive a write. Also address the concurrency gap the plan does not mention: `colima start` rewrites `colima.yaml` itself, so read-modify-write must re-stat mtime immediately before rename and abort on change, and the editor must be blocked while the instance is transitioning.

---

## Finding 4: Phase 2's capability caching rests on two wrong premises about existing code

- **Severity:** High
- **Plan:** Plan 1 (p0-polish-parity)
- **Location:** Phase 2, "Non-functional" + "Architecture" diagram + "Ghi chú"
- **Flaw:** Three verifiable claims fail:
  (a) `SYSTEM_INFO_CACHE` is not in `commands/system.rs`; it is in `helpers.rs`, is a single global for one concrete type (`system::SystemInfo`), and its TTL is hardcoded at **300s**, not the "~30s" the plan assumes. It is shared by `/api/system` and `/api/version`, so retuning it for capabilities changes unrelated endpoints.
  (b) `TimedCache` exposes only `get_or_init`. There is **no** invalidate/clear method, so "invalidate khi instance đổi trạng thái" requires modifying the shared cache type, which the plan's file list doesn't include.
  (c) It is a `std::sync::Mutex` whose guard is held across `load_system_info()`, which spawns three blocking child processes — inside `async` axum handlers with no `spawn_blocking`. The plan's non-functional requirement "Detect chạy bất đồng bộ, không chặn render" is contradicted by the very mechanism it says to reuse.
- **Failure scenario:**
  1. Capabilities detection is added on top of `SYSTEM_INFO_CACHE`, now spawning ~5 processes (`colima`, `docker`, `limactl`, `kubectl`, `docker compose`).
  2. Cold start: three page components mount and each calls `get_system_capabilities`. Request A takes the std Mutex and blocks a tokio worker for ~1.5s spawning processes. Requests B and C block on the same std Mutex, also on tokio workers.
  3. With few worker threads, the SSE stream and `/api/containers` starve behind them. The user sees a frozen UI on the exact first-run path this phase is meant to smooth.
  4. Separately: user installs Docker mid-session, clicks "Refresh". Nothing changes for up to 300s because there is no invalidate path — and `load_system_info` only reports installed/not-installed, so the plan's required `installed_not_running` state cannot be produced from it at all.
- **Evidence:**
  - `src-tauri/src/helpers.rs:130-131` — `pub static SYSTEM_INFO_CACHE: LazyLock<Mutex<TimedCache<system::SystemInfo>>> = ... TimedCache::new(Duration::from_secs(300))`
  - `src-tauri/src/helpers.rs:102-128` — `TimedCache` has only `new` and `get_or_init`; no invalidation
  - `src-tauri/src/routes/system.rs:12-18, 21-27` — `SYSTEM_INFO_CACHE.lock().map(|mut cache| cache.get_or_init(load_system_info))` inside `pub async fn`, no `spawn_blocking`
  - `src-tauri/src/helpers.rs:134-140` — `load_system_info` returns only `*_installed` / `*_version`; no running state, no `kubectl`, no `nerdctl`, no `compose`
  - Plan quote (phase-02): "Kết quả detect được cache (tái dùng `SYSTEM_INFO_CACHE` sẵn có trong Rust)" and diagram "`src-tauri/src/commands/system.rs └─ get_system_capabilities() ... (dùng SYSTEM_INFO_CACHE, TTL ~30s; invalidate khi instance đổi trạng thái)`".
- **Suggested fix:** Specify a *new* capability cache (`tokio::sync::RwLock` + explicit `invalidate()`), detection running under `spawn_blocking` and in parallel, and correct the file path to `helpers.rs`. Explicitly state that `load_system_info` cannot express `installed_not_running` and must be replaced, not wrapped.

---

## Finding 5: `routes/capabilities.rs` is an AI-agent endpoint manifest, not tool detection — "extend it (DRY)" merges two unrelated contracts

- **Severity:** High
- **Plan:** Plan 1 (p0-polish-parity)
- **Location:** Phase 2, "Ghi chú" + "Related Code Files"
- **Flaw:** The plan instructs: "`src-tauri/src/routes/capabilities.rs` đã có sẵn — kiểm tra và mở rộng thay vì tạo route mới (DRY)." The file is 35 lines returning a **static JSON document describing the HTTP API surface for AI agents** — `{"version":"1.0","description":"ColimaUI capabilities schema for AI Agents","endpoints":{...},"event_bus":{...}}`. It has nothing to do with detecting whether `colima`/`kubectl` is installed. "Capability" is an overloaded word and the plan matched on the name, not the behavior.
- **Failure scenario:**
  1. Implementer follows the instruction and adds `tools: [{id:"colima", state:"missing"}]` into the same payload, or worse, replaces the body.
  2. `/api/capabilities` is the discovery contract for AI agents (self-described as such, and the shell-sandbox / event-bus safety categories `DANGEROUS`/`SAFE` live in it). An agent that fetched the schema to decide which endpoints are safe now gets a payload whose shape changed, or a static doc that suddenly varies per host.
  3. Failure is silent — the endpoint has no test and no in-repo consumer to break at build time; only `api_server.rs:217` routes it.
- **Evidence:**
  - `src-tauri/src/routes/capabilities.rs:5-34` — full body is a `json!` literal: `"description": "ColimaUI capabilities schema for AI Agents"`, `"endpoints"`, `"event_bus"` with `"category": "DANGEROUS"` markers
  - `src-tauri/src/api_server.rs:217` — `.route("/api/capabilities", get(api_capabilities))` — the only registration; no other in-repo consumer, so the contract's consumers are external agents
- **Suggested fix:** Correct the plan: create a distinct route (`/api/system/tools` or similar) and leave `/api/capabilities` untouched. Remove the "DRY / mở rộng thay vì tạo route mới" instruction, which here actively causes harm.

---

## Finding 6: Tray cannot express `Transitioning` or `Error`, and its state source is a filesystem heuristic polled every 5s — stale menu items act on the wrong state

- **Severity:** High
- **Plan:** Plan 1 (p0-polish-parity)
- **Location:** Phase 3, "Architecture" (aggregate state `Error > Transitioning > Running > Stopped`) + "Success Criteria" ("Start/stop instance từ tray hoạt động, kết quả phản ánh trong ≤3s")
- **Flaw:** The aggregate state ladder requires per-instance `Error` and `Transitioning`. The only instance data feeding the Tauri event pipeline is `instance_reader::list_instances_fast()`, whose status is a binary derived from *file existence* (`ha.sock` or `ha.pid`) — `"Running"` or `"Stopped"`, nothing else. No error state, no transitioning state exists to aggregate. Additionally the poller emits **unconditionally** every 5s with no diffing, and the SSE publisher is a *separate* 2s loop with its own diffing — two independent pipelines the plan treats as one. The ≤3s success criterion is unachievable against a 5s tick.
- **Failure scenario:**
  1. User clicks tray → "Stop default". `colima stop` begins; lima tears down.
  2. `ha.pid` still exists for several seconds. The next poll tick still reports `Running`. Tray rebuilds with "Stop default" still enabled and icon still green.
  3. Impatient user clicks "Stop default" again. A second `colima stop` runs concurrently against a half-torn-down VM. There is no in-flight guard described anywhere in the phase.
  4. Symmetric and worse on start: `ha.sock` appears early in `colima start`, so tray reports `Running` while the Docker daemon is not yet reachable. The tray's "số container đang chạy" reads from the docker watcher, which is in its reconnect path — the menu shows "Running / 0 containers", indistinguishable from a genuinely empty running instance.
  5. Best case the user is confused; worst case double-invocation leaves a wedged lima instance.
- **Evidence:**
  - `src-tauri/src/instance_reader.rs:71-74` — `fn is_instance_running(lima_dir: &Path) -> bool { lima_dir.join("ha.sock").exists() || lima_dir.join("ha.pid").exists() }`
  - `src-tauri/src/instance_reader.rs:~105-110` — `status: if running { "Running" } else { "Stopped" }` — only two states exist
  - `src-tauri/src/poller.rs:39` — `interval(Duration::from_secs(5))`; `:61` — `handle.emit("instances-update", ...)` with no change comparison
  - `src-tauri/src/sse.rs:195-208` — a *second*, independent instance publisher on a 2s loop with its own `last_json` diffing
  - `src-tauri/src/docker_state.rs:115, 198-199` — `docker-reconnected` / `docker-connection-lost` are the only signals resembling an error state, and they are container-level, not instance-level
- **Suggested fix:** Add an explicit step to introduce a transition-tracking layer (record user-initiated start/stop with a pending flag and timeout) before the tray consumes it; disable the menu item while pending; drop or restate the ≤3s criterion against the 5s poll; and note that two event pipelines exist so the tray does not accidentally double-subscribe.

---

## Finding 7: Container labels are dropped in **two** independent mappers; Phase 4's file list names only one

- **Severity:** High
- **Plan:** Plan 1 (p0-polish-parity)
- **Location:** Phase 4, "Nguồn dữ liệu nhóm" + step 1 + "Related Code Files"
- **Flaw:** The plan hedges correctly ("Cần xác nhận ... có trả labels — nếu chưa thì bổ sung") but lists only `docker_state.rs` and `commands/containers.rs` as the places to fix. Traced: `commands/containers.rs` delegates to `docker_state::map_containers`, so those are one site — while `sse.rs` contains a **separate, inline** container mapper for the browser pipeline that the plan never mentions. Fixing the named files leaves browser mode without labels.
- **Failure scenario:**
  1. Dev adds `"Labels": c.labels` to `docker_state::map_containers`. Desktop grouping works. `composeGrouping.test.ts` passes (it is pure and fed fixtures).
  2. Browser mode at `:11420` receives SSE payloads built by the mapper at `sse.rs:152`, which still has no `Labels` key.
  3. `groupContainersByProject` sees `undefined` labels for every container and files all of them under "Standalone". No error, no toast — the feature just silently does nothing in one of the two supported modes, and no automated test covers it.
- **Evidence:**
  - `src-tauri/src/docker_state.rs:210-253` — `map_containers` emits `Id, Names, Image, Status, State, Ports, CreatedAt, Size, Command` — no `Labels`
  - `src-tauri/src/docker_state.rs:324` and `src-tauri/src/commands/containers.rs:119` — both call `map_containers` (single shared site)
  - `src-tauri/src/sse.rs:152` — `mapped_containers.push(serde_json::json!({ ... }))` — an independent second mapper in the browser path
  - Plan quote (phase-04): "Modify: `src-tauri/src/docker_state.rs`, `src-tauri/src/commands/containers.rs` — đảm bảo trả `labels`"
- **Suggested fix:** Add `src-tauri/src/sse.rs` to the file list, or better, make `sse.rs` call `docker_state::map_containers` so one mapper serves both modes. Add a validation step that checks the browser-mode SSE payload, not just the desktop event.

---

## Finding 8: Sidecar and core are updated through different channels — auto-update will strand paying users on a version-skewed handshake

- **Severity:** Critical
- **Plan:** Plan 2 (commercial-foundation)
- **Location:** Phase 1 "Giao thức" / "Risk Assessment"; Phase 4 "Auto-update"; plan.md Unresolved Question #3
- **Flaw:** Phase 1's failure handling for protocol skew is "Core từ chối handshake nếu `protocol_version` không tương thích, và nói rõ cần cập nhật bên nào." That message is the entire recovery plan — but there is no mechanism by which a user can update the sidecar. Phase 4 adds `tauri-plugin-updater`, which replaces the **app bundle**. The sidecar is a separately-signed binary from a private repo, and plan.md Q3 ("kèm trong bundle hay tải riêng?") is still open — meaning the plans commit to an auto-update design in Phase 4 before deciding what it updates. Nothing in the repo supports either path today: `tauri.conf.json` has no `externalBin`, no updater plugin is in `Cargo.toml`, and the only `[[bin]]` is a CLI gated behind `required-features = ["cli"]`.
- **Failure scenario:**
  1. Paying user runs core v1.4 + sidecar v1.4 (protocol v1), activated Pro, offline-capable.
  2. Auto-update ships core v1.5 with protocol v2. The updater replaces the app bundle. If the sidecar was distributed out-of-band (Q3 unresolved), the on-disk sidecar stays at v1.4.
  3. On next launch `bridge.rs` handshakes, gets protocol v1, and refuses. Per Phase 1 the core "runs fine without the sidecar" — so it degrades to Free.
  4. `license.status` is served *by the sidecar* (Phase 2 puts verification entirely in the private binary). With no sidecar reachable, the core cannot even tell the user they are Pro. A paying customer opens the app after a routine background update and finds every Pro feature gone, with a message telling them to update a binary they were never given a way to update.
  5. Phase 1's mitigation "tự khởi động lại tối đa N lần rồi bỏ cuộc" makes it worse: N restart attempts of a version-incompatible process is a restart storm at every launch.
- **Evidence:**
  - `src-tauri/Cargo.toml:23,40` — only `tauri-plugin-opener` and `tauri-plugin-http` present; no `tauri-plugin-updater`
  - `src-tauri/tauri.conf.json:34` — `"bundle": {` block contains no `externalBin`; grep for `externalBin`/`sidecar` returns nothing
  - `src-tauri/Cargo.toml:50-53` — `[[bin]] name = "colimaui" path = "src/bin/cli.rs" required-features = ["cli"]` — the only extra binary, feature-gated
  - Plan quotes: phase-01 "Core từ chối handshake nếu `protocol_version` không tương thích, và nói rõ cần cập nhật bên nào"; plan.md Q3 "Sidecar Pro phân phối thế nào ... (chưa chốt)"
- **Suggested fix:** Make Q3 a **blocking** decision on Phase 1, not an open question. If the sidecar ships inside the bundle via `externalBin`, core and sidecar version together and this whole class disappears — state that explicitly. Additionally: cache the last-known-good license state **in the core** (signed blob the core can read even without the sidecar) so a skew never presents as "you are not a customer"; and cap restart attempts per *process lifetime* with backoff, not per launch.

---

## Finding 9: Grace period, refresh, and `license.json` writes have no clock or crash safety — the failure mode locks out paying customers

- **Severity:** High
- **Plan:** Plan 2 (commercial-foundation)
- **Location:** Phase 2, "Requirements" (grace 14 ngày, refresh 7 ngày/lần) + step 5/7 + "Risk Assessment"
- **Flaw:** Every temporal guarantee is expressed in wall-clock terms with no monotonic anchor and no tamper detection. Separately, `store.rs` is specified as "hỏng file thì degrade về Free chứ không crash" but the write path is never specified as atomic — notably in contrast to Plan 1 Phase 5, which *does* call for temp-file + rename for a much less critical file. The asymmetry indicates the failure mode was not considered here.
- **Failure scenario A (crash mid-write):**
  1. Scheduled refresh succeeds; `store.rs` opens `~/.colima-ui/license.json` for truncating write.
  2. Machine sleeps/loses power/user force-quits between truncate and flush. File is zero-length or half-JSON.
  3. Next launch: parse fails → per spec, "degrade về Free". The user's license key is gone from disk. They are offline (the whole point of the offline-first design), so they cannot re-fetch it; the key only ever arrived by email (Phase 3). A paying customer is downgraded by a power failure.
- **Failure scenario B (clock):**
  1. User travels / NTP corrects a skewed RTC forward by 30 days, or a VM restores from a snapshot.
  2. `Grace{until}` and `Pro{until}` are evaluated against the new wall clock → instant `Expired`. Pro features vanish mid-work with no network to refresh against.
  3. Inverse: setting the clock back extends grace indefinitely. The plan accepts crack risk (principle #4), so this direction is fine — but it means grace cannot be relied on as a business control, which the phase does not acknowledge.
- **Failure scenario C (offline longer than grace):** A user on an air-gapped machine — a plausible Docker-tooling persona — activates once, then never reaches the network again. Requirements say "sau lần kích hoạt đầu, **offline vô thời hạn**", but refresh failure enters Grace at day 7 and Grace expires at day 21. Two requirements in the same phase contradict each other, and the stricter one silently wins.
- **Evidence:**
  - Plan quote (phase-02, Requirements): "Kích hoạt lần đầu cần mạng ...; sau đó **offline vô thời hạn** cho tới khi hết hạn subscription." vs step 7: "refresh định kỳ 7 ngày/lần, thất bại thì vào Grace" + "Grace period 14 ngày khi kiểm tra định kỳ thất bại"
  - Plan quote (phase-02, step 5): "`store.rs`: đọc/ghi 0600; hỏng file thì degrade về Free chứ không crash" — no atomicity specified
  - Contrast: `plans/260810-2258-colimaui-p0-polish-parity/phase-05-colima-config-knowledge-base.md` step 4 — "ghi atomic (ghi file tạm rồi rename)" is specified for `colima.yaml` but not for the license
  - Repo precedent that crash-safety is not currently a habit: `src-tauri/src/instance_reader.rs:80-82` uses `unwrap_or_default()` on a failed parse rather than surfacing corruption
- **Suggested fix:** Specify temp-file + `fsync` + rename for `license.json`, plus a `.bak` retained across writes and a "corrupt → attempt `.bak` → only then Free" ladder. Anchor grace on `max(wall_clock, last_seen_timestamp_persisted_in_license_json)` so backward clock moves cannot shorten it and forward jumps are bounded. Resolve the offline-forever vs grace-21-days contradiction explicitly in the requirements text.

---

## Finding 10: Purchase succeeds but the key never arrives — no in-app recovery path exists

- **Severity:** High
- **Plan:** Plan 2 (commercial-foundation)
- **Location:** Phase 3, "Architecture" (webhook → phát hành key → gửi email) + "Risk Assessment" row 2
- **Flaw:** Key delivery is single-channel: email. The plan hardens the *webhook* (signature verify, idempotency, retry queue) and its stated mitigation is "có công cụ phát hành key thủ công cho trường hợp khẩn" — a manual tool operated by the maintainer, requiring the customer to first realize something is wrong and find a support channel. The step *after* issuance — email delivery — has no retry, no bounce handling, and no alternative retrieval path. Phase 2's activation UI accepts only a pasted key; there is no "I paid, fetch my key" flow.
- **Failure scenario:**
  1. Customer pays. MoR fires `subscription.created`. Webhook verifies, is idempotent, issues and signs the key. Every success criterion in the phase is met.
  2. The transactional email lands in spam, or the provider soft-bounces a corporate MX, or the customer typo'd their email at checkout (MoR-hosted checkout — the app never validated it).
  3. Customer waits, then opens the app. `License.svelte` asks for a key they do not have. `UpgradeDialog` offers to sell them Pro again. The deep link requires a key. The MoR customer portal shows the subscription but not the license key (the key lives on the license server, not the MoR).
  4. Outcome: paid customer, zero Pro access, and their most likely next action is a chargeback — which then triggers `refund.created` and revokes the key they never received. Success criterion "Mua thành công → nhận email chứa key trong ≤2 phút" is measured on the happy path only.
- **Evidence:**
  - Plan quote (phase-03, Architecture): "Webhook → License Server → ... └─ gửi email chứa key" — the sole delivery arrow
  - Plan quote (phase-03, Requirements): "Trong app: nút 'Nâng cấp Pro' mở checkout, quay lại app thì tự nhận key nếu có thể (deep link)" — "nếu có thể" is the only fallback and it depends on the browser round-trip completing
  - Phase 2 activation surface is paste-only: "UI: nhập key, xem trạng thái license, xem còn bao nhiêu ngày, deactivate" (`phase-02`, Requirements) — no retrieval endpoint among the three specified (`/activate`, `/deactivate`, `/refresh`)
  - `src-tauri/Cargo.toml:23` — `tauri-plugin-opener` exists (checkout can open), but nothing supports a return channel; no deep-link scheme is registered in `tauri.conf.json` today
- **Suggested fix:** Add a fourth license-server endpoint: `POST /recover { email }` → re-sends the key to the address on file, rate-limited, plus a "Tôi đã mua nhưng chưa nhận được key" link directly in `License.svelte`. Track email send status in the webhook handler and alert on failures (the retry queue currently covers only webhook ingestion, not egress). Add an E2E case for "issuance succeeds, email fails" to the phase's test list.

---

## Cross-cutting note (not counted as a finding)

`API_BASE` is hardcoded to `http://127.0.0.1:11420` in `src/lib/api/client.ts:7`, while `api_server.rs:255` scans ports `11420..=11429` and binds the first free one. Any manual validation step phrased as "browser mode hiện lỗi giống hệt desktop mode" (Plan 1 Phase 1) or browser-mode SSE verification (Plan 1 Phase 4) is unreliable on a machine where 11420 is occupied — the tester will see a dead frontend and may attribute it to the change under review. Worth a one-line note in both phases' validation sections.
