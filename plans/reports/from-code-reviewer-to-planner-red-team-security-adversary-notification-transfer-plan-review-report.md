# Red Team Review — Security Adversary — Notification Center & Background Transfers plan

Reviewer role: Fact Checker + Security Adversary. Plan under review:
`/Users/longnd/Desktop/vnknowledge/colima-ui/plans/260812-1247-notification-center-background-transfers/`

---

## Finding 1: `link_url` is a vendor-controlled URI handed to the OS with no scheme or host allowlist
- **Severity:** Critical
- **Location:** Phase 5, "Architecture" (`link_url text` column) + Phase 3, "Architecture" (`kind="announcement" → tiêu đề + link ngoài`)
- **Flaw:** The plan adds a free-form `link_url` column and renders it as an "external link", but never specifies validation. The only external-open helper in this repo, `openExternal()`, passes the string straight to the Tauri opener plugin with zero scheme checking, and `opener:default` is already granted in the capability set.
- **Failure scenario:** Anyone who obtains write access to the announcements table (leaked `service_role` key, compromised Supabase project, malicious insider, or a mistyped row) sets `severity='critical'`, `title='Security update required'`, `link_url='file:///Applications/…'` or a custom scheme such as `x-apple-…`/`ms-…`/`itms-apps://`. Every installed client renders a vendor-branded critical alert and, on click, hands that URI to the OS handler outside the webview and outside the CSP. `https://` phishing is the low bar; non-http schemes reach local handlers. The plan's own risk table only considers "announcement becomes an ad channel" — it never considers the table as an attacker-controlled input into a privileged sink.
- **Evidence:**
  - `/Users/longnd/Desktop/vnknowledge/colima-ui/src/lib/external-links.ts:110-122` — `openExternal(url)` does no scheme/host validation; `openUrl(url)` then `window.open(url, ...)`.
  - `/Users/longnd/Desktop/vnknowledge/colima-ui/src-tauri/capabilities/default.json:10` — `"opener:default"` granted.
  - `/Users/longnd/Desktop/vnknowledge/colima-ui/src-tauri/tauri.conf.json:35` — CSP constrains the webview but does not constrain the opener plugin.
  - Phase 5 file, lines 36-49 — `link_url text` with no constraint; no validation step in "Implementation Steps" (lines 86-97).
- **Suggested fix:** Add a DB `check (link_url is null or link_url ~ '^https://')`, an app-side allowlist of vendor hosts (e.g. the app's own domain + `polar.sh`), display the resolved host next to the link, and require an explicit confirm for any host not on the list. Do not reuse bare `openExternal` for third-party-controlled strings.

## Finding 2: Announcement `title`/`body` are untrusted strings, and the plan never mandates plain-text rendering
- **Severity:** High
- **Location:** Phase 5, "Architecture" (`title jsonb not null`, `body jsonb`) + Phase 3, "Implementation Steps" step 3
- **Flaw:** The plan describes rendering the announcement inside `NotificationItem.svelte` but never states that `title`/`body` must be rendered as text. This repo has an established `{@html}` + markdown-render precedent that an implementer following "make the body look nice" will reach for.
- **Failure scenario:** Implementer renders `body` through the existing markdown path. A malicious/compromised row injects HTML — inline styles, an overlay that covers the panel, a fake "Enter your license key" form posting to an attacker endpoint. `script-src 'self'` blocks script execution, so this is UI-spoofing/credential-phishing rather than RCE, which is why it is High and not Critical. There is also no length cap: a 5 MB `body` renders into a `$state` array capped at 100 entries.
- **Evidence:**
  - `/Users/longnd/Desktop/vnknowledge/colima-ui/src/components/AiChatPanel.svelte:490,495,500` — `{@html renderMarkdownHTML(msg.content)}` precedent.
  - `/Users/longnd/Desktop/vnknowledge/colima-ui/src/styles/features.css:461` — comment confirming markdown is injected with `{@html}` as a project pattern.
  - `/Users/longnd/Desktop/vnknowledge/colima-ui/src-tauri/tauri.conf.json:35` — `script-src 'self'` (mitigates script, not markup/phishing); `img-src` excludes arbitrary remote hosts (blocks tracking pixels — the one thing that is already covered).
  - Phase 5, lines 86-97 — no sanitization or length-limit step.
- **Suggested fix:** State explicitly: announcements render as plain text only (Svelte interpolation, no `{@html}`), `title` ≤ 120 chars and `body` ≤ 2000 chars enforced both by DB `check` and client truncation, and `.limit(20)` on the select.

## Finding 3: The `redact()` guarantee in Phase 1 and Phase 4 does not hold — the transfer failure path never touches `redact()`
- **Severity:** High
- **Location:** Phase 1, "Requirements → Non-functional" ("mọi text đi qua `redact()` một lần, ở tầng store") and Phase 4, "Requirements → Non-functional" ("Nội dung notification đi qua `redact()`")
- **Flaw:** Three separate gaps. (a) The plan does not put `redact()` in the store — it relies on `globalToast` having already redacted, but Phase 2 routes `transfer.failed` into the store **directly from `transferEvents.ts`**, bypassing `globalToast` entirely. (b) That payload is raw Docker stderr, unredacted on the Rust side. (c) Even where `redact()` runs, the structured `error` object is passed through untouched.
- **Failure scenario:** `docker load -i /Volumes/ClientWork/acme-prod/db-image.tar` fails. Rust emits `transfer.failed` with `"docker failed: <raw stderr containing the absolute path and any registry auth error>"`. Phase 2 pushes that into the store as `detail`; Phase 4 forwards `settleJob` output to the macOS Notification Center, where it is persisted in the OS notification database and shown on the lock screen. `redact()` masks `/Users/<name>` and `/home/<name>` only — `/Volumes/...`, `/opt/...`, container names, and image references survive intact. This directly contradicts the plan-level acceptance criterion "Không có đường dẫn/tên container/tên image nào rời máy người dùng."
- **Evidence:**
  - `/Users/longnd/Desktop/vnknowledge/colima-ui/src-tauri/src/commands/file_transfer.rs:299-307` — `emit(app_ref, "transfer.failed", json!({ "jobId": job_id, "error": e }))`; `e` is the raw error string.
  - `/Users/longnd/Desktop/vnknowledge/colima-ui/src-tauri/src/streaming_cmd.rs:296` — `Err(format!("{} failed: {}", program, collect_stderr(stderr)))` — raw child stderr.
  - `grep -n redact src-tauri/src/commands/file_transfer.rs` → no matches; `grep -n redact src-tauri/src/sse.rs src-tauri/src/helpers.rs` → no matches.
  - `/Users/longnd/Desktop/vnknowledge/colima-ui/src/lib/redact.ts:69` — `HOME_PATH = /\/(Users|home)\/([^/\s:,;"')\]]+)/g` — no other path family is masked.
  - `/Users/longnd/Desktop/vnknowledge/colima-ui/src/lib/globalToast.ts:120` — `error: options?.error` stored without redaction (only `text` at :101 and `hint` at :121 are redacted).
- **Suggested fix:** Redact at the store boundary as the plan claims, not upstream: `pushNotification`/`settleJob` must run `redact()` on `title` and `detail` and on any string field of `error` before storage. For Phase 4, do not pass `detail` to the OS at all — send only a fixed template string ("Export finished") as the phase's own non-functional requirement already demands, which contradicts Phase 1's `detail` definition ("đường dẫn đích, tên container").

## Finding 4: Moving copy-out onto the `ToFile` sink makes cancellation delete a pre-existing user file
- **Severity:** High
- **Location:** Phase 2, "Phần B → B3", the `JobPaths { sink: Some(dest_tar), ..Default::default() }` change
- **Flaw:** The plan celebrates that `streaming_cmd` "đã tự dọn `ToFile` sink khi huỷ" and therefore deletes the manual `cleanup_on_abort`. But the sink cleanup is an unconditional `remove_file(path)` on cancel, non-zero exit, or spawn failure — it does not know whether the app created the file or whether it already belonged to the user.
- **Failure scenario:** User runs copy-out with `overwrite: true` onto an existing `backup.tar` (a legitimate flow the API and dialog both support — `resolve_destination` explicitly permits it). `File::create` truncates it, the user hits Cancel two seconds later, and `stream_to_file` calls `remove_file`. The user's original archive is now gone, and the failure path reports only "cancelled". Today's copy-out code path does not have this shape because `docker cp` writes the destination itself.
- **Evidence:**
  - `/Users/longnd/Desktop/vnknowledge/colima-ui/src-tauri/src/streaming_cmd.rs:259` — `std::fs::File::create(path)` (truncating), then `:267,:276,:288,:297` — `let _ = std::fs::remove_file(path);` on every failure/cancel branch.
  - `/Users/longnd/Desktop/vnknowledge/colima-ui/src-tauri/src/commands/file_transfer.rs:118-123` — `candidate.exists() && !overwrite` → overwrite is an accepted mode.
  - `/Users/longnd/Desktop/vnknowledge/colima-ui/src-tauri/src/commands/file_transfer.rs:507-515` — current copy-out has no sink, by deliberate design documented in-code.
- **Suggested fix:** Write to a sibling temp file (`<name>.tar.part`) created with `create_new(true)` and rename on success. That satisfies the plan's acceptance criterion "huỷ không để lại rác" without the delete-someone-else's-file failure mode, and it also removes the truncate-then-fail window for export.

## Finding 5: `container_id` is never flag-checked, so the first positional argument to `docker cp` can start with `-`
- **Severity:** High
- **Location:** Phase 2, "Phần B → B3 → Chi tiết cần cẩn thận" ("Chỉ `container_path` và `container_id` mới là input cần validate")
- **Flaw:** The plan reasons carefully about `reject_flag_like` and the literal `-` sink, then asserts `container_id` is validated. It is not validated by `reject_flag_like` — `is_valid_container_id` explicitly allows `-`, `.`, `/`, and `:` anywhere, including position 0. The composed argument is `format!("{}:{}", container_id, container_path)`, so a leading `-` in `container_id` makes the whole token flag-shaped.
- **Failure scenario:** An authenticated API client (or any code path that reaches `start_copy_from_container` with an id not sourced from the container list) sends `container_id: "-a"`. The argv becomes `["cp", "-a:/etc/passwd", "-"]`. Docker's flag parser consumes the token as shorthand flags rather than a source spec; the outcome depends on the runtime's parser, and under the new design whatever the runtime emits on stdout is written verbatim into the user's `.tar`. The plan's confidence that "chỉ `container_path`" needs the flag check is factually wrong, and its stated mitigation is therefore incomplete.
- **Evidence:**
  - `/Users/longnd/Desktop/vnknowledge/colima-ui/src-tauri/src/validation.rs` — `is_valid_container_id`: `c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.' || c == '/' || c == ':'` — no position constraint.
  - `/Users/longnd/Desktop/vnknowledge/colima-ui/src-tauri/src/commands/file_transfer.rs:491-506` — only `is_valid_container_id` on the id; `require_plain` (which calls `reject_flag_like` at :94) is applied to `container_path` only.
  - `/Users/longnd/Desktop/vnknowledge/colima-ui/src-tauri/src/commands/file_transfer.rs:75-80` — `reject_flag_like` checks `value.starts_with('-')` on the *whole* argument.
- **Suggested fix:** Add `reject_flag_like("Container id", &container_id)` and, more robustly, insert a `--` terminator: `vec!["cp", "--", &src_spec, "-"]`. The `--` makes the plan's "the `-` is a code-generated constant so it may skip validation" argument actually safe instead of merely asserted.

## Finding 6: TAR magic-byte sniffing is presented as a gate but is TOCTOU-defeatable and the plan's own fallback nullifies it
- **Severity:** Medium
- **Location:** Phase 2, "Phần B → B2" and Success Criteria ("Import một file `.gz`/`.zip` bị từ chối **trước** khi gọi runtime")
- **Flaw:** The sniff reads bytes from a user-supplied path, then a separate `docker load -i <same path>` re-opens it later in a spawned thread. The plan then adds a fallback that accepts "đuôi là `.tar` và 512 byte đầu đọc được" and instructs "ưu tiên cho qua khi không chắc". The result is a check that any non-TAR file with a `.tar` name passes, and that a file swapped between sniff and load bypasses entirely.
- **Failure scenario:** Path is checked at T0 and opened by the runtime at T1. Anything writable by another local process (`/tmp`, a synced folder, a network mount) can be replaced in between, so the "rejected before calling runtime" criterion is not a guarantee — it is a UX nicety. Worse, the acceptance criterion invites future code to treat `sniff_tar` as a security control on the `docker load` input, which parses attacker-supplied archive structure inside the VM.
- **Evidence:**
  - `/Users/longnd/Desktop/vnknowledge/colima-ui/src-tauri/src/commands/file_transfer.rs:127-134` — `require_existing_file` does `is_file()` at validation time only.
  - `/Users/longnd/Desktop/vnknowledge/colima-ui/src-tauri/src/commands/file_transfer.rs:389-403` — `start_image_load` validates, then `spawn_job` runs the command asynchronously afterwards.
  - Phase 2, lines 72-75 — the explicit "giữ mềm / ưu tiên cho qua" fallback.
- **Suggested fix:** Keep the sniff, but relabel it in the plan and in the code doc comment as a mistake-catcher, not a validation gate. Delete "trước khi gọi runtime" from the acceptance criterion or replace it with "rejects the common wrong-file cases". Sniff from an already-open file handle and pass that handle to the child if you want the check to actually bind.

## Finding 7: Dangling-symlink escape in the destination path now applies to copy-out as well
- **Severity:** Medium
- **Location:** Phase 2, "Phần B → B3" (reuse of `resolve_destination` + sink) — inherited, but the phase widens its blast radius
- **Flaw:** `assert_path_within` canonicalizes the candidate only when `candidate.exists()`. A symlink pointing at a non-existent target reports `exists() == false`, so the check falls back to `parent.canonicalize().join(file_name)`, which trivially stays inside the base. `File::create` then follows the symlink and writes outside the confined directory. The same broken-symlink case also defeats the `candidate.exists() && !overwrite` guard.
- **Failure scenario:** A shared or world-writable destination folder (e.g. `/tmp/exports`, a Dropbox-synced folder) contains a dangling symlink `containers.tar → ~/.config/somedaemon/config.yaml`. The user saves there; the app declares the path confined, creates the file through the link, writes a multi-gigabyte tar stream to it, and on cancel `remove_file` deletes the link target. The plan's B3 change routes copy-out onto exactly this write path while advertising "một hợp đồng duy nhất" safety.
- **Evidence:**
  - `/Users/longnd/Desktop/vnknowledge/colima-ui/src-tauri/src/validation.rs:198-213` — the `if candidate.exists() { canonicalize } else { parent.join(name) }` branch.
  - `/Users/longnd/Desktop/vnknowledge/colima-ui/src-tauri/src/commands/file_transfer.rs:112-124` — `resolve_destination` relies on that function plus `exists()`.
  - `/Users/longnd/Desktop/vnknowledge/colima-ui/src-tauri/src/streaming_cmd.rs:259` — `File::create` follows symlinks.
- **Suggested fix:** Use `symlink_metadata` for the existence check and reject when the candidate is a symlink of any kind; open the sink with `create_new(true)` on a temp name (see Finding 4), which closes both this and the truncate window.

## Finding 8: Phase 2 silently changes the authenticated HTTP API contract, and the affected route files are not in scope
- **Severity:** Medium
- **Location:** Phase 2, "Related Code Files"; Phase 6, "Related Code Files" (`docs/api.md`)
- **Flaw:** `require_archive_name` and `sniff_tar` are added to the shared `start_*` functions, which are the same entry points the loopback HTTP API calls. Existing API clients that pass a filename without `.tar`, or a `.tar.gz`, start receiving errors. Copy-out consumers start receiving a tar instead of the raw file. The plan lists neither `src-tauri/src/routes/file_transfer.rs` nor `src-tauri/src/routes/payloads.rs` as files to touch, and mentions the API only as a docs edit in Phase 6 — which the phase itself flags as the phase most likely to be dropped.
- **Evidence:**
  - `/Users/longnd/Desktop/vnknowledge/colima-ui/src-tauri/src/routes/file_transfer.rs:17-70` — `api_image_save`/`api_image_load`/`api_copy_from_container` call the same `file_transfer::start_*` functions with body-supplied `dest_dir`, `file_name`, `tar_path`.
  - `/Users/longnd/Desktop/vnknowledge/colima-ui/src-tauri/src/api_server.rs:131-134` — routes `/api/images/save`, `/api/images/load`, `/api/containers/cp/from`.
  - `/Users/longnd/Desktop/vnknowledge/colima-ui/src-tauri/src/api_server.rs:271` — `auth_middleware` layer (so this is a contract break for legitimate token-holding clients, not an exposure).
  - Phase 2, lines 117-128 — route files absent from the scope list.
- **Suggested fix:** Add the route/payload files to Phase 2's scope, move the `docs/api.md` update into Phase 2 (it documents a breaking change, not polish), and state the migration note for copy-out API consumers explicitly.

---

### Verification Results

- **Tier:** Full
- **Claims checked:** 41
- **VERIFIED:** 32
- **FAILED:** 6
- **UNVERIFIED:** 3

**FAILED claims**

1. **Phase 5, "Related Code Files":** `src/pages/settings/PrivacySettings.svelte` — FAILED (not found). `src/pages/settings/` contains only `Account.svelte`, `ColimaConfig.svelte`, `Subscription.svelte`. Phase 4 line 54 (`Modify: src/pages/settings/` for the OS-notification toggle) has the same problem.
2. **Phase 5, "Related Code Files":** `supabase/migrations/<ts>_announcements.sql` — FAILED (no `supabase/` directory exists at repo root). The plan's hedge ("nếu plan đó chưa chạy, tạo thư mục mới") means Phase 5 silently owns creating the whole migration workflow, which `plans/260811-1930-subscription-teams-supabase-schema/phase-03-supabase-schema-rls-migrations.md:53` treats as its own deliverable — an undeclared cross-plan dependency, not the "độc lập" claimed at `plan.md:81-85`.
3. **Phase 6, step 1:** "namespace `notifications.*` … đã có" — FAILED. `grep -o '"notifications[^"]*"' src/locales/en.json` returns nothing; only `"transfer"` exists.
4. **Phase 1, non-functional:** "mọi text đi qua `redact()` một lần, ở tầng store" — FAILED. Redaction lives in `src/lib/globalToast.ts:101`, not in any store, and the `error` payload at `:120` is unredacted. See Finding 3.
5. **Phase 2, B3 detail:** "Chỉ `container_path` và `container_id` mới là input cần validate" — FAILED as written; `container_id` is not subject to `reject_flag_like`. See Finding 5.
6. **Phase 5, Risk Assessment:** "lọc chính ở RLS dùng `now()` của server, client chỉ lọc thêm" — FAILED as a general statement. RLS gates `published_at`/`expires_at` only; `audience` and `min_version`/`max_version` are client-side only, which the same phase acknowledges at line 42. The stronger problem is that `published_at default now()` gives the table no draft state — inserting an embargoed advisory publishes it worldwide at insert time.

**UNVERIFIED**

- `src-tauri/tests/file_transfer_against_docker.rs` "thêm case cho B1-B3" — file exists; whether the harness can create a container with a directory to copy out was not exercised (requires Docker).
- Phase 5 "Đọc bằng anon key hoạt động khi **chưa** đăng nhập" — plausible, but `accountAvailability()` returns `"browser-mode"` and `getSupabase()` returns `null` outside Tauri (`src/lib/supabase.ts:55-58`), so announcements silently never load in browser mode. The plan does not say whether that is intended.
- Phase 3 "`Sidebar.svelte:62` đã có comment cảnh báo" — the comment exists but at lines 61-63, and it concerns a 15th *nav* item; it does not describe a reusable panel mechanism. `nav-panel-hint` is a CSS class at `src/components/Sidebar.svelte:133,139,332`, not the "panel-toggle mechanism đã có" the phase implies.

### Recommended Actions (priority order)

1. Add `link_url` scheme/host allowlist + explicit plain-text rendering rule + row/field size caps to Phase 5 before any migration is written (Findings 1, 2).
2. Move redaction into the store and strip `detail` from OS notifications; redact Rust-side `transfer.failed` before emit (Finding 3).
3. Replace the sink write with temp-file + atomic rename, `create_new(true)`, and `symlink_metadata` rejection (Findings 4, 7).
4. Add `reject_flag_like` on `container_id` and a `--` argv terminator (Finding 5).
5. Downgrade the `sniff_tar` acceptance criterion from a guarantee to a mistake-catcher (Finding 6).
6. Pull route files and `docs/api.md` into Phase 2 scope; declare the migration-workflow dependency on the subscription plan (Finding 8, FAILED claim 2).
7. Correct the six FAILED factual claims in the plan text before execution starts.

### Unresolved Questions

- Who holds the `service_role` key for the announcements project, and where? The whole Phase 5 threat model rests on that key, and nothing in the plan addresses its custody or rotation.
- Is copy-out reachable by any non-UI consumer today (scripts against the loopback API)? That determines whether the `.tar` contract change is a breaking change or an internal one.
- Does browser mode need announcements at all? If not, say so; if yes, `getSupabase()` returning `null` there must be addressed.
