# Red-team review — Scope & Complexity Critic

Plans reviewed: `plans/260811-2241-free-tier-foundation/`, `plans/260811-2245-pro-tier-features/`
Reviewer perspective: YAGNI enforcer + contract verifier. Date: 2026-08-11.

---

## Finding 1: Free Phase 4 deletes two Kubernetes components on a false premise

- **Severity:** Critical
- **Location:** Plan A, Phase 4, "Overview" / "Related Code Files" / "Implementation Steps" step 8
- **Flaw:** The phase states the two files "chồng lấn" with a Docker topology (container/network/volume/compose) and plans `Delete: src/pages/ClusterTopology.svelte, src/pages/XRay.svelte` plus `Modify: src/App.svelte, src/components/Sidebar.svelte (gỡ 2 mục cũ)`. Both files are **Kubernetes** views, and neither is routed from App.svelte or Sidebar.svelte. The same phase's risk table declares "k8s resources KHÔNG thuộc phase này" — so the phase simultaneously excludes k8s and deletes the only two k8s graph views.
- **Failure scenario:** Implementer follows the file list, removes both files, and the Kubernetes page loses its "Topology" and "X-Ray" tabs — a k8s feature regression shipped as a Docker-graph feature. Then they go to App.svelte/Sidebar.svelte looking for "2 mục cũ" to remove, find nothing, and either give up or delete an unrelated route.
- **Evidence (full consumer enumeration — 2 consumers total, both in one file):**
  - `src/pages/Kubernetes.svelte:13` — `import XRay from "./XRay.svelte";`
  - `src/pages/Kubernetes.svelte:14` — `import ClusterTopology from "./ClusterTopology.svelte";`
  - `src/pages/Kubernetes.svelte:72` — `{ id: "xray", label: "X-Ray", resource: "xray" }` (tab registration)
  - `src/pages/Kubernetes.svelte:576-577` — `{:else if activeResource === 'xray'} <XRay namespace={...} />`
  - `src/pages/Kubernetes.svelte:579` — `<ClusterTopology />`
  - `src/App.svelte:8-20` — page imports: Dashboard, Instances, Containers, Models, Images, Volumes, Networks, Compose, Kubernetes, LinuxVMs, Settings, Terminal, Help. **Neither ClusterTopology nor XRay appears.**
  - `src/components/Sidebar.svelte:31-58` — 13 menu ids; no topology/xray entry.
  - `grep -n "xray\|topology" src/locales/en.json` → **0 matches**; XRay uses inline `t('xray.*', { default: ... })` fallbacks (`src/pages/XRay.svelte:120-220`), so the "xoá chuỗi locale mồ côi" success criterion is verifying something that does not exist.
- **Contract verification result: FAILED.** Plan names 2 modify targets (App.svelte, Sidebar.svelte) that have zero references, and omits the 1 file that has all 5 references (`src/pages/Kubernetes.svelte`).
- **Suggested fix:** Drop the delete entirely. Build the Docker topology page as a new page; leave the k8s views where they are. If consolidation is genuinely wanted, that is a separate k8s-scoped phase whose only touched file is `src/pages/Kubernetes.svelte`.

---

## Finding 2: The 5 new capability ids are a feature ladder, and they do not connect to the entitlement that actually ships

- **Severity:** Critical
- **Location:** Plan B, plan.md, "Quy ước gating (áp dụng mọi phase)"
- **Flaw:** The plan mandates `compose.autofix`, `metrics.history`, `metrics.alerts`, `heal.rules`, `cluster.transfer` gated through `ProGate capability=...`. Two problems. (a) `hasCapability()` only returns true when a **sidecar** handshake declares that exact string; the shipped entitlement is a boolean `proState.paid` from the subscription cache. (b) A 5-entry per-feature capability list is precisely the "gate has to ask which tier" complexity `docs/pricing-rationale.md:24-27` refuses.
- **Failure scenario:** A paying Pro customer signs in. `proState.paid === true`, but `proStatus.state` is `"free"` because no sidecar is bundled. `hasCapability("metrics.history")` returns false → `isPaidButUnavailable` → the customer is shown a "needs_update / restore your Pro component" screen for a feature that lives entirely in the MIT core and has no sidecar. All five Pro features are unreachable for every paying customer on day one.
- **Evidence:**
  - `src/lib/pro.svelte.ts:111-113` — `return s.state === "active" && s.capabilities.includes(capability);`
  - `src/lib/pro.svelte.ts:64,103` — `proState.paid = !!s?.entitled;` (the real, account-based entitlement)
  - `src/lib/pro.svelte.ts:127-129` — `isPaidButUnavailable` returns true when `paid` but no capability → renders the "needs_update" branch (`src/components/ProGate.svelte:46-50`)
  - `src-tauri/src/pro/mod.rs:20-21` — "no sidecar is bundled yet, so the core runs the full free product"
  - `src-tauri/src/subscription/mod.rs:35-36` — "Whether Pro is entitled right now — **the only field a gate may read**"
  - `plans/260811-1930-subscription-teams-supabase-schema/plan.md:48,200-201` — "Entitlement stays a boolean — no tier concept … `ProGate` needs no edits"
  - `docs/pricing-rationale.md:24-27`
  - `src/components/ProGate.svelte:13-17` — GATED_ENUM has exactly 3 entries today; `src-tauri/src/telemetry/events.rs:30` `enum GatedCapability` must match.
- **Suggested fix:** Gate on `isPaid()` / `SubscriptionState.entitled` (one boolean, both layers). Keep capability strings as *telemetry labels only* — that is what `GATED_ENUM` already is. Delete the "add to GatedCapability enum + Rust enum for each capability" ceremony from the acceptance criteria.

---

## Finding 3: Pro Phase 1 jumps an unrun go/no-go gate and re-commits to a constraint already ruled infeasible

- **Severity:** Critical
- **Location:** Plan B, Phase 1, "Implementation Steps" 1-8 and "Success Criteria"
- **Flaw:** This phase is Bước C of an existing in-progress plan, promoted to "phase 1, the first sellable Pro feature". That plan explicitly makes Bước C **conditional** on a go/no-go gate that has not been run — no corpus exists. Plan B never mentions the gate. It also sets "Comment YAML còn nguyên sau fix deterministic" as a success criterion, which the prior red-team already concluded is not achievable with the repo's only YAML dependency.
- **Failure scenario:** Team commits to Pro Phase 1 as the launch feature, builds the strategy engine, and then discovers the ≥70%-correct bar is unreachable on real files — the exact outcome the go/no-go gate exists to detect *before* spending 5-8 days. The prior plan's warning "Không đặt auto-patch đầy đủ làm cổng launch" is silently reversed.
- **Evidence:**
  - `plans/260810-2258-colimaui-commercial-foundation/phase-05-compose-auto-fix-spike.md:44` (Bước B — cổng go/no-go), `:52` (Bước C only if passed), `:59` ("Không đặt auto-patch đầy đủ làm cổng launch")
  - same file `:123,144-147` — "Bước A: code xong, benchmark chưa chạy… Bước 1: thu ≥15 file compose lỗi thật. **Chưa có file nào.**"
  - same file `:19` — "Patch giữ nguyên comment/format là bất khả thi với dependency hiện tại… `serde_yml 0.0.12`"
  - `src-tauri/Cargo.toml:29` — `serde_yml = "0.0.12"` (unchanged; no round-trip YAML crate added)
- **Suggested fix:** Make Plan B Phase 1 `blockedBy` the go/no-go gate, and move step 1 (build the 15-20 file corpus) out into that gate where it already belongs. Downgrade comment preservation from success criterion to a documented limitation.

---

## Finding 4: Three-table downsample schema is over-engineered for a desktop app

- **Severity:** High
- **Location:** Plan B, Phase 2, "Architecture" (schema) and "Implementation Steps" 5-6
- **Flaw:** `samples_raw` + `samples_1m` + `samples_1h` + a retention task + a byte-ceiling enforcer + resolution-selecting query router — six moving parts to store metrics for at most a few dozen local containers. The plan's own justification ("864k dòng/ngày") argues against itself: raw is only kept 1 hour, so raw never exceeds ~36k rows. SQLite handles that trivially.
- **Failure scenario:** Three aggregation paths means three places for the boundary bug: a sample lands at the raw/1m cutover instant and is either double-counted into the 1m average or dropped. The 7-day chart then disagrees with the 1-hour chart for the same window, and nobody can tell which is right because there is no single source of truth. Cost: days of work plus a permanently untrustworthy chart, to solve a disk-space problem that a `DELETE WHERE ts < ?` on one table already solves.
- **Evidence:**
  - `plans/.../phase-02-metrics-time-series-store.md:42-49` (three tables), `:51` (the 864k claim), `:69` (two-stage rollup), `:70` (three-way query router)
  - `src-tauri/Cargo.toml:38` — `rusqlite 0.32` bundled; no time-series extension, so all rollup logic is hand-written SQL
- **Suggested fix:** One table, one index on `(container_id, ts)`, one retention `DELETE` by age, one capacity check. Add a rollup table only when a measured query exceeds the 200 ms criterion the plan already states.

---

## Finding 5: `metrics_store.rs` in `commands/` puts Pro feature code inside the MIT core

- **Severity:** High
- **Location:** Plan B, Phase 2, "Related Code Files"; Phase 3 `commands/alerts.rs`; Phase 4 `commands/self_heal.rs`
- **Flaw:** All three paid features are planned as modules in `src-tauri/src/commands/`, i.e. the open-source core, with a runtime `if entitled` check as the only boundary (Phase 2, step 7). The existing Pro boundary is explicitly the opposite: the core contains no feature logic. This also makes the `MetricSink` trait's stated purpose ("no `if pro` in the collector") false — the `if pro` just moves to registration in `lib.rs`.
- **Failure scenario:** Pro feature source ships in the MIT repo. Anyone deletes the entitlement check, rebuilds, and has the paid features — while the project has published that the core is MIT with no crippled build, so the code cannot be closed later without breaking that promise.
- **Evidence:**
  - `src-tauri/src/pro/mod.rs:3-6` — "Everything here ships in the open-source core… **but no Pro feature logic and no hardcoded list of what Pro can do**. The sidecar declares its own capabilities"
  - `src-tauri/src/pro/protocol.rs:11,50-51` — core routes opaque payloads, never parses them
  - `docs/pricing-rationale.md:108` — "The core app. MIT, in full, no crippled build."
  - `plans/.../phase-05-compose-auto-fix-spike.md:83` — "Bước C — repo private"
  - `src-tauri/src/commands/mod.rs` — 24 modules, all free-feature code; the plans add `metrics_store`, `metrics_retention`, `alerts`, `self_heal`, `compose_autofix`, `compose_autofix_apply` here
- **Suggested fix:** Decide the boundary once, in Plan B's plan.md, before any phase starts: either these features are core-MIT-and-boolean-gated (then say so explicitly and drop the sidecar/capability framing from Finding 2), or they go in the sidecar. The plans currently assume both.

---

## Finding 6: `FixStrategy` trait + 5 structs is a match statement wearing a costume

- **Severity:** High
- **Location:** Plan B, Phase 1, "Architecture"
- **Flaw:** Five deterministic strategies plus KB plus LLM, each a struct implementing `applies()` + `propose()`. The dispatch key is already a `&'static str` returned by an existing function, the set is fixed and closed ("Giới hạn cứng"), and there is no plugin/extension story. `applies()` duplicates the categorisation that `categorize()` already performed.
- **Failure scenario:** Every new fix requires touching a struct, a registry vector, and an ordering list — three edits where one match arm would do — and `applies()` drifts out of sync with `categorize()`, so a file is categorised `port_conflict` but no strategy claims it and the user gets "no fix available" for a category the code supposedly handles.
- **Evidence:**
  - `src-tauri/src/commands/compose_diagnose.rs:45` — `fn categorize(raw: &str) -> &'static str` (already returns a closed set of category strings)
  - `src-tauri/src/commands/compose_diagnose.rs:414` — test `categorize_buckets_common_errors` pins that closed set
  - `plans/.../phase-01-compose-auto-fix-patch-engine.md:44-49` (trait + 5 structs), `:81` (fixed ranking deterministic > KB > LLM — no dynamic dispatch need)
- **Suggested fix:** `fn propose(category: &str, yaml: &str, v: &ComposeValidation) -> Option<Patch>` with a match on `categorize()`'s output, then KB, then LLM as two more arms. Introduce a trait if and when a third dispatch source appears.

---

## Finding 7: Free Phase 1 asserts a dialog plugin that is not installed, and adds `docker load` for symmetry

- **Severity:** High
- **Location:** Plan A, Phase 1, "Architecture" and "Requirements"
- **Flaw:** The architecture block says path selection uses "tauri-plugin-dialog, **đã có trong capabilities**". It is not present in capabilities, `Cargo.toml`, or `package.json`. Separately, `docker load` import is justified as "đối xứng, gần như free vì cùng chỗ" — symmetry is not a user need, and it is the one operation in the phase that writes attacker-controlled image content into the engine.
- **Failure scenario:** Implementer wires `open()`/`save()` dialogs, gets a runtime permission denial with no compile-time error (Tauri capability failures are runtime), and the entire phase's UX premise ("chọn đường dẫn bằng dialog hệ thống") has to be redesigned mid-phase. Meanwhile `docker load` ships untested because it has no acceptance criterion.
- **Evidence:**
  - `src-tauri/capabilities/default.json:6-26` — permissions are exactly: `core:default`, `opener:default`, `updater:default`, `deep-link:default`, 3 × `core:event:*`, 4 × `core:window:*`, `http:default`. **No `dialog:*`.**
  - `grep -n "dialog\|notification" src-tauri/Cargo.toml package.json` → **0 matches** (neither plugin is a dependency)
  - `plans/.../phase-01-file-transfer-export-image-tar.md:35` (the claim), `:21` (docker load), Success Criteria `:68-72` — 5 criteria, **none covers import**
  - Same gap in Plan B Phase 3 step 1, which at least says to *check* capabilities first — Plan A does not.
- **Suggested fix:** Add "install + permission `tauri-plugin-dialog`" as an explicit step 0 with its own acceptance line. Cut `docker load` from this phase; add it when a user asks.

---

## Finding 8: Pro Phase 5 is not in the approved Pro catalogue and bypasses the Wave 2 entry gate

- **Severity:** Medium
- **Location:** Plan B, Phase 5 (whole phase); Plan B plan.md, "Overview"
- **Flaw:** The commercial-foundation plan holds a numbered Pro catalogue of 9 candidates, each carrying a principle-1 verdict. Cluster registry / cross-instance transfer / cross-cluster graph is **not among them**. It also skips Phase 7's entry conditions (revenue + ProGate/churn data before building more). Meanwhile it is the largest and highest-risk phase in either plan: a new registry, a piped `save | load` with progress and cancellation across two docker contexts, arch mismatch detection, plus a multi-instance graph mode.
- **Failure scenario:** The team spends the longest phase of the roadmap on a feature nobody has been asked for, before a single copy of Pro has been sold, and the arch-mismatch edge case (`plans/.../phase-05...md:50`) generates support load from users whose transferred images silently fail to run.
- **Evidence:**
  - `plans/260810-2258-colimaui-commercial-foundation/plan.md:92-100` — the 9-item catalogue; items 4 (self-healing) and 5 (activity history + alerts) are present, cluster transfer is absent
  - same file `:148-155` — Phase 6 Launch is **Pending**; Phase 7 Pro Wave 2 is **Pending**
  - `plans/.../phase-07-pro-wave-2.md:24` ("Điều kiện vào phase"), `:38` ("Chọn 2-3 mục **còn lại trong Danh mục Pro**"), `:78` ("Thứ tự chọn dựa trên dữ liệu ProGate/churn"), `:90` ("Wave 2 tối đa 3 mục. Muốn thêm thì mở plan mới")
  - Plan B ships **5** features — Wave 2's own cap is 3
- **Suggested fix:** **This is the single phase to cut.** Cutting Pro Phase 5 costs nothing (no dependent phase — plan.md:41 says it is independent), removes the largest effort block, and removes the only Free-phase justification for the `?instance=` parameter in Free Phase 4 step 4, which can then also be dropped as speculative. Run it through the principle-1 gate and the Wave 2 entry conditions if it ever comes back.

---

## Finding 9: Two of the five seeded healing rules do not earn the executor machinery

- **Severity:** Medium
- **Location:** Plan B, Phase 4, "Requirements" rules 3-4, "Implementation Steps" 4-5
- **Flaw:** Rules 3 (disk > X% → suggest prune) and 4 (OOM-killed → suggest raising mem limit) have **no automatic action** — both are permanently advisory. They therefore use none of what makes this phase expensive: no `HealAction` execution, no per-hour quota, no auto-mode confirmation dialog, no global kill switch. They are two notification cards being routed through a rules engine. Rule 4 additionally hard-couples the phase to Pro Phase 2's 7-day history for a `peak × 1.3` heuristic.
- **Failure scenario:** The seeded rule set makes the settings page look like an orchestrator config, so users toggle rules 3-4 to "Auto" expecting action, and get nothing — the mode switch is a no-op for those two rules. The plan has no criterion covering a suggest-only action in auto mode.
- **Evidence:**
  - `plans/.../phase-04-self-healing-rules.md:24-26` (rules 3-4 both "đề xuất"), `:43-44` (`HealAction` includes `SuggestPrune`, `SuggestMemLimit` alongside real actions), `:73` (mem suggestion needs Phase 2 history), `:83` ("5 kịch bản giả lập: mỗi cái **phục hồi đúng ở chế độ Auto**" — impossible for rules 3-4)
  - Phase dependency chain: `phase-04:6` `dependencies: [3]`, `phase-03:6` `dependencies: [2]` — a 3-phase-deep chain before rule 1 can fire
- **Suggested fix:** Seed 2 rules — unhealthy→restart and crash-loop→stop. Both are real actions and both exercise the quota/log/kill-switch machinery. Ship disk and OOM as plain notifications from the alerts phase, with no rule row.

---

## Phase granularity

Nine phases across two plans is too many for the amount of independent value. Concretely:

- **Plan B phases 2 + 3 should be one phase.** Phase 2 is explicitly "không có UI và không bán riêng" (`phase-02:14`) and Phase 3 is `dependencies: [2]` with no other consumer. A storage layer with no user-visible output and exactly one consumer is not a phase; it is the first half of the alerts phase. Splitting them guarantees a "done" phase that ships zero value and cannot be validated except by its own successor.
- **Plan A phases 1 and 2 are genuinely independent and correctly sized.**
- After cutting Pro Phase 5 (Finding 8) and merging 2+3, the roadmap is Free ×4 (or ×3 without `docker load`) + Pro ×3 — which fits inside the Wave 2 cap of 3 items that the existing plan already committed to.

---

## Cheapest cut, biggest saving

**Pro Phase 5** (cluster registry + transfer + cross-cluster graph). Zero dependent phases, largest scope, highest operational risk, and the only phase whose feature is absent from the agreed Pro catalogue. Cutting it also lets Free Phase 4 drop its speculative `?instance=` parameter.

## Unresolved questions

1. Are these Pro features core-MIT-with-boolean-gate, or sidecar? Every phase in Plan B is ambiguous on this and the answer changes the file layout of all five (Finding 5).
2. Who decided cluster transfer is a Pro feature, and when? It contradicts the catalogue in the commercial-foundation plan without an override note (that plan uses explicit "Ghi đè quyết định" entries elsewhere, e.g. `plan.md:64`).
3. Free Phase 4 replaces two k8s views with a Docker graph — is the k8s topology/X-Ray functionality intended to survive at all?
