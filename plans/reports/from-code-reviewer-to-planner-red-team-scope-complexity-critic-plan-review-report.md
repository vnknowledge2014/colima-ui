# Red Team Review — Scope & Complexity Critic

**Reviewer role:** Scope & Complexity Critic / Contract Verifier
**Date:** 2026-08-10
**Plans reviewed:** `plans/260810-2258-colimaui-p0-polish-parity/`, `plans/260810-2258-colimaui-commercial-foundation/`
**Method:** grep/glob verification of every factual claim against the repo. No code, lint, or build checks run.

---

## Finding 1: Phase 1's core evidence claim is false — error surfacing already exists on the main paths

- **Severity:** Critical
- **Plan:** Plan 1 (p0-polish-parity)
- **Location:** Phase 1, section "Bằng chứng vấn đề" (and `plan.md` "Overview" bullet 1)
- **Flaw:** The plan justifies 4-6 days plus a 7-module Rust refactor on the claim that only 2 files touch toast. That is wrong by an order of magnitude. Toast is already wired into every primary action page.
- **Failure scenario:** 4-6 days (plus follow-on churn across `containers.rs`, `colima.rs`, `compose.rs`, `volumes.rs`, `networks.rs`, `kubernetes.rs`, `lima.rs` — 2758 lines of Rust) is spent re-plumbing a path that already delivers the headline user benefit. Phase 2 and Phase 4 both declare `dependencies: [1]`, so this wrong premise gates 11-15 further days.
- **Evidence:**
  - Plan quote (phase-01, "Bằng chứng vấn đề"): *"Chỉ **2 file** import toast: `src/lib/globalToast.ts`, `src/components/ToastContainer.svelte`."*
  - Reality: `grep -rl globalToast src` → **15 files**, including `src/pages/Containers.svelte`, `Volumes.svelte`, `Networks.svelte`, `Images.svelte`, `Compose.svelte`, `Instances.svelte`, `Kubernetes.svelte`, `LinuxVMs.svelte`, `Models.svelte`, `Settings.svelte`.
  - `grep -ic toast src/pages/*.svelte` → **104 call sites**: Kubernetes 21, Instances 16, Images 14, Networks 10, LinuxVMs 10, Containers 9, Volumes 9.
  - `src/pages/Containers.svelte:96,113,206` are literally `globalToast("error", String(e))` on the remove/action/start handlers — the exact "silent failure" the plan says does not surface.
  - The 164 `catch` figure is accurate but unweighted: `src/pages/Containers.svelte:236,245,252,281` write errors into visible detail panes (`detailLogs = \`Error: ${e}\``), not silence.
  - `ColimaError` at `src-tauri/src/error.rs:6-13` already has a 6-variant kind enum and is already the return type across the command layer (`src-tauri/src/commands/compose.rs:36,69,97,115,133,157`).
- **Suggested fix:** Cut Phase 1 from 4-6 days to ~1.5 days. There is exactly one choke point for both modes: `src/lib/api/client.ts:26` `call<T>()`, whose Tauri branch at lines 36-44 already unwraps `{type, message}` and whose HTTP branch at lines 70-73 throws `json.error`. Add `normalizeError()` there, plus a `hint` lookup table keyed on stderr substrings, and audit only the ~10 handlers that currently swallow (`Containers.svelte:129,150` `catch {}` in bulk ops). **Lost:** structured `command`/`exit_code` fields and `ErrorLogPanel.svelte`. Neither is a user-retention driver; both can be added if the AI-diagnostic flow demands them.

---

## Finding 2: 12-16 weeks of build before a single revenue signal, on a project that has existed for 19 weeks total

- **Severity:** Critical
- **Plan:** Both (`plan.md` "Ước lượng" in each)
- **Location:** Plan 1 "Ước lượng" (5-7 weeks) + Plan 2 "Ước lượng" (7-9 weeks); Plan 2 Phase 6 "Cổng launch"
- **Flaw:** Every willingness-to-pay signal is deferred to the last phase of the second plan. The pricing page — the single cheapest validation artifact — is buried at Plan 2 Phase 3 step 10 and Phase 6 step 9, behind license boundary, entitlement, MoR approval, telemetry, auto-update, and the flagship feature.
- **Failure scenario:** Plan 2's own risk register calls risk #4 (*"Người dùng thấy giá trị Pro không đủ để trả $6"*) **Cao**, with mitigation "prove it at Phase 5". Phase 5 is ~week 10 of Plan 2. If the answer is no, 10+ weeks of sidecar architecture, license server, activation flow, telemetry, and auto-update are sunk on a product nobody buys. For a solo maintainer this is the entire remaining year's discretionary capacity.
- **Evidence:**
  - Repo history: first commit `2026-03-26`, latest `2026-08-10`, **102 commits total** (`git log --oneline | wc -l`). The two plans propose 12-16 weeks — roughly 70-90% of the project's entire lifetime to date — as one uninterrupted pre-revenue block.
  - Whole Rust command layer built in that time: **5896 lines** (`wc -l src-tauri/src/commands/*.rs`).
  - Plan 2 `plan.md` Dependencies: *"ràng buộc thật ở thời điểm **launch** (Phase 6)"* — confirming no earlier revenue gate exists.
- **Suggested fix:** Insert a week-1 validation slice ahead of both plans: pricing page + comparison page + "Pro waitlist / $60 lifetime pre-order" using the MoR's hosted checkout only. That artifact is already scoped in Plan 2 Phase 3 steps 10 and Phase 6 step 9 — just move it to the front. Gate everything in Plan 2 Phases 1-2 on N pre-orders. **Lost:** nothing; the page has to be built regardless.

---

## Finding 3: Phase 6 (DNS + local CA + TLS proxy + cross-platform trust store) at 8-12 days is not a credible estimate

- **Severity:** Critical
- **Plan:** Plan 1 (p0-polish-parity)
- **Location:** Phase 6, sections "Architecture", "Dependency mới", "Implementation Steps"
- **Flaw:** Six new Rust modules, four new crates, a DNS server, TLS termination with SNI cert selection, a local CA, macOS `/etc/resolver` + Keychain integration, a Linux resolver matrix (systemd-resolved / NetworkManager / dnsmasq), WebSocket proxying, port-conflict fallback, and clean uninstall — estimated at 8-12 days. Every one of these is a multi-day subsystem on its own; the trust-store and Linux-resolver branches alone are open-ended.
- **Failure scenario:** The realistic outcome is a 4-6 week slip that consumes Plan 1 whole, delaying everything downstream (Plan 2 Phase 6 is hard-gated on Plan 1). The plan's own risk table rates three separate risks "Cao" and names the failure mode: *"Phạm vi phình to nuốt cả plan"* — **Cao**. When a plan predicts its own devouring, believe it.
- **Evidence:**
  - Comparable in-repo subsystems and their build cost: `src-tauri/src/commands/knowledge_bank.rs` (883 lines) shipped as one commit `2026-04-30 "feat: add self-learning AI diagnostic agent with knowledge bank, command sandbox, and feedback loop"`; `kubernetes.rs` (497 lines) accreted from `2026-03-27` to `2026-08-08`. Neither of those touches privileged system state, a trust store, or a DNS resolver.
  - Zero existing precedent in the repo for privileged install: `grep -rn "tray\|updater" src-tauri/Cargo.toml` shows only `tauri = { version = "2", features = ["tray-icon"] }` at line 22, and dependencies are limited to `tauri-plugin-opener` (line 23) and `tauri-plugin-http` (line 40). No signing, notarization, or privileged-helper machinery exists to build on.
  - Plan quote (`plan.md` "Nguyên tắc"): *"Không thêm dependency mới trừ khi bắt buộc (Phase 6 là ngoại lệ duy nhất)"* — the phase requires 4 new crates (`rcgen`, `hickory-server`, `hyper-rustls`/`rustls`, `tokio-rustls`), a self-acknowledged violation of the plan's own principle.
- **Suggested fix:** **Delete Phase 6 from Plan 1 entirely.** If domain access matters, ship the 10% that delivers 80%: a `*.localhost` / `/etc/hosts`-free HTTP-only reverse proxy on a single fixed port, routing by `Host` header, with no DNS server, no CA, and no privileged install — that is `registry.rs` + `proxy.rs`, ~2-3 days. **Lost:** the "no cert warning" HTTPS story and OrbStack visual parity. That is a legitimate loss, but it belongs in its own plan with its own spike gate, not as the tail of a polish plan.

---

## Finding 4: `DataTable` abstraction has zero existing consumers — Containers.svelte is not even a table

- **Severity:** High
- **Plan:** Plan 1 (p0-polish-parity)
- **Location:** Phase 4, sections "Architecture" and "Related Code Files"
- **Flaw:** The phase creates `DataTable.svelte`, `ColumnHeader.svelte`, `TableGroup.svelte`, `Tooltip.svelte`, and `tablePrefs.svelte.ts` — five new shared modules — to serve one page. The Contract Verifier test (two or more real consumers) fails outright.
- **Failure scenario:** 4-5 days building a generic table framework, column-resize pointer handling, a persisted-prefs schema with migration, and a portal tooltip. The single measurable user win — compose grouping — is `composeGrouping.ts` plus ~40 lines of markup in one file. The prefs-migration code (step 5: *"migrate an toàn khi cột thay đổi giữa các phiên bản"*) is maintenance debt for a feature nobody has asked for yet.
- **Evidence:**
  - `grep -rln "<table" src/pages src/components` → **only 2 files**: `src/pages/Kubernetes.svelte`, `src/pages/Dashboard.svelte`. Neither is in the phase's primary migration target list.
  - `src/pages/Containers.svelte` (535 lines) contains **no `<table>` element at all** — its only table-ish token is `class="table-actions"` at line 378. The page it is meant to convert is a div/card layout; "converting Containers to DataTable" is a from-scratch rewrite, not an extraction.
  - `grep -c "<th" src/pages/*.svelte` → Kubernetes 3, Dashboard 2, all others 0. There is no duplicated table markup to DRY up.
  - Tooltip: the plan's own non-functional note cites commit `3908c73` *"use native tooltip for sidebar to prevent clipping"*, and `title=` is already used across 8 pages (Kubernetes 12, LinuxVMs 4, Containers 3, Images 3, Models 3). The repo already decided against a custom tooltip.
  - Grouping data does not exist yet: `grep -rn labels src-tauri/src/commands/containers.rs src-tauri/src/docker_state.rs src/lib/api/types.ts` → **0 matches**. Labels plumbing is real work; the table framework around it is not.
- **Suggested fix:** Keep steps 1, 2, 6, 8 (labels in backend, `composeGrouping.ts` + tests, group header markup inline in `Containers.svelte`, group/flat toggle). Delete `DataTable.svelte`, `ColumnHeader.svelte`, `Tooltip.svelte`, `tablePrefs.svelte.ts`, and column resize/visibility persistence. Estimated 4-5 days → ~2 days. **Lost:** column resizing and persisted column prefs. Neither appears in the plan's own acceptance criteria as an OrbStack parity gap; they are gold plating.

---

## Finding 5: A private repo + sidecar + versioned JSON-RPC + separate CI + separate signing, to gate features that do not exist

- **Severity:** High
- **Plan:** Plan 2 (commercial-foundation)
- **Location:** Phase 1, sections "Architecture", "Related Code Files", "Implementation Steps" (steps 5, 9)
- **Flaw:** 5-8 days of pure infrastructure — second repo, second signing identity, second CI pipeline, IPC protocol with version negotiation, health check, restart-with-backoff, capability registry exposed over both Tauri IPC and HTTP — before one line of paid functionality exists. Step 5 builds an `echo` capability whose only purpose is to prove the pipe.
- **Failure scenario:** Two repos to maintain, two release artifacts to sign and notarize, and a protocol frozen at v1 before any real capability shaped it. Plan 2's own risk #1 is exactly this: *"Thiết kế protocol sai, phải sửa sau khi đã có tính năng Pro"* — **Cao**. The mitigation (build `echo` first) does not reduce that risk, because `echo` exercises none of the real payload shapes (compose file contents, diffs, image scan results).
- **Evidence:**
  - No signing/notarization infrastructure exists to duplicate: `src-tauri/Cargo.toml` carries only `tauri-plugin-opener` (line 23) and `tauri-plugin-http` (line 40); no updater, no signing plugin. Plan 2 Phase 4 step 7 concedes the update signing key must still be created.
  - **Contract inconsistency:** Phase 1 states *"Core MIT không được import, link tĩnh, hay chứa bất kỳ mã nguồn Pro nào"*, yet Phase 5 "Related Code Files" says `Modify: src-tauri/src/pro/registry.rs — đăng ký capability compose.analyze / compose.patch` — i.e. Pro capability names registered inside the MIT core. Either the boundary is looser than stated or the file list is wrong. This must be resolved before the boundary is called "proven".
  - Overhead budget (`<200ms` startup) is asserted with no baseline measured anywhere in the repo.
- **Suggested fix:** The cheapest legally clean boundary is not a sidecar. You hold the copyright on all of it, so you can publish `colima-ui` under MIT and keep a `pro/` subtree under a proprietary license in the *same* repo, or simply not publish that subtree — no IPC, no second CI, no protocol versioning. If a process boundary is still wanted for licensing optics, defer it until the flagship feature exists so the protocol can be shaped by a real payload. **Lost:** the "not one line of Pro code in the MIT repo" bragging right in the acceptance criteria. That is a marketing claim, not a legal requirement, and it is already being violated by the Phase 5 file list.

---

## Finding 6: Building a license server, activation service, fingerprinting, grace periods, and webhook queue in-house — all commodity

- **Severity:** High
- **Plan:** Plan 2 (commercial-foundation)
- **Location:** Phase 2 "Architecture" (License server block) + Phase 3 "Architecture" and "Implementation Steps" (steps 3-7)
- **Flaw:** 10-13 days combined to build Ed25519 key issuance, a 3-endpoint activation server, machine fingerprinting, a 5-state license machine, a 14-day grace period, webhook signature verification, idempotency, a retry queue, and transactional email. Every one of these is a standard feature of the MoR the plan is already committing to.
- **Failure scenario:** 10-13 days spent, plus permanent operational burden (a service that, per the plan's own risk table, *"chết → người trả tiền bị chặn"* — **Cao**) and a key-rotation plan for a private signing key. The plan simultaneously declares (`plan.md` principle #4) *"License check là hàng rào lịch sự, không phải DRM"* — the invested engineering is wildly disproportionate to a politeness fence.
- **Evidence:**
  - Plan quote (phase-02, Requirements): fingerprint + max-machines + activation registry + deactivation. Plan quote (phase-02, risk table, rated **Cao**): *"Fingerprint đổi sau khi update OS/app → khoá nhầm người trả tiền"*. This is a self-inflicted High risk that exists only because the machinery is homegrown.
  - Plan quote (phase-03, Requirements): *"Sự kiện webhook thất bại phải vào hàng đợi retry"* — building a durable queue for an expected volume of, at launch, single-digit orders.
  - No existing server-side code in this repo to build on: `ls src-tauri/src` command layer is entirely local-process (`grep -n "pub async fn" src-tauri/src/commands/knowledge_bank.rs` → all local SQLite/rusqlite operations). This is a greenfield service plus greenfield ops for a solo maintainer.
  - Plan 2 `plan.md` Unresolved Question #1 shows the MoR is not even chosen yet, so the build is being scoped before knowing what the provider gives away free.
- **Suggested fix:** Resolve Unresolved Question #1 first, then delete the in-house license server. Polar, LemonSqueezy, and Paddle all ship license-key issuance, activation-count limits, a validate/activate/deactivate API, and the purchase email; Keygen covers the same if none fit. Phase 2 collapses to: call the provider's validate endpoint, cache the response locally with a timestamp, allow N days offline. Phase 3 collapses to: hosted checkout link + deep-link handler. Estimated 10-13 days → ~3-4 days, and the "license server down" and "fingerprint drift" risks move off your books. **Lost:** offline-forever signature verification and provider independence. Given principle #4 explicitly refuses to fight cracking, neither is worth two weeks.

---

## Finding 7: Compose auto-fix tier 1 rebuilds validation that `docker compose config` already performs, and tier 2 already exists

- **Severity:** High
- **Plan:** Plan 2 (commercial-foundation)
- **Location:** Phase 5, sections "Kiến trúc lai" (Tầng 1), "Implementation Steps" (steps 1-4)
- **Flaw:** The tier-1 rule list is: YAML syntax errors, missing required keys, wrong value types, `depends_on` pointing at a nonexistent service, wrong network name, undefined environment variables. `docker compose config` reports essentially all of these, with file and line, for free. The plan proposes a span-preserving YAML parser and hand-written rules for each — plus a 30-file benchmark corpus to grade work that a subprocess call already does.
- **Failure scenario:** A meaningful share of the 8-10 day flagship budget goes to reimplementing upstream validation. Worse, the corpus-collection step (step 1: *"≥30 file compose lỗi thật … Không có corpus thì không đo được chất lượng"*) is a blocking prerequisite with no time-boxed exit, and it gates the launch (Phase 6 step 1).
- **Evidence:**
  - `grep -c '"config"' src-tauri/src/commands/compose.rs` → **0**. The `config` subcommand is never invoked. Existing commands are `list_compose_projects` (compose.rs:36), `compose_up` (:69), `compose_down` (:97), `compose_restart` (:115), `compose_logs` (:133), `compose_ps` (:157). The cheapest validation source available is not wired up at all.
  - stderr is already captured on every path — `src-tauri/src/commands/compose.rs:43,86,104,122,142,166` all do `String::from_utf8_lossy(&output.stderr)`. The raw material for a diagnosis feature is already in hand today.
  - Tier 2 is not new work either: `src-tauri/src/commands/knowledge_bank.rs` already provides `kb_query` (:337), `kb_feedback` (:459), `kb_save_solution` (:483), `kb_learn` (:498), `kb_save_anti_pattern` (:513) over SQLite. Tier 3 is also largely present: `src-tauri/src/commands/ai_chat.rs` (571 lines) and `agent_loop.rs` (135 lines) already implement a BYOK diagnostic agent.
  - So of the three tiers, one is free from a subprocess, one exists (883 lines), and one exists (706 lines). The genuinely new work is `patch.rs` — which the plan itself flags as *"đây là phần khó nhất"*.
- **Suggested fix:** Reorder. Day 1-2: wire `docker compose config --quiet`, pipe stderr through the existing `kb_query`, surface findings in `Compose.svelte`. Measure how many real errors that alone explains. Only then decide whether hand-written span-aware rules earn their keep, and let the same shipped feature tell you whether patch application is worth building. Cut the 30-file corpus to a time-boxed 10 files. **Lost:** the "100% chính xác, chạy tức thì" tier-1 marketing line and sub-500ms latency (a subprocess is ~200-800ms). Neither is why anyone pays $6.

---

## Finding 8: Phase 6 ships five more Pro features before the first sale, and one of them requires an account system nothing else in the plan builds

- **Severity:** Medium
- **Plan:** Plan 2 (commercial-foundation)
- **Location:** Phase 6, sections "Bộ tính năng Pro v1" and "Implementation Steps" (steps 2-6)
- **Flaw:** Dockerfile optimizer, self-healing, activity history, vulnerability scan, and config sync — 8-10 days — are all built before a single customer has paid, on the assumption that $6 needs a full basket. Learning is deferred until all five exist.
- **Failure scenario:** If the flagship is the real draw, four of these are wasted. If the flagship is weak, adding four unvalidated features will not rescue it — and the plan's own risk register concedes this (*"Gói Pro vẫn không đủ giá trị cho $6"* — **Cao**, mitigated by a 20-person beta that happens after all five are built).
- **Evidence:**
  - **Contract gap:** Phase 6 requires *"Đồng bộ cấu hình … qua tài khoản"* and `src/config_sync/`. No account system exists anywhere in Plan 2 — Phase 2 issues signed license *keys*, and Phase 2 explicitly avoids identity (*"Machine fingerprint **không dùng** thông tin định danh cá nhân"*). Phase 3's data model stores only *"email, license id, trạng thái subscription, id giao dịch"*. Config sync therefore requires an unplanned auth service plus a sync backend — unestimated in the 8-10 days.
  - `vuln_scan` adds an external binary dependency (Trivy) with an auto-download path; the repo currently resolves external tools through `src-tauri/src/path_util.rs` only for tools the user installed (`colima`, `docker`, `limactl`, `kubectl` per Plan 1 `plan.md` Dependencies). Bundling/downloading a scanner is a new distribution and update surface.
  - Activity history proposes reusing SQLite (`knowledge_bank.rs:14` `get_db()` global `Mutex<Connection>`) for time-series writes — a single global mutex connection is a questionable substrate for continuous metric ingestion.
- **Suggested fix:** Launch with the flagship plus exactly one cheap companion (Dockerfile optimizer — no external binary, no backend). Delete config sync outright (it is the only feature requiring accounts). Defer self-heal, activity history, and vuln scan to a post-revenue plan. Estimated 8-10 days → ~3 days. **Lost:** basket thickness. Ship the beta four weeks earlier and let 20 beta users tell you which of the four to build.

---

## Finding 9: Phase 2 adds a third onboarding surface alongside two that already exist

- **Severity:** Medium
- **Plan:** Plan 1 (p0-polish-parity)
- **Location:** Phase 2, "Architecture" (capabilities store → EmptyState → Dashboard onboarding checklist)
- **Flaw:** The phase adds a capability-detection store, a shared `EmptyState.svelte`, per-page empty states, *and* a Dashboard onboarding checklist — on top of an existing 568-line setup wizard, an existing tour, and empty-state markup already present in 11 files.
- **Failure scenario:** Three overlapping first-run surfaces (wizard, tour, dashboard checklist) that must be kept mutually consistent. A new user can plausibly see all three at once. The plan does not say which wins or when each is suppressed.
- **Evidence:**
  - `src/components/SetupWizard.svelte` = **568 lines**; `src/components/GettingStartedTour.svelte` = **171 lines**; plus `src/components/TourTooltip.svelte`.
  - `grep -rl "empty-state" src` → **11 files** already have empty states: `Compose.svelte`, `ClusterTopology.svelte`, `Instances.svelte`, `Dashboard.svelte`, `Containers.svelte`, `Images.svelte`, `LinuxVMs.svelte`, `Models.svelte`, `Terminal.svelte`, `Kubernetes.svelte`, `KubernetesHealth.svelte` (46 total occurrences).
  - The phase's genuinely new and valuable part is small and it names it: `get_system_capabilities()` in `src-tauri/src/commands/system.rs` (462 lines, `SYSTEM_INFO_CACHE` already exists) and the note that `src-tauri/src/routes/capabilities.rs` already exists.
- **Suggested fix:** Keep the capability detection command and feed its state into the **existing** `SetupWizard.svelte` and the existing 11 empty states. Drop the new `EmptyState.svelte` component and the Dashboard onboarding checklist. Estimated 3-4 days → ~2 days. **Lost:** a uniform empty-state look. Cosmetic, and reachable later by refactoring the 11 existing ones if it ever hurts.

---

## Finding 10: Two write paths to the same Colima state, plus a cross-plan file-name mismatch on DiffView

- **Severity:** Medium
- **Plan:** Plan 1 (p0-polish-parity), with a contract break into Plan 2
- **Location:** Phase 5, "Architecture" (`colima_config.rs` → `write_instance_config`) and "Related Code Files"
- **Flaw:** The phase writes Colima's YAML config file directly, while the app already configures the same fields through CLI flags at instance start. Two writers to one piece of state, with no stated reconciliation.
- **Failure scenario:** A user edits CPU/memory in the new config UI, then starts the instance through the existing path — which passes `--cpu/--memory/--disk` flags and silently overrides the edit. The user concludes the settings screen is broken. The plan's diff dialog will show a change that does not survive the next start.
- **Evidence:**
  - The existing flag-based writer covers precisely the fields the new form proposes to own: `src-tauri/src/commands/colima.rs:87` `start_instance(config: StartConfig)` pushes `--runtime` (:99), `--cpu` (:102), `--memory` (:105), `--disk` (:108), `--vm-type` (:110-112), `--arch` (:115-117), `--mount-type` (:120-122), mounts (:125), dns (:130). `StartConfig` is defined at colima.rs:43-46.
  - **Cross-plan contract mismatch:** Plan 2 phase-05 "Related Code Files" states *"Reuse: `src/components/DiffView.svelte` — đã tạo ở plan `260810-2258-colimaui-p0-polish-parity` Phase 5"*. Plan 1 Phase 5's file list contains no `DiffView.svelte` — it lists `src/components/settings/ConfigDiffDialog.svelte`. `ls src/components | grep -i diff` → **no matches**; the file does not exist today. Plan 2 is depending on an artifact Plan 1 does not promise, under a name Plan 1 does not use.
- **Suggested fix:** Pick one writer. The cheapest correct option is to keep `StartConfig` authoritative and have the settings form edit `StartConfig` (persisted via the existing `set_setting`, `knowledge_bank.rs:794`) rather than the YAML file — this deletes `write_instance_config`, the backup logic, and the raw-YAML editor with its validation. Separately, either add `DiffView.svelte` to Plan 1 Phase 5 by that exact name or correct Plan 2 Phase 5's reuse claim. **Lost:** raw-YAML editing for advanced users, who already have `$EDITOR` and `colima start --edit`.

---

## Recommended Cuts

Priority order, most weeks saved first.

| # | Cut | Where | Saved |
|---|---|---|---|
| 1 | **Delete Plan 1 Phase 6 (container domains / DNS / CA / TLS proxy) entirely.** Re-plan as a standalone effort with its own spike gate; optionally ship an HTTP-only Host-header proxy (~2-3 days) if domain access is truly demanded. | Plan 1 Phase 6 | **8-12 days** (realistically more) |
| 2 | **Replace the in-house license + activation server with the MoR's built-in license keys.** Resolve Unresolved Question #1 first. Phases 2 and 3 collapse to key validation + hosted checkout + deep link. | Plan 2 Phases 2, 3 | **7-9 days** |
| 3 | **Cut Plan 2 Phase 6 to flagship + Dockerfile optimizer.** Delete config sync (needs an account system nothing else builds), self-heal, activity history, vuln scan. | Plan 2 Phase 6 | **5-7 days** |
| 4 | **Delete the DataTable/ColumnHeader/Tooltip/tablePrefs framework.** Keep `composeGrouping.ts` + labels plumbing + inline group headers in `Containers.svelte`. | Plan 1 Phase 4 | **2-3 days** |
| 5 | **Rescope Phase 1 to `normalizeError()` at `src/lib/api/client.ts:26` plus a hint table.** Drop the 7-module Rust refactor and `ErrorLogPanel`. Premise was falsified — toast is already wired at 104 sites. | Plan 1 Phase 1 | **3-4 days** |
| 6 | **Defer the sidecar/private-repo/JSON-RPC boundary.** Use a proprietary-licensed subtree until the flagship feature exists and can shape the protocol. | Plan 2 Phase 1 | **3-5 days** |
| 7 | **Cut compose auto-fix tier 1 to `docker compose config` + existing Knowledge Bank.** Time-box the corpus to 10 files; reassess hand-written rules after shipping. | Plan 2 Phase 5 | **3-4 days** |
| 8 | **Drop the new EmptyState component and Dashboard onboarding checklist.** Feed capability detection into the existing 568-line SetupWizard and the 11 existing empty states. | Plan 1 Phase 2 | **1-2 days** |
| 9 | **Drop raw-YAML config writing.** Make `StartConfig` the single writer. | Plan 1 Phase 5 | **1-2 days** |
| 10 | **Move the pricing + comparison page to week 1** as a paid-waitlist validation gate ahead of both plans. Net-zero cost (already scoped in Plan 2 Phase 3 step 10), but it is the highest-value reordering in this review. | Both | 0 days, de-risks all of it |

**Net effect:** roughly 12-16 weeks → roughly **5-7 weeks**, with the first willingness-to-pay signal arriving in week 1 instead of week 16.

---

## Unresolved Questions for the planner

1. Was the "only 2 files import toast" claim produced by a case-sensitive grep for `toast` that missed `globalToast`? Every downstream estimate in Plan 1 Phase 1 depends on that number.
2. Plan 2 Phase 1 forbids Pro code in the MIT core, but Phase 5 modifies `src-tauri/src/pro/registry.rs` to register `compose.analyze`. Which is the real contract?
3. Config sync (Plan 2 Phase 6) requires user accounts. Where is that scoped? It appears in no phase estimate.
4. Which MoR? Plan 2 Unresolved Question #1 is unanswered, yet Phases 2 and 3 are estimated as if none of the provider's built-in licensing is usable.
5. `DiffView.svelte` vs `ConfigDiffDialog.svelte` — one plan depends on a file the other does not promise.
