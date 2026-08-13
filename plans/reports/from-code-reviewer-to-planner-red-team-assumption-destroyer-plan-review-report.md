# Red Team Review — Assumption Destroyer / Scope Auditor

Reviewed: `plans/260810-2258-colimaui-p0-polish-parity/*` and `plans/260810-2258-colimaui-commercial-foundation/*`.
Method: every "already exists / already works" claim grepped against the repo. Advisory only; no plan file modified.

## Verdict on the 14 audited claims

| # | Claim | Verdict |
|---|---|---|
| 1 | 164 catch / 2 toast files / 13 console.error | **TRUE** (verified exactly) |
| 2 | `tray-icon` enabled, no tray code in lib.rs | **TRUE** |
| 3 | `routes/capabilities.rs` is system capability detection | **FALSE** — it is an AI-agent API schema |
| 4 | `instance_reader.rs` "already reads instance config" | **MISLEADING** — 7 fields, lossy, read-only |
| 5 | `serde_yml` already a dependency | **TRUE** (`0.0.12`) |
| 6 | KB 883 lines, reusable for docs + compose matching | **PARTLY FALSE** — no article table, FTS only on `agent_memory` |
| 7 | `src/hooks/useHotkeys.ts` exists → hotkeys cheap | **FALSE** — it is React code in a Svelte repo, `react` is not a dependency |
| 8 | Compose labels available from backend | **FALSE** — neither code path returns labels |
| 9 | `src/lib/markdown.ts` can render KB articles | **PARTLY TRUE** — 83-line hand-rolled renderer, limited |
| 10 | `settingsStore.svelte.ts` can persist prefs | **PARTLY TRUE** — flat `string→string`, async, no schema |
| 11 | Sidecar keeps MIT boundary clean | **UNPROVEN** — browser mode and distribution unaddressed |
| 12 | Stable macOS fingerprint achievable | **UNPROVEN / risky** |
| 13 | Span-preserving YAML patching feasible | **FALSE with current deps** |
| 14 | Effort estimates credible | **NO** — see Finding 10 |

---

## Finding 1: `routes/capabilities.rs` is an AI-agent API schema, not system capability detection

- **Severity:** Critical
- **Plan:** Plan 1 (p0-polish-parity)
- **Location:** Phase 2, "Architecture" + "Ghi chú" + "Implementation Steps" step 1/3
- **Flaw:** The plan states `src-tauri/src/routes/capabilities.rs` already exists and instructs the implementer to "extend it rather than create a new route (DRY)". That file has nothing to do with detecting `colima`/`docker`/`limactl`/`kubectl` on the host. It is a static JSON blob describing the HTTP API surface and event-bus categories **for AI agents**.
- **Failure scenario:** The implementer opens the file expecting detection scaffolding, finds a hardcoded `json!` literal, and then either (a) bolts host-tool detection into an AI-agent discovery document — permanently coupling two unrelated contracts, so any future Pro capability added to the AI schema silently changes the empty-state UI payload — or (b) discovers the mismatch mid-phase and re-plans. The word "capabilities" is now overloaded a third time by Plan 2 Phase 1 (`pro/registry.rs` "năng lực Pro"), guaranteeing confusion.
- **Evidence:**
  - `src-tauri/src/routes/capabilities.rs:5-35` — `pub async fn api_capabilities()` returns `json!({ "version": "1.0", "description": "ColimaUI capabilities schema for AI Agents", "endpoints": {...}, "event_bus": {...} })`. 35 lines total, no process spawning, no PATH lookup, no `colima`/`kubectl` reference.
  - Plan text (`phase-02-empty-states-onboarding.md:46`): "`src-tauri/src/routes/capabilities.rs` đã có sẵn — kiểm tra và mở rộng thay vì tạo route mới (DRY)."
- **Suggested fix:** Drop the reuse instruction. Create a distinct route (`routes/system_capabilities.rs` or extend `routes/system.rs`) and rename the domain concept to avoid a three-way collision with the AI schema and Plan 2's Pro capability registry.

---

## Finding 2: `useHotkeys.ts` is React code in a Svelte 5 repo and cannot be used

- **Severity:** High
- **Plan:** Plan 1 (p0-polish-parity)
- **Location:** Phase 1, "Implementation Steps" step 7 ("dùng `src/hooks/useHotkeys.ts` sẵn có")
- **Flaw:** The plan treats the file's existence as evidence that a hotkey mechanism is available. The file imports from `react`, which is not in `package.json`, and nothing in the codebase imports it. It is dead code that does not compile in this stack.
- **Failure scenario:** The implementer writes `import { useHotkeys } from "../hooks/useHotkeys"` in a `.svelte` file to open `ErrorLogPanel`. Vite fails to resolve `react`; if someone "fixes" it by installing React, a 40 MB dead dependency lands in the bundle. More likely: the implementer stops, writes a Svelte `$effect`-based replacement, and the phase absorbs unplanned work — and Phase 4 (which also assumes cheap keyboard handling for table interactions) inherits the same surprise.
- **Evidence:**
  - `src/hooks/useHotkeys.ts:1` — `import { useEffect } from "react";`
  - `src/hooks/useHotkeys.ts:9-11` — `export function useHotkeys(map: HotkeyMap) { useEffect(() => {`
  - `grep -n 'react' package.json` → no match. `grep -rn 'useHotkeys' src --include='*.svelte' --include='*.ts'` outside the file itself → no match.
- **Suggested fix:** Add an explicit sub-task: delete `src/hooks/useHotkeys.ts` and write a Svelte 5 rune-based hotkey helper. Budget it. Also treat this file as a warning sign that other "already exists" artifacts in the repo may be non-functional leftovers.

---

## Finding 3: `ColimaError` cannot be extended "additively" — the proposed change breaks every error consumer

- **Severity:** Critical
- **Plan:** Plan 1 (p0-polish-parity)
- **Location:** Phase 1, "Implementation Steps" step 1 and "Risk Assessment" row 1
- **Flaw:** The plan asserts backwards compatibility is achievable by "adding optional fields, not renaming old fields" (`phase-01:73`, and the mitigation "Chỉ thêm field optional"). This is impossible given the actual type. `ColimaError` is an enum of **newtype variants** serialized with `#[serde(tag = "type", content = "message")]`, so the wire shape is `{"type":"CommandFailed","message":"<string>"}`. Adding `command`/`exit_code`/`stderr`/`hint` requires converting variants to struct variants, at which point `message` becomes an **object**, not a string. Every consumer that read `message` as a string breaks.
- **Failure scenario:** After the Rust refactor, `src/lib/api/client.ts:41` executes `throw \`[${err.type}] ${err.message}\`` and produces `[CommandFailed] [object Object]`. That thrown value is a **string**, and all 164 existing catch sites downstream consume strings. The result is that every error message in the app degrades to `[object Object]` — the exact opposite of the phase's goal — and it will pass any test suite that only asserts "an error was thrown". The plan's own acceptance criterion "HTTP route cũ không vỡ" is therefore unverifiable as written.
- **Evidence:**
  - `src-tauri/src/error.rs:4-12` — `#[derive(Debug, Serialize, Clone)] #[serde(tag = "type", content = "message")] pub enum ColimaError { CommandFailed(String), Validation(String), NotFound(String), Internal(String), Network(String), Unknown(String), }`
  - `src/lib/api/client.ts:40-41` — `if (err && typeof err === 'object' && err.type && err.message) { throw \`[${err.type}] ${err.message}\`; }` — structured error is flattened to a string at the boundary, so `normalizeError()` returning an `AppError` object is itself a breaking change to all 164 catch sites.
  - Verified counts: 164 catch blocks, 2 files referencing toast (`src/components/ToastContainer.svelte`, `src/lib/globalToast.ts`), 13 `console.error`.
- **Suggested fix:** Stop calling this backwards-compatible. Plan it as an explicit, versioned contract change: introduce a **new** `ColimaErrorV2` struct (flat, not an enum-with-content), keep the old serializer behind a compatibility shim for one release, and add a step that migrates `client.ts` from `throw <string>` to `throw <AppError>` together with a sweep of all catch sites that pattern-match on string content. Budget the sweep separately from the Rust work.

---

## Finding 4: Compose grouping has no data source — neither container list path returns labels, and the two paths disagree

- **Severity:** Critical
- **Plan:** Plan 1 (p0-polish-parity)
- **Location:** Phase 4, "Architecture" → "Nguồn dữ liệu nhóm", and "Implementation Steps" step 1
- **Flaw:** The plan defers verification ("Cần xác nhận `docker_state.rs` / `commands/containers.rs` có trả labels — nếu chưa thì bổ sung"). The verification fails. `map_containers` builds an explicit JSON object with 9 keys and **labels is not one of them**. Worse, there are two independent container list paths with different label representations, and the plan treats them as one.
- **Failure scenario:** Step 1 fails, so the entire phase's headline feature (grouping, the OrbStack-parity item) is blocked behind a backend change the plan allocated no time for. When the implementer adds labels, they hit a shape mismatch: the Bollard path yields `HashMap<String,String>`, while the CLI path (`docker ps --format json`) yields `Labels` as a single comma-joined `"k=v,k=v"` string. `groupContainersByProject()` — written as a "pure, easy to test" function against one shape — will silently produce a single bogus "Standalone" group whenever the app falls back to the CLI path. That fallback is invisible to the user, so the bug looks intermittent.
- **Evidence:**
  - `src-tauri/src/docker_state.rs:210-254` — `map_containers` emits exactly `Id, Names, Image, Status, State, Ports, CreatedAt, Size, Command`. `grep -in 'label' src-tauri/src/docker_state.rs` → no match.
  - `grep -in 'label' src-tauri/src/commands/containers.rs` → no match (773 lines).
  - Two paths: `src-tauri/src/commands/containers.rs:119` uses `crate::docker_state::map_containers(&containers)` (Bollard), while `src-tauri/src/commands/containers.rs:71-72` `list_containers_cli` and `:132` shell out to `docker ps --format json --no-trunc`.
  - `src/lib/api/types.ts:86,96` show the established repo convention for this data — `Labels: string` on `DockerVolume`/`DockerNetwork` — i.e. the comma-joined CLI form, and there is no `Container` interface carrying labels at all.
- **Suggested fix:** Promote "add labels to both container paths and normalize them to one shape in Rust" to an explicit, estimated step 0 of Phase 4 (or fold it into Phase 1's Rust work). Normalize to `HashMap<String,String>` **in Rust** so the TS grouping function sees one shape; add a Rust unit test that both paths produce identical label output for the same container.

---

## Finding 5: `instance_reader.rs` cannot be "reused" for config writing — it is a lossy 7-field read that swallows parse errors

- **Severity:** Critical
- **Plan:** Plan 1 (p0-polish-parity)
- **Location:** Phase 5, "Overview" ("`instance_reader.rs` đã đọc được cấu hình instance"), "Related Code Files" ("Modify: `instance_reader.rs` — tái dùng logic đọc config"), "Implementation Steps" steps 1-2
- **Flaw:** The plan's form covers CPU, memory, disk, runtime, VM type, mount type, DNS, network address, Kubernetes on/off **and version**. The existing struct parses seven fields: `cpu, memory, disk, arch, runtime, kubernetes.enabled, hostname`. There is no `vmType`, no `mountType`, no `dns`, no `network`, no `kubernetes.version`. So roughly half the form has no existing parsing. Worse, the reader is explicitly documented as a *partial* deserialize and calls `unwrap_or_default()` on the YAML parse — a malformed or unexpected `colima.yaml` yields a struct of zeros with **no error signal**.
- **Failure scenario:** An implementer following "tái dùng logic đọc config" reuses `ColimaConfig` as the write model. Round-trip serialization then writes back a file containing only those 7 keys, silently deleting every other key in the user's `colima.yaml` — `mounts`, `provision`, `env`, `sshPort`, `docker` daemon config. The plan's own top risk ("Ghi hỏng config làm instance không khởi động được") fires on day one. The `#[serde(flatten)]` mitigation in step 2 is proposed but cannot be retrofitted onto the existing struct without also fixing the `unwrap_or_default()`, since a parse failure would still silently produce an empty flatten map and then write it.
- **Evidence:**
  - `src-tauri/src/instance_reader.rs:24-48` — `/// Partial deserialize of colima.yaml — only the fields we need.` then `struct ColimaConfig { cpu: u32, memory: u32, disk: u32, arch: String, runtime: String, kubernetes: KubernetesConfig, hostname: String }`; `struct KubernetesConfig { enabled: bool }` — no `version` field.
  - `src-tauri/src/instance_reader.rs:83` — `serde_yml::from_str(&content).unwrap_or_default()` — a corrupt/unknown-schema YAML silently becomes all-zeros.
  - Confirms claim 5: `src-tauri/Cargo.toml:29` — `serde_yml = "0.0.12"`.
- **Suggested fix:** Make it explicit that Phase 5 creates a **separate write-model** type in `colima_config.rs` and does not reuse `instance_reader::ColimaConfig`. Add a hard requirement: the write path parses to `serde_yml::Value` (or a struct with `#[serde(flatten)] rest: BTreeMap<String, Value>`) and **must return Err on parse failure**, never `unwrap_or_default()`. Add a test that a `colima.yaml` containing `mounts`/`provision`/`env` survives a read-modify-write byte-comparably for untouched keys.

---

## Finding 6: The Knowledge Bank has no article storage and no full-text index on solutions — "reuse it for docs" understates the work

- **Severity:** High
- **Plan:** Plan 1 Phase 5 and Plan 2 Phase 5
- **Location:** Plan 1 Phase 5 "Overview" + "Quyết định" ("dùng lại `knowledge_bank.rs` + SQLite thay vì dựng hệ thống docs mới — đã có schema, feedback loop"); Plan 2 Phase 5 "Kiến trúc lai" Tầng 2
- **Flaw:** The line count (883) and feedback loop are real, but the schema does not support either proposed new use. There is no articles/docs table; the `solutions` table has no title, no body/markdown, no platform, no version, no `docs_url` slug — the columns are `error_pattern, error_category, solution_text, root_cause, commands, likes, dislikes, source`. And the FTS5 virtual table indexes **`agent_memory` only**, not `solutions`. So "Help offline, tìm kiếm được, ≥5 bài" (Plan 1 Phase 5 success criterion) requires a new table, a new FTS index, new triggers, and a seeding/versioning mechanism — none of which is "reuse".
- **Failure scenario:** Plan 1 Phase 5 step 9 says "seed KB vào SQLite lúc khởi động (idempotent, có version)". There is no version column anywhere in `solutions`, so idempotent re-seeding on app upgrade has no key to dedupe on except `error_pattern` text equality — editing an article's wording on the next release creates a duplicate row instead of updating it, and search returns both. Separately, Plan 2 Phase 5's Tầng 2 "khớp chữ ký lỗi" would run `LIKE`/regex scans over `solutions` because no FTS index covers it, so the "<500ms" budget is asserted rather than designed. Note also the count is 23 builtin patterns, not "22+", which is fine — but it means the matching corpus is Colima/Lima VM boot errors (disk lock, QEMU, DNS), essentially **zero overlap** with compose-file syntax errors, so Tầng 2 contributes ~nothing to the Plan 2 flagship on day one.
- **Evidence:**
  - `src-tauri/src/commands/knowledge_bank.rs:30-42` — `CREATE TABLE IF NOT EXISTS solutions (id, error_pattern, error_category, solution_text, root_cause, commands, likes, dislikes, source, created_at, last_used_at)` — no title/body/platform/version.
  - `src-tauri/src/commands/knowledge_bank.rs:59-63` — `CREATE VIRTUAL TABLE IF NOT EXISTS agent_memory_fts USING fts5(content, ...)`; triggers at `:96-109` are all `ON agent_memory`. No FTS over `solutions`.
  - `src-tauri/src/commands/knowledge_bank.rs:557-567` — the only FTS query path is `SELECT content FROM agent_memory_fts WHERE agent_memory_fts MATCH ?1`.
  - Builtin content is VM-level, e.g. `:138` `r"failed to run attach disk.*in use"`, `:159` `r"qemu.*failed|qemu.*not found|qemu.*error"`, `:201` `r"lock.*acquired|flock.*failed"`, `:238` DNS resolution fix.
  - Feedback loop is real: `:455-459` `/// Record feedback (like/dislike) for a solution.` `pub async fn kb_feedback(solution_id: i64, is_like: bool)`.
  - Claim 9 context: `src/lib/markdown.ts` is 83 lines of hand-rolled line-by-line rendering (`:11` `export function renderMarkdown`, with `<think>` block handling at `:23`) — it is an AI-chat renderer, not a docs renderer; tables/anchors/heading-links for a Help page are not present.
- **Suggested fix:** Rewrite the Phase 5 "Quyết định" to say what it actually is: *add* an `articles` table + `articles_fts` + seed-versioning to the existing `knowledge.db`, reusing the connection and feedback pattern only. Budget the schema migration. For Plan 2 Phase 5, drop the assumption that Tầng 2 helps at launch and state that a compose-specific solution corpus must be built (it is not in the 23 builtin VM patterns). Assess `markdown.ts` against real KB article content before committing to it.

---

## Finding 7: `settingsStore` is a flat async string map with no schema — wrong substrate for table prefs, and unavailable to the tray at startup

- **Severity:** High
- **Plan:** Plan 1 (p0-polish-parity)
- **Location:** Phase 2 step 7 (onboarding progress), Phase 3 "Related Code Files" (tray settings), Phase 4 "Architecture" (`tablePrefs.svelte.ts` "lưu qua settingsStore")
- **Flaw:** Three phases lean on `settingsStore.svelte.ts` as if it were a typed, synchronous, versioned preference store. It is a 63-line `$state<{ [key: string]: string }>` that round-trips **every** write through an async backend call, has no namespacing, no schema version, and no error surfacing — both catch blocks are `console.error`, which is precisely the anti-pattern Phase 1 exists to eliminate.
- **Failure scenario:** (a) Phase 4's `tablePrefs` must JSON-encode widths/visibility/sort/collapsed-groups into a single string key. Every column drag issues an async `settingsApi.set` round-trip; on drag, that is a write per pointer event unless debounced, which the plan does not mention. If the write fails, `console.error` swallows it and the user's column widths silently reset on next launch — a bug report that will be impossible to reproduce. (b) Phase 4's own risk row "Prefs lưu cũ làm vỡ UI sau khi đổi cột" is mitigated with "có version cho schema prefs", but there is no versioning mechanism to build on. (c) Phase 3 needs "hiện tray icon" and "thu về tray khi đóng" **in Rust during `setup()`**, before any frontend has loaded settings; the plan never says how Rust reads these. Settings live in the `app_settings` SQLite table, so Rust *can* read them, but that is a separate code path nobody budgeted.
- **Evidence:**
  - `src/lib/settingsStore.svelte.ts:4-5` — `export const appState = $state({ isSettingsLoaded: false }); export const settingsState = $state<{ [key: string]: string }>({});` — values are strings only.
  - `src/lib/settingsStore.svelte.ts` `setAppSetting` — `try { await settingsApi.set(key, value); } catch (e) { console.error(\`Failed to save setting ${key}:\`, e); }` — silent failure, and one of the 13 `console.error` sites Phase 1 is meant to remove.
  - `src/lib/settingsStore.svelte.ts` `loadAllSettings` — `catch (e) { console.error("Failed to load settings from DB:", e); }`; note `appState.isSettingsLoaded` is never set on the failure path, so a settings-load failure leaves the app permanently in the "not loaded" state.
  - Backing store: `src-tauri/src/commands/knowledge_bank.rs:72` — `CREATE TABLE IF NOT EXISTS app_settings (...)`, i.e. settings share `knowledge.db`.
  - Existing key convention is un-namespaced flat strings: `colimaui_setup_complete`, `colimaui_tour_complete`, `ai_panel_width`, `ColimaCustomProfiles`.
- **Suggested fix:** Add an explicit Phase 1 (or Phase 2) task to harden `settingsStore`: typed JSON get/set helpers, a `schema_version` key, debounced writes, and error reporting through the new `reportError`. For Phase 3, specify the Rust-side read of `app_settings` for tray prefs and how a Settings toggle propagates to a running tray without restart (the success criterion already hedges "có thể yêu cầu restart" — decide it, do not defer).

---

## Finding 8: Plan 2's sidecar design ignores browser mode and has no distribution mechanism in the repo today

- **Severity:** High
- **Plan:** Plan 2 (commercial-foundation)
- **Location:** Phase 1, "Architecture" + "Implementation Steps" steps 3-4, and plan.md Unresolved Question #3
- **Flaw:** Three unstated dependencies. (1) **Distribution:** the plan says the core finds the sidecar "cạnh executable + biến môi trường dev". Tauri's supported mechanism is `externalBin`, and there is no `externalBin` entry in `tauri.conf.json` — only `resources/**/*`. Shipping a signed second binary requires bundler config, notarization of a second artifact, and (per Unresolved Question #3) a decision that is still open while Phase 1 is scheduled to produce working code. (2) **Browser mode:** Plan 1's stated principle is "Giữ dual-mode (Tauri IPC + HTTP browser) ở mọi thay đổi backend", and Phase 1 step 4 says capabilities are exposed "qua Tauri command + HTTP route cho browser mode" — but the phase never addresses that the HTTP server's port is not fixed. (3) **Security asymmetry:** the phase's non-functional requirement is a `0600` Unix socket "không expose ra mạng", yet the same Pro capabilities are simultaneously exposed over `127.0.0.1:11420` via the HTTP route. The socket permission is therefore not the real trust boundary — the API bearer token is — and the phase's security test ("socket quyền 0600; không tiến trình người dùng khác kết nối được") tests the wrong door.
- **Failure scenario:** The team builds and validates the sidecar against `pnpm tauri dev` with an env-var path. At packaging time they discover the bundle has no slot for the binary, notarization of the second artifact is unplanned, and Unresolved Question #3 (bundle vs. post-activation download) is still open — a Phase 1 architecture decision resurfacing at Phase 6 launch. Separately, a QA engineer testing browser mode with a second app instance running hits port 11421 and reports "Pro features don't work in browser mode".
- **Evidence:**
  - `grep -n 'externalBin\|sidecar' src-tauri/tauri.conf.json src-tauri/capabilities/default.json` → no match. `src-tauri/tauri.conf.json:34-38` — `"bundle": { ... "resources": ["resources/**/*"] }` only.
  - `src-tauri/src/lib.rs:46-53` — `.setup(|app| { ... api_server::start_api_server(); poller::start_instance_poller(app.handle());` — the HTTP server for browser mode runs **inside** the Tauri process, so browser mode is not a separate deployment, but it does share the port-scan behaviour below.
  - `src-tauri/src/api_server.rs:257` — `tokio::net::TcpListener::bind(format!("127.0.0.1:{}", port))` inside a scan loop; `:301` — `"[API Server] Could not bind to any port 11420-11429 — API server disabled"`. The port is not fixed at 11420, which also invalidates Plan 1 Phase 1's manual test "browser mode (`:11420`)".
  - Auth does exist and is the real boundary: `src-tauri/src/auth.rs:28-49` — `auth_middleware`, `Bearer <token>`, plus a `?token=` query-param path for SSE at `:40`.
  - Existing extra binaries are CLI clients, not sidecars: `src-tauri/src/bin/cli.rs`, `src-tauri/src/bin/cui.rs` (`cui.rs:116` posts to `http://127.0.0.1:11420/api/cli/chat` — itself hardcoding the port that the server may not have bound).
- **Suggested fix:** Move Unresolved Question #3 to a blocking step 0 of Phase 1. Add explicit deliverables: `externalBin` config, second-artifact notarization, and a decision on whether Pro capabilities are exposed over the HTTP route at all. If they are, state that the bearer token — not socket permissions — is the trust boundary and test that instead. Fix the port assumption in both plans' test steps.

---

## Finding 9: Comment- and span-preserving YAML patching is not achievable with the repo's YAML stack

- **Severity:** High
- **Plan:** Plan 2 (commercial-foundation)
- **Location:** Phase 5, "Implementation Steps" steps 3 and 5, and success criterion "Áp dụng bản vá giữ nguyên comment và định dạng gốc"
- **Flaw:** Step 3 requires "YAML parser có span"; step 5 requires text-level patching by span that preserves comments, and labels it "phần khó nhất". The plan names no crate for this and the repo's only YAML dependency is `serde_yml 0.0.12`, which is a serde data-model parser: it discards comments, discards formatting, and exposes no source spans. There is no Rust crate in the ecosystem that gives both a serde-style data model and lossless round-tripping the way Python's `ruamel.yaml` does; the realistic options (`yaml-rust2`, `marked-yaml`, or `saphyr`) give you markers/events but require hand-writing the entire edit-by-offset layer, and comment attachment must be reconstructed manually.
- **Failure scenario:** Step 3 is scheduled as a routine parsing task inside an 8-10 day phase whose headline deliverable is a ≥70% corpus fix rate. In reality the team must either (a) adopt a second, unfamiliar YAML crate and hand-build a span→byte-offset patcher — easily the majority of the phase's budget on its own — or (b) fall back to serialize-round-trip, which reformats the user's file and destroys comments, directly violating the success criterion and triggering the phase's own risk row "Áp dụng patch làm mất comment/format". Because the corpus benchmark is the launch gate (plan.md risk #4, "nếu <50% thì hoãn launch"), a slip here slips the entire commercial launch.
- **Evidence:**
  - `src-tauri/Cargo.toml:29` — `serde_yml = "0.0.12"` is the only YAML dependency in the workspace. It is a `serde_yaml` fork with the same data model: no comment retention, no span API.
  - `src-tauri/src/instance_reader.rs:83` — the repo's only existing YAML usage is `serde_yml::from_str(&content)`, i.e. lossy deserialize-into-struct. There is no precedent for span-aware parsing anywhere in the codebase.
  - Plan text (`phase-05:76`): "dùng YAML parser có span, không parse mất vị trí" — no crate named. (`phase-05:78`): "chỉnh sửa text theo span thay vì serialize lại toàn file."
  - The same lossless-round-trip requirement appears in Plan 1 Phase 5 step 2 (`#[serde(flatten)]` for colima.yaml), which is a *different and weaker* technique — it preserves unknown **keys** but still destroys comments and key order. The two plans are solving overlapping problems with incompatible approaches.
- **Suggested fix:** Add a blocking 1-day spike at the start of Phase 5 to pick and prove the span-aware crate, with a go/no-go exactly like Plan 1 Phase 6's spike gate. If lossless patching proves infeasible in the budget, downgrade the v1 success criterion from "preserve comments" to "show a diff and let the user apply it manually", and say so before the corpus benchmark is used as a launch gate. Also reconcile with Plan 1 Phase 5 so colima.yaml and compose.yaml use one YAML strategy.

---

## Finding 10: Effort estimates omit the prerequisite work the codebase actually requires

- **Severity:** High
- **Plan:** Both
- **Location:** Plan 1 plan.md "Ước lượng" (5-7 weeks total); Plan 2 plan.md "Ước lượng" (7-9 weeks total)
- **Flaw:** The estimates are internally consistent but were derived from the optimistic "already exists" claims that Findings 1-9 falsify. Every falsified claim converts assumed-free work into unbudgeted work, and several estimates are implausible on raw scale alone.
  - **Phase 1, 4-6 days:** the scope is a breaking rewrite of the Rust error type, a helper applied across **7 command modules**, a change to the TS error contract at `client.ts`, and an audit of **164 catch sites** across ~11.9k lines of frontend. `containers.rs` alone is 773 lines. The success criterion "Mọi `catch` trong `src/` hoặc gọi `reportError` hoặc có comment lý do" is 164 individual judgment calls.
  - **Phase 3, 2-3 days** is justified purely by "`tray-icon` feature đã bật sẵn" — true, but the feature flag is the cheapest part; the estimate excludes designing 4 template icons at 1x/2x, Rust-side settings reads (Finding 7), Linux DE degradation testing on GNOME **and** KDE, and lifecycle changes for close-to-tray.
  - **Phase 4, 4-5 days** excludes the backend label work of Finding 4 and extracting a `DataTable` that must absorb a 535-line `Containers.svelte`.
  - **i18n tax, uncounted:** every phase in Plan 1 ends with "i18n toàn bộ chuỗi mới" across **4 locale files** (en/ja/vi/zh). Nobody on the team is likely to be fluent in all four; either translation quality degrades or this is recurring unestimated work in all 6 phases.
  - **Plan 2 Phase 4, 4-5 days** for telemetry + crash reporting + signed auto-update assumes the release pipeline already signs builds. It does not.
- **Failure scenario:** Phase 1 is the declared foundation for Phases 2, 4, 5 and for Plan 2 Phase 5's `errorReporter` integration. When it runs 2-3x over, every dependent phase slips, and the two plans' only real coupling point — Plan 2 Phase 6 launch — slips with it. Because Plan 1 Phase 6 (8-12 days, the riskiest) is scheduled last, the natural pressure response is to cut it, which removes the single biggest OrbStack-parity differentiator while the paid tier ships anyway.
- **Evidence:**
  - Scale: `wc -l` → `src-tauri/src/commands/containers.rs` 773, `src-tauri/src/docker_state.rs` 335, `src-tauri/src/lib.rs` 204, `src/pages/Containers.svelte` 535. Totals: `src/**/*.{ts,svelte}` = 11,914 lines; `src-tauri/src/**/*.rs` = 13,951 lines.
  - 164 catch blocks (verified), 13 `console.error` (verified), 2 toast files (verified).
  - Locales: `src/locales/` contains `en.json, ja.json, vi.json, zh.json` — 4 files per string change.
  - No signing infrastructure: `.github/workflows/` contains `ci.yml`, `promote.yml`, `release.yml`; `grep -n 'sign\|notarize\|APPLE' .github/workflows/release.yml` matches only `TAURI_VERSION=$(echo "$VERSION" | ...)` at `:160` and `conf.version = '$TAURI_VERSION'` at `:164` — i.e. version stamping, **no** code signing, no notarization, no updater key. `grep -n 'updater' src-tauri/Cargo.toml` → no match; only `tauri-plugin-opener` (`:23`) and `tauri-plugin-http` (`:40`) are present.
- **Suggested fix:** Re-estimate Phase 1 after the `ColimaError` contract decision from Finding 3 is made, and split it into "Rust error model" and "frontend catch-site sweep" as separately tracked work. Add explicit line items for: container label plumbing (Phase 4), tray icon asset design (Phase 3), i18n across 4 locales (every phase), and Apple Developer ID signing + notarization + updater keypair (Plan 2 Phase 4) — the last is a hard prerequisite for auto-update and is currently absent from the repo, not merely "extend the existing workflows".

---

## Finding 11: The macOS machine fingerprint requirement is asserted, not designed

- **Severity:** Medium
- **Plan:** Plan 2 (commercial-foundation)
- **Location:** Phase 2, "Non-functional" + "Implementation Steps" step 4 + "Tests / Validation"
- **Flaw:** Step 4 says "fingerprint từ đặc trưng phần cứng ổn định, hash SHA-256" and the test is "fingerprint **không đổi** sau khi restart và sau khi cập nhật app" — but no hardware source is named. On macOS the candidates each have a documented failure mode: `IOPlatformUUID` is stable but changes on logic-board replacement and is identical across VM clones from the same image; the hardware serial requires reading IOKit and is arguably PII under the plan's own "không dùng thông tin định danh cá nhân" rule; MAC addresses are randomized and change with network interface state. The phase's own risk table rates "Fingerprint đổi sau khi update OS/app → khoá nhầm người trả tiền" as **Cao** and then mitigates it with "chọn đặc trưng ổn định nhất" — restating the unsolved problem as its own solution.
- **Failure scenario:** Ed25519 verification, the license state machine, and the activation server are all built on top of an unspecified fingerprint. If the chosen source turns out to drift (or to collide across cloned corporate VM images, where every machine reports the same UUID and a 3-5 seat limit is exhausted by one customer's fleet), the fix is not local — it invalidates every already-issued activation record on the server and requires a migration for paying customers. The plan's max-activations feature (`POST /activate → { activations_used, max }`) makes this a customer-facing billing incident, not a bug.
- **Evidence:**
  - Plan text (`phase-02:32`): "Machine fingerprint **không dùng** thông tin định danh cá nhân — hash ổn định từ đặc trưng phần cứng, không gửi giá trị thô." No source named anywhere in the phase.
  - Plan text (`phase-02:82`, step 4): "fingerprint từ đặc trưng phần cứng ổn định, hash SHA-256" — still no source.
  - Risk table `phase-02:114` rates it **Cao** with mitigation "Chọn đặc trưng ổn định nhất" — circular.
  - The repo has platform-detection scaffolding that would host this (`src-tauri/src/platform.rs`) but no hardware-identity code exists today: `grep -rn 'IOPlatformUUID\|ioreg\|machine_id' src-tauri/src/` → no match.
- **Suggested fix:** Name the exact source (recommend `IOPlatformUUID` via IOKit on macOS, `/etc/machine-id` on Linux) in the phase, and add an empirical pre-work check on real hardware across an OS update. Design the server to tolerate fingerprint rotation from day one — store a small history per license and rebind rather than reject — since the plan's stated policy is already "nếu nghi ngờ thì ưu tiên mở cho người dùng".

---

## Cross-cutting note

Findings 1, 2, 4, 5, and 6 share one root cause: the plans treat **the existence of a file or a line count** as evidence of capability. `capabilities.rs` exists but means something else; `useHotkeys.ts` exists but is from another framework; `instance_reader.rs` reads config but only 7 fields, lossily; `knowledge_bank.rs` is genuinely 883 lines but its schema does not cover the two proposed reuses. Before execution, every remaining "đã có sẵn / tận dụng sẵn" claim in both plans should be re-verified by reading the symbol, not the filename.

## Unresolved questions for the planner

1. Is the `ColimaError` wire-format break acceptable in one release, or is a parallel V2 contract required? This changes Phase 1's shape and estimate more than any other decision.
2. Should Pro capabilities be reachable over the HTTP API at all, or Tauri-IPC only? Answering "IPC only" removes the browser-mode gap in Plan 2 Phase 1 but breaks Plan 1's stated dual-mode principle.
3. Who owns ja/zh translation quality across 10+ phases of new strings?
4. Is Apple Developer ID signing + notarization already procured? Plan 2 Phase 4 and Phase 1's "ký + notarize riêng" both assume it, and the repo has none.
