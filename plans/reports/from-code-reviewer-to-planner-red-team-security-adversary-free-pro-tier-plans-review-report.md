# Red-team review — Security Adversary perspective

Plans reviewed: `plans/260811-2241-free-tier-foundation/`, `plans/260811-2245-pro-tier-features/`
Reviewer role: fact-checking hostile reviewer. No code changed.

## Finding 1: Every new endpoint inherits an API token that is handed out unauthenticated

- **Severity:** Critical
- **Location:** Plan A, Phase 1 "Architecture"; Plan A Phase 2 route `POST /api/diagnostics/bundle`; Plan B Phase 1 `/api/compose/autofix/apply`; Plan B Phase 4 `routes/self_heal.rs`
- **Flaw:** Both plans add HTTP routes behind `auth_middleware` and treat that as the trust boundary. `GET /api/auth/token` is in the *public* router and returns the full bearer token to any caller. CORS only constrains browsers; a non-browser local process just reads it.
- **Failure scenario:** A malicious npm postinstall script running as the user does `GET 127.0.0.1:11420/api/auth/token`, then `POST /api/containers/cp` with `direction=from_container, dest=~/.ssh/authorized_keys` (Plan A Phase 1), or `POST /api/compose/autofix/apply` with an attacker-supplied `new_content` and an arbitrary `file_path` (Plan B Phase 1), or `POST /api/self_heal` to flip every rule to `Auto` and restart the VM (Plan B Phase 4). Today the worst an attacker gets from this port is container control; these plans upgrade it to arbitrary host file write and unattended destructive automation. Neither plan mentions it.
- **Evidence:** `src-tauri/src/api_server.rs:256` (`/api/auth/token` in the unauthenticated router), `src-tauri/src/routes/system.rs:263-265` (`ok(get_api_token())`), `src-tauri/src/api_server.rs:252` (auth layer applies only to the protected router), `src-tauri/src/api_server.rs:53-58` (CORS = browser-only control).
- **Suggested fix:** Before either plan lands write-capable routes, decide the token-distribution model (e.g. token file with 0600 perms read by the webview at boot, drop `/api/auth/token`). Until then, file-write and self-heal mutation routes must be Tauri commands only, not HTTP routes. Add this as an explicit dependency in both plan.md files.

## Finding 2: `redact.rs` cannot satisfy Plan A Phase 2's own success criteria

- **Severity:** Critical
- **Location:** Plan A, Phase 2, "Implementation Steps" step 9 + "Success Criteria" bullet 1
- **Flaw:** The plan asserts a fixture containing "AWS key, bearer token, password trong env, đường dẫn `/Users/<tên>`" must not leak, and lists modifying `redact.rs` only conditionally ("bổ sung pattern nếu test lộ thiếu sót"). Three of those four classes are not covered at all today.
- **Failure scenario:** Bundle includes container logs. A Postgres container logs `POSTGRES_PASSWORD=hunter2` — `QUERY_PARAM` requires a leading `?` or `&`, so it does not match. An app logs `AKIAIOSFODNN7EXAMPLE` / `aws_secret_access_key` — no shape in `KEY_SHAPES`. A stack trace contains `/Users/jane.doe/work/acme-secret-project/...` — nothing scrubs home paths, so the bug report deanonymises the user and their employer's project names. A JWT (`eyJ...`) or a PEM private key in a log line passes through untouched. The user reviews a preview that *looks* redacted (`<redacted>` appears elsewhere), trusts it, and pastes it into a public GitHub issue.
- **Evidence:** `src-tauri/src/redact.rs:26` (query-param regex anchored on `[?&]`), `src-tauri/src/redact.rs:54-72` (`KEY_SHAPES` = sk-/AIza/gsk_/hf_/gh[pousr]_/colima- only; no AKIA, no JWT, no PEM), `src-tauri/src/redact.rs:171-174` (`leaves_ordinary_errors_untouched` documents the pass-through default). No home-path handling anywhere in the file.
- **Suggested fix:** Promote "extend `redact.rs`" from conditional to a required, first step of Phase 2 with a named pattern list (AKIA/ASIA, `eyJ` JWTs, `-----BEGIN * PRIVATE KEY-----`, `(?i)(password|passwd|secret|token)\s*[:=]\s*\S+` for log/env lines, `$HOME` → `~`), and make the fixture test a merge blocker. Additionally: container logs are the highest-risk section — make it opt-*in*, not `included_by_default`.

## Finding 3: Pro auto-fix will either destroy the user's secrets or exfiltrate them — the plan picks neither

- **Severity:** Critical
- **Location:** Plan B, Phase 1, "Architecture" (`LlmStrategy → dùng llm_payload_preview đã có`) + step 5 ("yêu cầu model trả về file đầy đủ, validate bằng `compose_validate`")
- **Flaw:** `llm_payload_preview` is built from `redact_compose()`, which replaces every `environment:`, `secrets:` and `env_file:` block with `# [redacted]`. The plan then asks the model to return the **full file** and writes it to disk.
- **Failure scenario:** User runs auto-fix on a compose file with 40 env vars and DB credentials. The model only ever saw `environment: # [redacted]`, so the "full file" it returns has literally that. `compose_validate` runs `docker compose config --quiet`, which accepts a service with no environment block — **the patch validates clean**. The diff shows removed lines, but a user who trusts a "≥70% correct" feature clicks Apply and silently loses every env var and secret reference in the file. Plan B Phase 1's absolute criterion "0 file bị làm hỏng thêm" is measured by `compose_validate`, which cannot detect this class of damage. The only obvious workaround — send the raw YAML — ships the user's production credentials to a third-party LLM, which is exactly what `redact_compose` was written to prevent.
- **Evidence:** `src-tauri/src/commands/compose_diagnose.rs:293` (`build_llm_preview(..., &redacted)`), `redact_compose` in the same file (blanks `environment:`/`secrets:` blocks and marks `env_file: # [not inlined]`), `src-tauri/src/commands/compose_diagnose.rs:212-256` (`compose_validate` = `docker compose config --quiet`; structural only).
- **Suggested fix:** Make `LlmStrategy` return a *bounded* edit (line range + replacement) that is re-applied to the original unredacted file locally, never a whole-file replacement. Add a hard pre-apply invariant: every redacted region present in the original must be byte-identical in the candidate, else reject the patch. Add that invariant to Success Criteria; the current `compose_validate` gate is not sufficient and the plan claims it is.

## Finding 4: The "backend gate" for Pro is a client-supplied, unsigned JSON blob

- **Severity:** High
- **Location:** Plan B, plan.md "Quy ước gating" ("Gate ở UI + backend"); Plan B Phase 2 step 7 ("chỉ đăng ký khi entitlement Pro; đây là điểm gate phía backend")
- **Flaw:** The only backend entitlement source that persists is `subscription::cache`, and it is populated by whatever the caller POSTs. There is no signature, no server verification, no binding to anything the app can check offline.
- **Failure scenario:** `POST /api/subscription/store {"user_id":"x","entitled":true,"tier":"pro_teams","expires_at":"2099-01-01"}` (token from Finding 1, or from a devtools console in the webview) permanently entitles the machine. Every phase that gates on `proState.paid` / `subscription_state` — Phase 2's SQLite sink, Phase 3's alerts, Phase 5's `cluster.transfer` — unlocks. The genuinely unforgeable signal is `pro::ProStatus::Active{capabilities}`, which comes from the sidecar handshake, but Phase 2's gate is written against entitlement, not capability.
- **Evidence:** `src-tauri/src/subscription/mod.rs:99-107` (`EntitlementPayload` is plain `Deserialize`), `src-tauri/src/subscription/mod.rs:125-144` (`subscription_store` writes it verbatim), `src-tauri/src/subscription/mod.rs:176-184` (`api_subscription_store` exposes it over HTTP), `src-tauri/src/pro/mod.rs:25-48` (the capability-based status that is *not* used as the gate).
- **Suggested fix:** State in Plan B plan.md that `subscription` cache is a *display/UX* signal and that every backend gate must key off `ProStatus::has_capability()`. If a feature has no sidecar implementation, it is not gateable and should not claim a backend gate. Either that, or add signature verification of the oracle payload as an explicit blocking prerequisite phase.

## Finding 5: Self-healing keeps executing after entitlement lapses, with its kill switch behind the gate

- **Severity:** High
- **Location:** Plan B, Phase 4, "Requirements" ("Công tắc tắt toàn cục") + "Related Code Files" (`Settings.svelte` … `bọc ProGate capability="heal.rules"`); Plan B Phase 2 risk row ("Người dùng huỷ Pro… chỉ ngừng ghi và ngừng cho truy vấn qua UI")
- **Flaw:** No phase specifies what happens to `Auto`-mode rules when entitlement expires, when the sidecar is missing, or when `ProStatus` becomes `NeedsUpdate`. Meanwhile `ProGate` in the `locked` state renders an upsell button *instead of* its children.
- **Failure scenario:** User enables "unhealthy > 5 min → restart" in Auto, then their card fails / they go offline past cache expiry. `hasCapability("heal.rules")` returns false, so the Self-healing settings page and the global kill switch render as a "Buy Pro" button. The `HealExecutor` — a backend tokio task with no entitlement check specified anywhere in Phase 4 — keeps restarting containers and, per rule 5, restarting the colima VM. The user cannot see the rule list, cannot see the log, and cannot turn it off from the app. This is Plan B's self-declared "highest risk on the whole roadmap" occurring in the one state the plan never models.
- **Evidence:** `src/components/ProGate.svelte:45-51` (`view` derivation), `src/components/ProGate.svelte:63-80` (`unlocked` → children; `locked` → upsell button, children never rendered), `src/lib/pro.svelte.ts:111-113` (`hasCapability` false unless `state === "active"`), `src-tauri/src/subscription/cache.rs:54` (`is_entitled_at` — expiry is a hard cliff). Phase 4 lists no entitlement check in `HealExecutor`.
- **Suggested fix:** Add a required rule to Phase 4: on any transition out of entitled, all rules demote to `Suggest` and the executor logs the demotion. The global kill switch and the heal log must live *outside* `ProGate` (they are safety controls, not features). Add a success criterion for the entitlement-lapse transition.

## Finding 6: Plan A Phase 1's core security control depends on a plugin that is not installed

- **Severity:** High
- **Location:** Plan A, Phase 1, "Architecture" — "dialog chọn path (tauri-plugin-dialog, đã có trong capabilities)"
- **Flaw:** False. `tauri-plugin-dialog` is absent from `Cargo.toml`, `package.json`, and `capabilities/default.json`. The phase's implicit threat model ("user picked the path via a trusted OS dialog, therefore the path is trusted") therefore rests on code that does not exist, and the phase budgets no work for adding it (new dependency + capability entry + permission review).
- **Failure scenario:** Implementer discovers the gap mid-phase and takes the shortcut: accept the path as a string from the frontend. The API route now accepts any host path from any caller, and Finding 1 becomes arbitrary file write. Even with the plugin, the HTTP route is reachable without the dialog — the dialog is a UX affordance, never an authorization control.
- **Evidence:** `src-tauri/capabilities/default.json` (permissions list: core, opener, updater, deep-link, http — no `dialog`), `src-tauri/Cargo.toml:23,41,42,45` (opener, http, updater, deep-link only), `package.json` (no `@tauri-apps/plugin-dialog`).
- **Suggested fix:** Correct the claim, add "install + configure tauri-plugin-dialog" as step 0, and state explicitly that the dialog is not a trust boundary — the backend must independently confine destinations to an allowed root.

## Finding 7: `validation.rs` has no function that does what Plan A Phase 1 asks of it

- **Severity:** High
- **Location:** Plan A, Phase 1, "Bảo mật — điểm dễ sai nhất của phase này" and step 3; Plan A plan.md "Nền tảng tái dùng" row (`validation.rs` → Phase 1 validate đường dẫn)
- **Flaw:** The plan treats `validation.rs` as an existing path validator. It contains `contains_shell_injection` (a metacharacter denylist) and `assert_path_within(base, candidate)` (requires a base directory the plan never names). Rejecting `..` is not confinement: `/Users/victim/.ssh/authorized_keys` and `/Users/victim/Library/LaunchAgents/x.plist` contain no `..` and pass every existing check.
- **Failure scenario:** Implementer wires `contains_shell_injection(dest)` and ticks the success criterion "Path chứa `../` bị từ chối", believing the phase is secure. `docker cp <ctr>:/evil /Users/victim/Library/LaunchAgents/persist.plist` succeeds → persistence. Separately, the denylist bans `$` and `..`, which breaks legitimate macOS paths and relative paths, so the first bug report will pressure someone into loosening it.
- **Evidence:** `src-tauri/src/validation.rs:7-21` (`contains_shell_injection`: bans `; \` $ && || $( ${ .. >/`), `src-tauri/src/validation.rs:174-205` (`assert_path_within` needs an explicit base). `src-tauri/src/commands/compose_diagnose.rs` `ensure_compose_path` is the existing precedent — extension allowlist + injection check, still no confinement.
- **Suggested fix:** Name the allowed roots in the phase (e.g. `$HOME/Downloads`, `$HOME/Desktop`, plus an explicit user-configured directory) and require `assert_path_within` against them for every host-side destination. Add a success criterion using an absolute path with no `..` that must still be rejected.

## Finding 8: Plan A Phase 2 misstates `crash.rs` — there is nothing on disk to read

- **Severity:** Medium
- **Location:** Plan A, Phase 2, "Related Code Files" ("thêm hàm đọc crash report gần nhất, hiện chỉ có ghi") and step 1 (`pub fn latest_report() -> Option<String>` "đọc crash report đã lưu")
- **Flaw:** `crash.rs` never persists anything. The panic hook builds a redacted string and `eprintln!`s it; the module doc says transmission and sinks are deferred. `latest_report()` is not "add a reader", it is "design crash persistence": file location, rotation, size cap, and the fact that a crash file on disk is a new at-rest artifact carrying whatever `redact()` misses (see Finding 2).
- **Failure scenario:** Phase estimated as trivial; implementer adds a quick `~/Library/Application Support/.../last-crash.txt` with no rotation and no size cap, redacted only by the incomplete `redact()`. The file accumulates and is now a persistent, unencrypted secret store that Spotlight indexes.
- **Evidence:** `src-tauri/src/crash.rs:35-41` (`redacted_report` returns a String), `src-tauri/src/crash.rs:48-55` (`install` → `eprintln!` only), `src-tauri/src/crash.rs:12-15` (module doc: "Transmission is deferred (no endpoint yet)"). No `fs::write` in the file.
- **Suggested fix:** Rewrite the step as "add crash persistence" with explicit decisions on path, single-file overwrite, size cap, and a note that Finding 2's redaction work must land first.

## Finding 9: Plan B Phase 5's transfer mechanism has no basis in this codebase

- **Severity:** Medium
- **Location:** Plan B, Phase 5, "Architecture" (`docker --context <from> save <image> | docker --context <to> load`) and step 1/3
- **Flaw:** `--context` appears nowhere in the repo. Engine selection is done by pointing at a per-instance unix socket (`unix:///Users/…/.colima/<profile>/docker.sock`). Whether colima registers docker contexts is environment-dependent and unverified by the plan; the whole phase, including the arch-mismatch guard ("lấy từ `docker info` qua context đó"), is built on it.
- **Failure scenario:** Contexts are absent or named differently on the user's machine; `docker --context colima-work` errors, or worse resolves to the *default* context, so "transfer from A to B" silently loads the image back into A. Secondarily, `image` is a user-supplied string interpolated into an argv position with no validation named in the phase — `is_valid_container_id` is the closest existing helper and it permits characters that make `--flag`-shaped arguments possible.
- **Evidence:** `grep -rn -- "--context" src-tauri/src/` → no matches. `src-tauri/src/docker_state.rs:11-22` and `src-tauri/src/adapters/docker.rs:13` (socket-URL-based host selection). `src-tauri/src/validation.rs:24-30` (`is_valid_container_id` allows `/ : . - _` and alphanumerics).
- **Suggested fix:** Step 1 must verify context availability on a real machine and define the fallback (explicit `DOCKER_HOST=unix://…` per side, consistent with `docker_state.rs`). Add explicit validation of `image` against a `repo[:tag][@sha256:…]` grammar, and a `--` separator before positional args.

## Finding 10: Gating convention references a closed telemetry enum without acknowledging the consequence

- **Severity:** Medium
- **Location:** Plan B, plan.md "Quy ước gating" ("phải thêm vào `GATED_ENUM` … **và** enum `GatedCapability` phía Rust")
- **Flaw:** The instruction is factually correct about the two locations, but `GatedCapability` is a closed telemetry enum on the wire. Adding five ids (`metrics.history`, `metrics.alerts`, `heal.rules`, `cluster.transfer`, plus autofix already present) changes the telemetry event schema, which is consent-gated and documented as a deliberately closed vocabulary. No phase owns that change or the doc update.
- **Failure scenario:** Two phases add variants independently, the ingest side rejects unknown values, and the Pro funnel data is silently wrong for the entire launch — while every phase's checklist reads green because the string exists in both files.
- **Evidence:** `src/components/ProGate.svelte:13-17` (`GATED_ENUM` currently has 3 entries: compose.autofix, compose.diagnose, dockerfile.optimize), `src-tauri/src/telemetry/events.rs:30` (`pub enum GatedCapability`), `src-tauri/src/telemetry/events.rs:55` (`ProGateReached { capability: GatedCapability }`), `src-tauri/src/telemetry/mod.rs:103`.
- **Suggested fix:** Assign the enum extension + `docs/telemetry.md` update to one phase (Phase 2, the earliest), and add "unknown capability id is dropped, not sent as a free string" as an explicit acceptance item.

---

## Verification Results

Claims sampled: 31 · VERIFIED: 22 · FAILED: 6 · UNVERIFIED: 3

**Verified (representative):**
- `redact::redact`, `redact_err` — `src-tauri/src/redact.rs:79,98`
- `crash::redacted_report` — `src-tauri/src/crash.rs:35`
- `all_container_stats`, `container_top` — `src-tauri/src/commands/containers.rs:606,624`
- `publish_sse_event` — `src-tauri/src/sse.rs:29`
- `error_signature` at line 83, `categorize` at line 45 — `src-tauri/src/commands/compose_diagnose.rs` (plan's "dòng 83" is exact)
- `src/pages/ClusterTopology.svelte`, `src/pages/XRay.svelte` both exist
- `rusqlite` present, no new dep needed — `src-tauri/Cargo.toml:38`
- `compose.autofix` already declared — `src/components/ProGate.svelte:14`
- `instance_reader` exposes `arch` — `src-tauri/src/instance_reader.rs:34,178`
- `subscription/cache.rs` handles offline expiry — `src-tauri/src/subscription/cache.rs:54,81`
- `src-tauri/src/validation.rs`, `assert_path_within` — exists (but see Finding 7)
- `src/lib/external-links.ts` with `openExternal` — `:93`
- `scripts/compose-diagnose-benchmark.sh` — exists
- `routes/compose.rs` is the right file to extend — `:62`

**Failed:**
1. Plan A P1: "tauri-plugin-dialog, đã có trong capabilities" — not in `capabilities/default.json`, `Cargo.toml`, or `package.json`. (Finding 6)
2. Plan A P2: "`crash.rs` … hiện chỉ có ghi" — it does not write; `eprintln!` only, `src-tauri/src/crash.rs:48-55`. (Finding 8)
3. Plan A P1/plan.md: "`validation.rs` → validate đường dẫn" — no path-confinement API usable without a named base. (Finding 7)
4. Plan B P5: `docker --context` — zero occurrences in the repo; engine selection is socket-based. (Finding 9)
5. Plan B P2 step 7 / plan.md: "gate ở backend" via entitlement — the entitlement record is client-supplied and unsigned. (Finding 4)
6. Plan B P1 step 5: reusing `llm_payload_preview` to obtain a full replacement file — the preview is secret-stripped, so the round-trip is lossy. (Finding 3)

**Unverified (ambiguous, needs the author to state intent):**
- Plan A P3: "`sse.rs` đã theo dõi được số connection" — `sse.rs` exposes `get_sse_tx`/`publish_sse_event`; whether `broadcast::Sender::receiver_count()` is an acceptable subscriber signal is a design call, not a fact in the repo.
- Plan A P4: "d3-force nếu đã có" — no `d3` in `package.json`; plan's conditional resolves to "self-write", but the phase does not say so definitively.
- Plan B P3 step 1: notification permission — `capabilities/default.json` has none, so the plan's "check first" branch resolves to "add a plugin", which the phase does not budget.
