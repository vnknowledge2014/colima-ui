# ColimaUI — Implementation Checklist (Master)

The single place to see **what is done and what is still owed** across every plan
in [`plans/`](./plans/). The plan docs themselves are the source of truth — this
file only mirrors their `status` fields and points at them. Update this file
whenever a plan's status changes.

- Source of truth: `plans/<plan-id>/plan.md` frontmatter + phase tables
- Completed plans are archived in [`plans/archive/`](./plans/archive/)
- Decision history (red-team, blockedBy/blocks, cancelled phases): keep in place, never delete

**Lần đối chiếu gần nhất với code thật: 2026-08-13 08:17.** Mọi trạng thái plan
bên dưới đã được kiểm bằng sự tồn tại của module/symbol, không chỉ đọc `status`
tự khai — [báo cáo](plans/reports/from-sequential-thinking-to-owner-260813-0817-plans-progress-audit-report.md).
Lần đó không tìm thấy tuyên bố sai nào.

---

## Cổng chất lượng — đo 2026-08-13

| Cổng | Kết quả | Ghi chú |
|---|---|---|
| `cargo test --lib` | ✅ **452 pass, 0 fail**, 5 ignored | |
| `pnpm check` (svelte-check) | ✅ 407 file, 0 lỗi, 0 cảnh báo | |
| `pnpm check:tokens` | ✅ 86 defined / 78 referenced | |
| `pnpm build` | ✅ | |
| `pnpm check:i18n` | ❌ **đỏ** | `en.json` thiếu key `security.*`; `ja`/`vi`/`zh` mỗi file 605 key và đều pass |
| `pnpm vitest` | ❌ **không chạy được** | Chặn **toàn bộ** test frontend |

**Hai blocker toàn repo, không thuộc plan nào:**

1. **Test frontend không chạy được.** `undici@8.10.0` ném
   `webidl.util.markAsUncloneable is not a function` dưới Node 20.19.1, mà jsdom
   import undici. Mọi file `*.test.ts` của Svelte đều không chạy. Con số 452 test
   Rust che mất điều này: backend được kiểm rất kỹ, còn trang Security,
   notification centre và activity UI hiện **chỉ có svelte-check + build đứng
   sau** — kiểm kiểu và cú pháp, không kiểm hành vi.
2. **`en.json` thiếu key trong khi ba bản dịch có đủ.** Ngược đời, vì `en` là
   `FALLBACK_LOCALE`: người dùng tiếng Anh rơi vào `default:` viết inline trong
   lời gọi `t()`, còn vi/ja/zh dùng bản dịch thật.

### Lệnh kiểm chứng

Chạy hết trước khi commit một cụm lớn:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --lib   # 452 test
pnpm check && pnpm typecheck                            # svelte-check + tsc
pnpm check:tokens && pnpm check:i18n                    # cổng CSS + i18n
pnpm build
```

**5 test bị `#[ignore]`, không chạy trong lượt thường** — `cargo test --lib` báo
"5 ignored" mà không nói chúng là gì:

| Test | Vì sao ignore | Chạy bằng |
|---|---|---|
| 3 soak trong `metrics_store.rs` | mô phỏng một ngày ở nhịp production, ~15s | `cargo test --lib -- --ignored soak` |
| `runtime.rs` | cần container runtime đang chạy | `cargo test --lib -- --ignored` |
| `compose_autofix.rs` | cần docker | `cargo test --lib -- --ignored` |

**Script trong `scripts/`** — cổng của từng plan, không phải tiện ích lặt vặt:

| Script | Thuộc | Làm gì |
|---|---|---|
| `check-css-tokens.mjs` | toàn repo | Bắt `var(--token)` không ai định nghĩa — trình duyệt bỏ qua im lặng |
| `check-i18n-keys.mjs` | toàn repo | Bắt key `t()` thiếu và locale lệch nhau |
| `security-scanner-bakeoff.py` | plan 10 phase 1 bước 0 | Đo Trivy vs Grype trên image local — cơ sở cho quyết định chốt Trivy |
| `validate-kb-compose.sh` | plan 10 phase 8 | Trích YAML trong bài KB honeypot, kiểm bằng `docker compose config`, khẳng định mọi cổng bind `127.0.0.1` |
| `compose-autofix-gate.py` | plan 8 phase 1 | Cổng go/no-go tỉ lệ fix tất định |
| `compose-diagnose-benchmark.sh` | plan 8 phase 1 | Đo chẩn đoán trên corpus |
| `build-compose-corpus.py` | plan 8 phase 1 | Dựng corpus compose hỏng để đo |
| `setup-hooks.sh`, `release.sh`, `split_api.cjs` | hạ tầng | |

---

## Status summary

| # | Plan | Status | Còn lại | Location |
|---|------|--------|---------|----------|
| 1 | [P0 — Polish & Parity](plans/archive/260810-2258-colimaui-p0-polish-parity/plan.md) | ✅ completed | — | `plans/archive/` |
| 2 | [Security Hotfix](plans/archive/260810-2258-colimaui-security-hotfix/plan.md) | ✅ completed | — | `plans/archive/` |
| 3 | [Native Account (Supabase OAuth)](plans/archive/260811-1815-native-account-supabase-oauth/plan.md) | ✅ completed | — | `plans/archive/` |
| 7 | [Free Tier Foundation](plans/archive/260811-2241-free-tier-foundation/plan.md) | ✅ completed | — | `plans/archive/` |
| 9 | [Platform Prerequisites](plans/archive/260811-2300-platform-prerequisites/plan.md) | ✅ completed | — | `plans/archive/` |
| 4 | [Terminal Integration](plans/260811-0919-terminal-integration/plan.md) | 🔶 in-progress | Phase 6: auto+OS-level xong (Session 2); ~10' manual pass bằng mắt | `plans/` |
| 5 | [Subscription Teams + Supabase Schema](plans/260811-1930-subscription-teams-supabase-schema/plan.md) | 🔶 in-progress | Phase 7: live-purchase verify — chờ deploy edge functions | `plans/` |
| 6 | [Commercial Foundation](plans/260810-2258-colimaui-commercial-foundation/plan.md) | 🔶 in_progress | MoR/website, auto-update (Apple Dev ID), launch, wave 2 | `plans/` |
| 8 | [Pro Tier Features](plans/260811-2245-pro-tier-features/plan.md) | 🔶 in-progress | Phase 1 xong; phase 2 xong backend+UI còn soak; phase 3 code xong 2026-08-13, còn 5 kịch bản chạy tay | `plans/` |
| 10 | [Security Posture](plans/260812-1013-security-posture/plan.md) | 🔶 in-progress | Phase 1, 2, 4, 5, 8 xong + phase 3 (UI); còn phase 6, 7 và kênh catalog ký số chờ quyết định khoá | `plans/` |
| 11 | [Notification Center & Background Transfers](plans/archive/260812-1247-notification-center-background-transfers/plan.md) | ✅ completed | — | `plans/archive/` |
| 12 | [Cloud Broadcast Announcements](plans/archive/260812-1303-cloud-broadcast-announcements/plan.md) | ✅ completed | — | `plans/archive/` |
| 13 | [Security UI — posture-first redesign](plans/archive/260812-2318-security-ui-posture-redesign/plan.md) | ✅ completed | — | `plans/archive/` |
| 14 | [Activity history — nhật ký hành động cục bộ](plans/260813-0142-activity-history-local/plan.md) | 🔶 in-progress | Phase 1, 2 xong 2026-08-13; phase 3 (cắm ghi nhận vào ~30 lệnh) và 4 (UI) chưa làm | `plans/` |

---

## Chi tiết từng plan

### ✅ Đã xong (archived)

- [x] **[1. P0 — Polish & Parity](plans/archive/260810-2258-colimaui-p0-polish-parity/plan.md)** — 5/5 phase completed
- [x] **[2. Security Hotfix](plans/archive/260810-2258-colimaui-security-hotfix/plan.md)** — 3/3 phase completed
- [x] **[3. Native Account (Supabase OAuth)](plans/archive/260811-1815-native-account-supabase-oauth/plan.md)** — 5/5 phase completed; luồng sign-in → entitlement → badge chạy được
- [x] **[7. Free Tier Foundation](plans/archive/260811-2241-free-tier-foundation/plan.md)** — 7/7 phase Done (2026-08-12): file transfer + export image TAR, diagnostic bundle + report bug, activity monitor live, topology (ELK + network grouping), topology visual language, compose-declared services. Đã archive
- [x] **[9. Platform Prerequisites](plans/archive/260811-2300-platform-prerequisites/plan.md)** — 7/7 phase Completed (2026-08-12): token API hardening, redact mở rộng, crash persistence, streaming + path confinement, SSE subscriber registry, event source không debounce, entitlement gate theo `paid`. Đã archive
- [x] **[11. Notification Center & Background Transfers](plans/archive/260812-1247-notification-center-background-transfers/plan.md)** — 7/7 phase Completed (2026-08-12): hợp đồng `.tar`, job registry phục hồi job nền, notification store, progress rời modal chặn sang Notification Center, OS-native notification. Đã archive. Panel chuông được redesign sang bề mặt glass 2026-08-13 (`bbac1ff`) — cùng ngôn ngữ với toast; không đổi store, contract hay hành vi
- [x] **[12. Cloud Broadcast Announcements](plans/archive/260812-1303-cloud-broadcast-announcements/plan.md)** — 3/3 phase Completed (2026-08-12): feed JSON tĩnh fetch ở Rust (không bảng Supabase, không nới CSP), poll + lọc audience/version phía client với `critical` bỏ qua mọi bộ lọc, allowlist link + công tắc tắt hẳn request + `docs/telemetry.md`. **Feed chỉ sống khi `announcements.json` lên `main`.** Đã archive
- [x] **[13. Security UI — posture-first redesign](plans/archive/260812-2318-security-ui-posture-redesign/plan.md)** — 3/3 phase DONE (2026-08-13): khung IA + tab shell `Overview / Images / Runtime / History` + posture header, Overview (posture summary + next actions), Images tab + detail. Không đổi backend, không đổi contract `src/lib/api/security.ts` — posture tổng hợp client-side. Đã archive. **Chưa commit** (xem plan status)

### 🔶 Đang làm dở

- [ ] **[4. Terminal Integration](plans/260811-0919-terminal-integration/plan.md)**
  - PTY backend, transport, xterm rewrite, security/lifecycle, K8s exec: **completed**
  - ⏳ Phase 6 (Verification): **automated + OS-level đã verify (Session 2, 2026-08-12)** (cargo 157/157; PTY semantics, transport, kill-external, quit-no-orphans, security — xem [phase-06](plans/260811-0919-terminal-integration/phase-06-verification.md)); còn **~10 phút manual pass bằng mắt** (vim/htop in-app, tabs, k8s exec với cluster ổn định, history ↑)
- [ ] **[5. Subscription Teams + Supabase Schema](plans/260811-1930-subscription-teams-supabase-schema/plan.md)**
  - Tier design, Polar seats, entitlement oracle, client rewrite, Subscription UI: **completed** (phase 3 schema đã **cancelled** — Polar là roster)
  - ⏳ Phase 7 (Verify & Harden): **partial** — mọi gate offline pass; **live-purchase tests blocked** — cần `supabase functions deploy` + secrets (xem [docs/setup.md](./docs/setup.md))
- [ ] **[6. Commercial Foundation](plans/260810-2258-colimaui-commercial-foundation/plan.md)**
  - Pricing/waitlist, MoR billing (license client + Polar core xong), Pro boundary scaffolding, telemetry/crash core, compose auto-fix spike bước A: **in progress**
  - ⏳ Còn: verify live MoR (org_id + key), auto-update/signing (Apple Dev ID), **Phase 6 Launch, 7 Wave 2, 8 Native Account → đã chuyển sang plan 3**
- [ ] **[8. Pro Tier Features](plans/260811-2245-pro-tier-features/plan.md)**
  - Phase 1 (Compose auto-fix patch engine): **xong 2026-08-13** — 7/20 corpus, 0 file hỏng thêm
  - Phase 2 (Metrics history + Alerts): **backend + UI xong 2026-08-13** (`commands/metrics_store.rs`, `commands/alerts.rs`); soak **mô phỏng** đã có (`cargo test --lib -- --ignored soak`, 3 test ~15s: trần dung lượng, hình dạng raw/rollup, truy vấn 7 ngày)
  - ⏳ Phase 2 vẫn cần **soak đồng hồ thật 24h/20 container** — thời gian mô phỏng không đo được tỉ lệ drop, rò rỉ bộ nhớ, hay nhịp lấy mẫu trôi
  - Phase 3 (Self-healing rules): **code xong 2026-08-13** — cổng "P6 có reconnect" đã qua (`docker_state.rs:71-134`); `commands/self_heal.rs` + `routes/self_heal.rs` + `lib/api/self-heal.ts` + `SelfHealing.svelte` + `HealLog.svelte` + `HealSuggestionBanner.svelte` + tray + 4 locale; 7 unit test
  - ⏳ Phase 3 còn **5 kịch bản chỉ kiểm được trên máy thật**: crash-loop 5 lần/2 phút, quota sống qua restart app, entitlement hết hạn giữa chừng, `colima stop && start`, brake dừng việc đang chờ
- [ ] **[10. Security Posture](plans/260812-1013-security-posture/plan.md)**
  - Phase 1 (Trivy scan engine), 2 (rule pack + scoring), 8 (honeypot KB) **xong 2026-08-12**; phase 3 xong phần UI
  - Phase 4 (AI triage + hardening patch) và 5 (lịch sử điểm + policy gate + alert) **xong 2026-08-13** — `security_triage.rs`, `security_history.rs`, `security_policy.rs`, `security_watch.rs`; UI: `TriageList`, `SecurityPatchDiff`, `ScoreTrendChart`, `PolicyEditor`, `ScheduledRescanSwitch`
  - ⚠️ **Vênh UI cần xử lý:** phase 5 xong nhưng ba component của nó render trong **detail của tab Images** (`Security.svelte:509,513,520`), còn **tab History vẫn hiện "Score history is not available yet"** (`Security.svelte:328-341`). Tab đang nói sai với người dùng
  - ⏳ Nửa sau phase 3: kênh cập nhật catalog/rule pack có chữ ký — chờ quyết định về khoá ed25519 (Unresolved #1). Giấy phép nguồn rule đã rà: [verdict](plans/reports/from-security-phase2-step0-to-owner-260812-2109-rule-source-licensing-verdict-report.md)
  - ⏳ Phase 6 (detonation sandbox) — chỉ chờ phase 1, tức **đã hết gate**
  - ⏳ Phase 7 = tab **Runtime** (mặt tiền Falco). Chờ phase 5 (nay đã xong) nên đã hết gate; **chưa có dòng code nào** — không có `falco` trong `src/` lẫn `src-tauri/src/`, tab Runtime hiển thị "chưa khả dụng", đúng thực tế

- [ ] **[14. Activity history — nhật ký hành động cục bộ](plans/260813-0142-activity-history-local/plan.md)**
  - Phase 1 (chokepoint `runtime::run`): **xong** — 4 bản sao `docker_output` đã gộp còn 1
  - Phase 2 (kho + retention theo tier): **xong 2026-08-13** — `commands/activity.rs`, `activity.db`, `routes/activity.rs`; 11 unit test
  - ⏳ Phase 3: cắm `record()` vào ~30 lệnh mutating (prune, remove, start/stop, pull, apply config)
  - ⏳ Phase 4: tab đọc hợp nhất trên `Activity.svelte` — Free thấy ngắn hạn, Pro thấy dài + lọc + xuất

### ⏸ Chưa làm / 📝 draft (không tick cho tới khi có code)

_Trống — mọi plan đang mở đều đã có code chạy._

---

## Câu hỏi mở — gom từ mục Unresolved của các plan

CHECKLIST trước đây chỉ nêu một câu (khoá ed25519). Đây là **tất cả** những câu
chưa chốt, kèm thứ chúng chặn. Câu đã chốt nằm lại trong plan gốc với dấu gạch
ngang — không lặp ở đây.

### Chạm tới cam kết đã công bố (đáng lo nhất)

| # | Câu hỏi | Plan | Vì sao đáng lo |
|---|---|---|---|
| A | **Chưa kiểm offline thật.** Bakeoff mới chứng minh cờ `--skip-db-update` chạy được, chưa hề ngắt mạng | 10 (#5) | Acceptance chung của plan 10 hứa *"Mọi tính năng Free chạy hoàn toàn offline"*. Đang hứa một thứ chưa ai kiểm bằng airplane mode |
| B | **Trivy bật secret scanning mặc định** — nó đọc nội dung file bên trong image. Chưa quyết có tắt bằng `--scanners vuln` không | 10 (#6) | Chạm thẳng cam kết riêng tư. Cũng ảnh hưởng tốc độ scan |
| C | **Trivy đọc hỏng 2/23 image** (`not found in tar`) — bug của Trivy, của containerd, hay đặc thù image? | 10 (#7) | Nếu ~9% là tỉ lệ thật thì đường báo lỗi của phase 1 quan trọng hơn dự tính |

### Chặn việc đóng plan

| # | Câu hỏi | Plan | Chặn gì |
|---|---|---|---|
| D | **Ai ký catalog + rule pack, khoá riêng cất đâu?** Thủ công hay CI? | 10 (#1) | Nửa sau phase 3. Không có nó thì rule pack mãi là bản nhúng trong binary, không cập nhật được |
| E | **Ngưỡng điểm mặc định cho policy gate** | 10 (#4) | Cần dữ liệu thật từ phase 2 trước khi đặt số |
| F | **Ngưỡng waitlist bao nhiêu thì "đủ nhu cầu để đi tiếp"?** Cần con số cụ thể | 6 (#4) | Phase 1 không kết luận được, kéo theo Launch |

### Kinh doanh / pháp lý

| # | Câu hỏi | Plan |
|---|---|---|
| G | **Pháp nhân bán hàng đặt ở đâu?** Ảnh hưởng thuế và MoR | 6 (#2) |
| H | **Có cấp license miễn phí cho maintainer OSS / sinh viên không?** | 6 (#5) |

> Ba câu đã chốt gần đây, ghi lại để không hỏi lại: **scanner = Trivy**
> (2026-08-12, bakeoff 23 image); **Falco do người dùng tự cài** (2026-08-13, đo
> thật mất 9 giây trong VM nên bundle vô ích); **MoR = Polar**, **auth = Supabase**.

---

## Việc kế tiếp (theo thứ tự khuyến nghị)

### Chỉ chủ dự án mở được (mọi thứ khác đang chờ chúng)

1. **Secrets Supabase** — `supabase functions deploy` + secrets ([docs/setup.md](docs/setup.md) phase 3–5). Mở plan 5 phase 7 (verify mua thật), xong thì plan 5 archived
2. **`org_id` + key MoR test** — mở plan 6 phase 2 (verify billing live)
3. **Quyết định khoá ed25519** — ai giữ, ký ở đâu (Unresolved #1). Mở nửa sau plan 10 phase 3
4. **Apple Developer ID** — mở plan 6 phase 4 (auto-update + ký)
5. **~10 phút pass thị giác terminal** — plan 4 phase 6 ghi rõ "needs eyes"; xong thì plan 4 archived
6. **Soak 24h/20 container thật** — plan 8 phase 2. Soak mô phỏng đã phủ phần số học; đồng hồ thật vẫn cần cho tỉ lệ drop và rò rỉ bộ nhớ
7. **5 kịch bản self-healing trên máy thật** — plan 8 phase 3
8. **Dữ liệu waitlist** — plan 6 phase 6 (Launch) không chạy được cho tới khi có, vì bộ tính năng Pro v1 do waitlist quyết định
9. **Ai đang reset nhánh `dev`?** Commit `dc1a315` biến mất khỏi history là dấu
   hiệu quy trình git chồng chéo giữa các phiên làm việc. Cần biết trước khi 346
   file chưa commit gặp chuyện tương tự — đây là câu hỏi về quy trình, không ai
   trả lời thay được

### Làm được ngay, không chờ ai

10. **Thêm key `security.*` vào `en.json`** — cổng `check:i18n` đang đỏ, và đây là
   cổng duy nhất trong repo đỏ vì lý do nội tại. Nhỏ, sửa nhanh
11. **Gỡ test frontend** (undici/Node) — không phải để tăng con số, mà vì trang
    Security và notification centre hiện không có gì kiểm hành vi. Chẩn đoán
    undici/Node trước khi thêm dependency: có thể chỉ cần nâng undici hoặc đổi
    Node, ít xâm lấn hơn là thêm `happy-dom`
12. **Plan 14 (Activity history)** — `blockedBy: []`, plan đang mở duy nhất không vướng gì
13. **Vá vênh tab History** — phase 5 đã xong nhưng tab vẫn báo "chưa khả dụng"; hoặc chuyển `ScoreTrendChart`/`PolicyEditor` sang tab History, hoặc bỏ tab và giữ chúng ở Images
14. **Plan 10 phase 6** (detonation sandbox) — chỉ chờ phase 1, đã hết gate
15. **Kiểm offline thật bằng airplane mode** (câu A) — tắt wifi, scan một image đã
    có DB sẵn, xác nhận không có request nào rời máy. Vài phút, và nó quyết định
    được quyền nói "hoạt động offline" trên UI hay không
16. **Đo chi phí secret scanning của Trivy** (câu B) — chạy cùng image có và
    không có `--scanners vuln`, so thời gian. Có số rồi mới quyết được tắt hay
    giữ; hiện đang bật mặc định mà chưa ai biết giá

### Cần commit

**Trạng thái git đo 2026-08-13 08:17: 346 đường dẫn chưa commit**
— 147 modified, 169 untracked, 25 added, 3 AM, 2 deleted. Phân bố:
`src-tauri/src` 85, `src/lib` 68, `src/components` 34, `plans/reports` 32,
`src-tauri/resources` 26, `src/pages` 23.

Đó là **toàn bộ Lớp 1 + Lớp 2 của security, activity history, self-healing và
notification centre** nằm ngoài git.

17. **Push `announcements.json` lên `main`** — plan 12 xong nhưng feed còn 404; file có sẵn local, chưa track
18. **Commit Security UI + self-healing** — plan 13 ghi rõ "chưa commit"; plan chỉ khép khi code vào nhánh
19. **Khôi phục hoặc commit lại honeypot KB (plan 10 phase 8)** — commit `dc1a315`
    đã bị **reset khỏi history**; object còn dangling, file còn trong working tree
    nên nội dung không mất. `git cherry-pick dc1a315` nếu muốn giữ commit đó, hoặc
    bỏ qua nếu định gộp vào cụm commit lớn. Làm trước khi `git gc` dọn nó đi

> **Rủi ro mất việc không phải giả định — nó đã xảy ra một lần.** Với 346 file
> đang nằm ngoài git, nên chia thành vài commit theo plan thay vì một commit
> khổng lồ, và nên làm trước các việc khác trong mục này.
>
> Đã vào nhánh hôm nay: `bbac1ff` — redesign notification centre sang bề mặt
> glass (2 file, xem plan 11).
