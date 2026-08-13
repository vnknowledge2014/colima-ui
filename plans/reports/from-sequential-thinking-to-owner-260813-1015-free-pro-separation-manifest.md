# Manifest tách Free / Pro — toàn bộ working tree

**Ngày:** 2026-08-13 · **Nhánh:** dev · **Trạng thái: CHƯA commit, CHƯA sửa file nào.**
Tài liệu này chỉ phân loại và mô tả việc phải làm. Không có thay đổi code kèm theo.

Phạm vi: 555 file chưa commit (đã bung thư mục untracked), theo 3 plan:
`260812-1013-security-posture`, `260811-2245-pro-tier-features`,
`260810-2258-colimaui-commercial-foundation`.

---

## Phát hiện quan trọng nhất — đọc trước khi quyết định

**Codebase này được xây theo mô hình gate lúc chạy, không phải tách lúc build.**
Bằng chứng trong chính các plan:

- `260810-2258.../phase-03-pro-boundary.md:38` — "License là hàng rào lịch sự, không phải DRM".
- `260811-2245.../plan.md` — "Bỏ tuyên bố *gate ở backend*. Không cưỡng chế được:
  `subscription_store` nhận `entitled: true` từ client (`subscription/mod.rs:126-143`)".
- Cơ chế thật: `ProGate.svelte` + `proState.paid` bọc UI Pro; lệnh Tauri của Pro
  vẫn đăng ký vô điều kiện trong `lib.rs`.

Hệ quả: **code Pro vốn được thiết kế để ship chung một binary với Free.** Yêu cầu
"chỉ commit tính năng free" không phải là bỏ bớt file — nó là một thay đổi kiến trúc
phát hành. Hai cách hiểu, chi phí rất khác nhau:

| | Phương án A — giữ nguyên thiết kế | Phương án B — tách nguồn thật |
|---|---|---|
| Làm gì | Commit tất cả; Pro bị khoá bằng `ProGate` lúc chạy | Gỡ hẳn code Pro khỏi cây Free |
| Chi phí | 0 | Sửa ~14 file shared, xem mục 4 |
| Rủi ro | Người khác đọc source vẫn thấy code Pro | Cây Free vỡ build tới khi sửa xong hết mục 4 |
| Khớp plan? | Có — đúng thứ 3 plan đã chốt | Không — đảo lại quyết định đã chốt |

Manifest dưới đây phục vụ **phương án B** (điều bạn yêu cầu). Nếu chọn A thì
không có gì để tách.

---

## 1. Pro — tính năng (phase 4, 5, 6, 7 của security-posture + Pro tier)

Gỡ được ngay, không file nào khác trong nhóm Free import chúng.

### Backend (Rust)
```
src-tauri/src/commands/security_triage.rs        P4 AI triage
src-tauri/src/commands/security_autofix.rs       P4 hardening patch
src-tauri/src/commands/security_history.rs       P5 lịch sử điểm
src-tauri/src/commands/security_policy.rs        P5 policy gate
src-tauri/src/commands/alerts.rs                 P5 alert
src-tauri/src/commands/metrics_store.rs          P5 store cho history
src-tauri/src/commands/detonation.rs             P6
src-tauri/src/routes/detonation.rs               P6
src-tauri/src/commands/falco_bridge.rs           P7
src-tauri/src/commands/falco_triage.rs           P7
src-tauri/src/commands/compose_autofix.rs        Pro tier P1
src-tauri/src/commands/compose_autofix_apply.rs  Pro tier P1
src-tauri/src/commands/compose_autofix_fixers.rs Pro tier P1
```

### Frontend
```
src/components/security/TriageList.svelte          P4
src/components/security/SecurityPatchDiff.svelte   P4
src/components/security/ScoreTrendChart.svelte     P5
src/components/security/score-trend.ts + .test.ts  P5
src/components/security/PolicyEditor.svelte        P5
src/components/security/ScheduledRescanSwitch.svelte P5
src/components/security/DetonationPanel.svelte     P6
src/components/security/DetonationTimeline.svelte  P6
src/components/security/EventStream.svelte         P7
src/lib/api/detonation.ts                          P6
src/lib/api/falco.ts                               P7
src/lib/metricsHistory.ts + .test.ts               P5
src/lib/alertNotifications.ts                      P5
```

### Nội dung KB
```
src-tauri/resources/kb/{en,ja,vi,zh}/image-hardening.md   P4
src-tauri/resources/kb/{en,ja,vi,zh}/install-falco.md     P7
```

## 2. Pro — hạ tầng thương mại (commercial-foundation)

> **QUYẾT ĐỊNH 2026-08-13: GIỮ LẠI trong bản Free — không gỡ nhóm này.**
> `ProGate`/`UpgradeDialog` ở lại để Activity, Compose, SelfHealing, AI chat vẫn
> chạy và vẫn còn đường nâng cấp. Bản Free chỉ lộ khung gate, không lộ tính năng
> Pro. Danh sách dưới đây giữ lại để tham chiếu, **không phải việc phải làm.**

```
src-tauri/src/pro/{mod,bridge,protocol}.rs
src-tauri/src/subscription/{mod,cache}.rs
src-tauri/src/telemetry/{mod,events,consent}.rs
src-tauri/src/account_session.rs
src/components/ProGate.svelte
src/components/UpgradeDialog.svelte
src/components/ConsentDialog.svelte
src/components/pro/{ProBadge,ProLockedPreview}.svelte
src/components/account/{SignInPanel,UserBadge}.svelte
src/components/settings/PrivacySettings.svelte
src/lib/pro.svelte.ts
src/lib/account.svelte.ts
src/lib/account-oauth.ts
src/lib/api/subscription.ts
src/lib/api/telemetry.ts
src/store/upgrade.svelte.ts
src/store/account-dialog.svelte.ts
src/pages/settings/{Subscription,Account}.svelte
supabase/functions/            (entitlement, polar)
```

## 3. Free — commit được (phase 1, 2, 3, 8)

```
src-tauri/src/commands/security_scan.rs      P1 scan engine
src-tauri/src/commands/security_score.rs     P2 scoring
src-tauri/src/commands/security_rules.rs     P2 rule pack
src-tauri/src/commands/security_catalog.rs   P3 catalog
src-tauri/src/commands/security_watch.rs     P3  ⚠ xem mục 4
src-tauri/resources/rules/v1.json            P2
src-tauri/resources/catalog/v1.json          P3
src-tauri/tests/security_scan_against_trivy.rs
src-tauri/tests/security_score_against_corpus.rs
src/components/security/PostureHeader.svelte       P3
src/components/security/NextActions.svelte         P3
src/components/security/ImageRanking.svelte        P3
src/components/security/FindingsTable.svelte       P3
src/components/security/RuleChecklist.svelte(+test) P3
src/components/security/AlternativesPanel.svelte   P3
src/components/security/ScoreBreakdownCard.svelte(+test) P3
src/components/security/security-posture.ts(+test) P3
src/components/security/security-actions.ts(+test)  P3
src-tauri/resources/kb/{en,ja,vi,zh}/honeypot-*.md  P8 (4 bài × 4 ngôn ngữ)
src-tauri/resources/kb/{en,ja,vi,zh}/install-trivy.md P1
scripts/validate-kb-compose.sh, scripts/security-scanner-bakeoff.py
```

Ngoài ra ~357 file còn lại của working tree (terminal PTY, notifications,
transfer, k8s/kind, topology, activity, diagnostics, docs, hooks CI…) **không
thuộc trục Free/Pro** — chúng là nền tảng chung, đi cùng bản Free.

## 4. Điểm coupling phải sửa tay — đây là phần đắt

Đây là lý do không thể chỉ `git add` các file Free. Mỗi dòng dưới là một file
Free/shared đang trỏ vào code Pro:

| File | Việc phải làm |
|---|---|
| `src/pages/Security.svelte` | Bỏ 8 import (dòng 54-61) + 4 khối markup (342, 353, 475-498, 508-540). Tab **Runtime** và **History** biến mất hoàn toàn. |
| `src-tauri/src/lib.rs` | Bỏ 48 chỗ: `pub mod subscription/telemetry`, `use compose_autofix*`, `spawn_orphan_sweep`, `metrics_store::start_if_entitled`, và ~35 entry trong `invoke_handler` (dòng 245-320). |
| `src-tauri/src/commands/mod.rs` | Bỏ 11 khai báo `pub mod`. |
| `src-tauri/src/routes/mod.rs` | Bỏ `pub mod detonation`. |
| `src-tauri/src/routes/security.rs` | 42 tham chiếu — file này phục vụ cả route Free lẫn Pro, phải tách hàm chứ không xoá file. |
| `src-tauri/src/api_server.rs` | 25 chỗ đăng ký route Pro. |
| `src-tauri/src/commands/security_watch.rs` | 13 chỗ — quét định kỳ (Free) gọi sang policy/alert (Pro). Phải cắt nhánh Pro, giữ scan. |
| `src-tauri/src/routes/compose.rs`, `commands/compose_diagnose.rs` | Route autofix (Pro) nằm chung với diagnose (Free). |
| `src/lib/api/security.ts`, `src/lib/api/compose.ts`, `src/lib/api/index.ts` | Bỏ export của client Pro. |
| `src/App.svelte` | Bỏ `loadProStatus`/`loadSession`/ConsentDialog/UpgradeDialog. |
| `src/pages/Activity.svelte`, `src/pages/settings/SelfHealing.svelte`, `src/components/compose/DiagnosePanel.svelte`, `src/components/AiChatPanel.svelte` | Đều bọc `ProGate` — phải bỏ gate hoặc bỏ tính năng. |
| `src/pages/Settings.svelte` | Bỏ mục Subscription, Account, Privacy. |
| `src/locales/{en,vi,ja,zh}.json` | ~50 key Pro; `scripts/check-i18n-keys.mjs` sẽ báo lỗi nếu bỏ key mà còn chỗ dùng, hoặc ngược lại. |
| `src-tauri/src/commands/kb_articles.rs` | 11 chỗ — index KB liệt kê cả bài Pro. |

## 5. Thứ tự thực hiện — phương án B (đã duyệt 2026-08-13)

> ### ⛔ Ràng buộc bắt buộc: CHỈ COMMIT, KHÔNG PUSH
>
> `origin` = `https://github.com/vnknowledge2014/colima-ui.git` — **repo public**.
> Mọi bước dưới đây dừng ở commit local. Không `git push`, không tạo PR, không
> `--set-upstream`, kể cả cho nhánh `pro/*`. Push chỉ khi bạn yêu cầu rõ trong
> lượt đó.
>
> **Đã kiểm chứng 2026-08-13:** code Pro **chưa từng lên remote**. `git log --all
> --remotes` trả về 0 commit cho `detonation.rs`, `falco_bridge.rs`,
> `security_triage.rs`, `subscription/mod.rs`, `pro/mod.rs`, `ProGate.svelte` —
> tất cả còn ở trạng thái untracked. Nghĩa là **không cần rewrite history**, và
> chỉ cần không push nhánh Pro là repo public không bao giờ thấy code Pro.
> Rủi ro duy nhất còn lại là một lệnh push nhầm.
>
> Nhánh `dev` đang có 1 commit local chưa push — kiểm tra commit đó trước khi
> làm bất cứ việc gì, để chắc nó không chứa code Pro.

1. Tạo nhánh lưu Pro trước khi gỡ: `git checkout -b pro/security-intelligence` rồi
   commit toàn bộ **ở local**, để code Pro không mất. Nhánh này **không bao giờ
   được push**. Cân nhắc đặt nó ngoài repo (bundle/worktree riêng) nếu muốn chắc
   chắn hơn — xem câu hỏi 5.
2. Quay lại `dev`, gỡ nhóm 1 (tính năng Pro) + sửa coupling backend (`lib.rs`,
   `commands/mod.rs`, `routes/mod.rs`, `api_server.rs`, `routes/security.rs`).
3. Sửa coupling frontend (`Security.svelte` trước, rồi `App.svelte`, `Settings.svelte`).
4. Quyết định nhóm 2: giữ `ProGate` như no-op hay gỡ hẳn. **Khuyến nghị giữ** —
   gỡ hẳn lan sang 6 trang ngoài phạm vi bảo mật, không đáng.
5. Dọn locale, chạy `scripts/check-i18n-keys.mjs`.
6. Quality gate bắt buộc: `pnpm lint`, `pnpm typecheck`, `pnpm check`, `pnpm lint:rust`.
7. Chỉ commit (local) sau khi cả 4 cổng xanh và bạn duyệt. **Dừng ở đây — không push.**

---

## Câu hỏi chưa có lời đáp

1. ~~Chọn A hay B?~~ **Đã chốt 2026-08-13: phương án B**, vì `origin` là repo public.
2. ~~Nhóm 2 có nằm trong phạm vi Pro không?~~ **Đã chốt: GIỮ LẠI.** Xem mục 2.
3. ~~`compose_autofix*` cắt ở đâu?~~ **Đã chốt: gỡ autofix, giữ `compose_diagnose`.**
   Free vẫn chẩn đoán compose và hiện vấn đề; đề xuất + áp patch tự động là Pro.
   Phải sửa `routes/compose.rs`, `commands/compose_diagnose.rs`, `DiagnosePanel.svelte`,
   `src/lib/api/compose.ts`.
4. Repo hai nhánh (Free public / Pro private) sẽ phải merge ngược lâu dài. Chưa có
   plan nào mô tả quy trình đó.
5. **Giữ nhánh `pro/*` trong cùng repo có đủ an toàn không?** Một nhánh local trong
   repo có remote public chỉ cách một lệnh `git push --all` là lộ. An toàn hơn:
   `git bundle` hoặc một repo private tách hẳn. Cần bạn chốt.
