---
title: Notification Center & background transfers
description: >-
  Sửa hợp đồng .tar cho export/import/copy-out, dựng job registry để job nền
  phục hồi được, rồi chuyển progress từ modal chặn sang Notification Center
status: completed
priority: P1
branch: dev
tags:
  - notification
  - transfer
  - tar
  - ux
blockedBy: []
blocks:
  - 260812-1303-cloud-broadcast-announcements
created: '2026-08-12T09:12:31.632Z'
createdBy: 'ck:plan'
source: skill
---

# Notification Center & background transfers

## Overview

Hai vấn đề, một chuỗi phụ thuộc.

**Vấn đề 1 — hợp đồng `.tar`.** `docker save` ghi tar không nén, nhưng không đường
nào trong app đảm bảo sản phẩm là tar hợp lệ: export nhận tên file không đuôi,
import không kiểm nội dung, và copy-out từ một thư mục trong container ghi ra một
**cây thư mục** chứ không phải file. Đây là lỗi đúng nghĩa, độc lập với UI, chỉ nằm
ở Rust — nên nó đi trước.

**Vấn đề 2 — modal chặn.** Backend transfer đã chạy nền (job id, `streaming_cmd`,
event `transfer.*` trên Tauri + SSE, huỷ được), nhưng `TransferDialog.svelte` giữ
modal mở suốt thời gian job chạy.

Điểm mà bản plan đầu **sai** và red team đã chỉ ra: gỡ modal đi không phải là việc
thuần frontend. Modal hiện là thứ **duy nhất neo vòng đời một job** — đóng nó lại mà
không có gì thay thế thì reload cửa sổ là mất dấu job (job id là `AtomicU64`
process-local, không có API nào liệt kê job đang chạy), và một cú rớt SSE để lại
entry "running" vĩnh viễn. Vì vậy Phase 2 dựng registry + đường đối soát trước khi
UI được phép đóng modal.

## Quyết định đã chốt

| Quyết định | Lý do |
|---|---|
| Copy-out **luôn** ra `.tar` qua `docker cp <id>:<path> -` | Chủ sở hữu tái khẳng định sau khi nghe phản biện. Một hợp đồng duy nhất, không nhánh file-hay-thư-mục, progress đo được. Chi phí đã biết và chấp nhận: viết lại integration test, bump `docs/api.md`, người copy 1 file phải tự `tar -xf`. |
| Tách tar-fix ra Phase 1, không phụ thuộc gì | Nó là Rust-only và là sửa lỗi P1; buộc nó chờ một Svelte store là sai thứ tự. |
| Job registry (Phase 2) đi trước UI | Không có nó thì "chạy nền" nghĩa là "mất dấu", không phải "chạy nền". |
| **Mở rộng** `errorLog.svelte.ts` thay vì thêm store mới | `errorReporter.ts:32-33` đã ghi mọi lỗi vào `errorLogState`. Thêm store thứ hai cùng hình dạng là trùng lặp trạng thái. |
| Cloud broadcast tách sang plan riêng | Không đóng góp vào acceptance criteria nào của plan này, và cần hạ tầng chưa tồn tại. → `plans/260812-1303-cloud-broadcast-announcements/` |
| Không thêm Pro gating | Notification là hạ tầng UX. |

## Phases

| Phase | Name | Status |
|-------|------|--------|
| 1 | [Test harness and tar contract](./phase-01-test-harness-and-tar-contract.md) | Completed |
| 2 | [Job registry and recovery](./phase-02-job-registry-and-recovery.md) | Completed |
| 3 | [Notification store core](./phase-03-notification-store-core.md) | Completed |
| 4 | [Transfer UI to background](./phase-04-transfer-ui-to-background.md) | Completed |
| 5 | [Notification Center UI](./phase-05-notification-center-ui.md) | Completed |
| 6 | [OS-native notifications](./phase-06-os-native-notifications.md) | Completed |
| 7 | [i18n tests docs](./phase-07-i18n-tests-docs.md) | Completed |

Phase 1 độc lập hoàn toàn. Phase 2 độc lập với 1. Phase 3-5 là chuỗi. Phase 6 cắt
được.

## Kiến trúc

```
Rust                                   Frontend
────                                   ────────
file_transfer.rs
  ├ require_archive_name  (P1)
  ├ sniff_tar             (P1)
  ├ `--` terminator       (P1)
  └ copy-out → sink .tar  (P1)

job_registry (P2)  ──────────────►  transfer_list command / GET /api/transfers
  ├ job sống lâu hơn webview
  ├ register TRƯỚC khi spawn
  └ cancel trả sự thật

emit "transfer.*" ──┬─ Tauri event ──►  transferEvents.ts (P4)
                    └─ SSE ──────────►    + onerror/reconnect
                                          + reconcile qua transfer_list
                                              │
                                              ▼
                                    errorLog.svelte.ts mở rộng (P3)
                                       ├──► ToastContainer (đã có)
                                       ├──► NotificationPanel (P5)
                                       └──► osNotify (P6)
```

## Sự thật đã kiểm chứng (nền cho mọi phase)

Bản plan đầu có 6 claim sai; đây là bản đã đối chiếu code:

| Claim | Sự thật |
|---|---|
| `pnpm test` | **Không tồn tại.** `package.json` chỉ có dev/build/check/typecheck/lint/preview/tauri. Vitest đã cài, chưa nối script. |
| Settings store | `src/lib/settingsStore.svelte.ts` (không phải `src/store/`). `PrivacySettings.svelte` không tồn tại. |
| `redact()` trên đường transfer | **Không có.** Không `redact` nào trong đường emit Rust; `transfer.failed` mang stderr thô của Docker. |
| `is_valid_container_id` | Cho phép `-` đứng đầu (`validation.rs`); `reject_flag_like` chỉ áp `container_path`. |
| `openExternal` | Không validate scheme/host (`external-links.ts:110-122`). |
| CSS progress bar | Chỉ ở `TransferDialog.svelte:373-386`, **không** trùng lặp. |
| Sidebar panel-toggle | Không tái dùng được: `Sidebar.svelte:306` hardcode `item.id === "ai-chat"`. |
| Số call site `globalToast` | 130 call site / 161 ref / 32 file. |
| SSE | `broadcast::channel(64)`, lossy có chủ đích; `POLL_INTERVAL` 200ms → ~5 event/s/job. |
| `ToastContainer` | Đang mount **hai lần** ở `App.svelte:203` và `:244` — lỗi có sẵn, ghi nhận, ngoài phạm vi. |

## Acceptance criteria

- [x] `pnpm test` chạy được và xanh; `cargo test` xanh.
- [x] Export không đuôi `.tar` không lọt qua; import file không phải TAR bị từ chối
      trước khi gọi runtime.
- [x] Copy-out từ một thư mục ra `.tar` mà `tar -tf` liệt kê được.
- [x] Huỷ không xoá và không truncate file đã tồn tại từ trước ở đích.
- [x] Reload cửa sổ giữa lúc job chạy → job hiện lại trong Notification Center, vẫn
      huỷ được.
- [x] Rớt SSE rồi nối lại → không còn entry kẹt "running".
- [x] Bấm Start → dialog đóng ngay; lỗi validate lúc start vẫn giữ dialog mở.
- [x] `docs/api.md` mô tả đúng hợp đồng `.tar` mới của cả ba endpoint.

## Dependencies

- `blocks: [260812-1303-cloud-broadcast-announcements]` — plan cloud cần store và
  panel từ Phase 3 và 5 làm chỗ hiển thị.
- Không bị plan nào block.

## Red Team Review

### Session — 2026-08-12
**Findings:** 35 thô → 15 sau khử trùng lặp (14 accepted, 1 escalated to owner)
**Severity breakdown:** 6 Critical, 9 High (+10 Medium gộp vào các phase)
**Reviewers:** Security Adversary, Failure Mode Analyst, Assumption Destroyer,
Scope & Complexity Critic. Evidence filter: 0 finding bị loại — tất cả có `file:line`.

| # | Finding | Sev | Disposition | Applied To |
|---|---------|-----|-------------|------------|
| 1 | `pnpm test` không tồn tại, mọi phase gate vào nó | Critical | Accept | Phase 1 |
| 2 | Always-tar làm vỡ integration test đang có | Critical | Accept | Phase 1 |
| 3 | Cửa sổ mất event: spawn trước khi `Ok()` trả về | Critical | Accept | Phase 2 |
| 4 | Không có đường phục hồi job mồ côi | Critical | Accept | Phase 2 |
| 5 | `link_url` → `openExternal` không validate | Critical | Accept | → plan cloud |
| 6 | Store mới trùng `errorLogState`; chống ngập sai | Critical | Accept | Phase 3 |
| 7 | Claim `redact()` sai trên đường transfer | High | Accept | Phase 2, 6 |
| 8 | Huỷ truncate/xoá file người dùng đã có | High | Accept | Phase 1 |
| 9 | `-` đứng đầu container id; cần `--` | High | Accept | Phase 1 |
| 10 | SSE lossy + không reconnect → entry bất tử | High | Accept | Phase 4 |
| 11 | Race huỷ; `cancel_stream` trả `false` bị vứt | High | Accept | Phase 2, 5 |
| 12 | Copy-out sink không kiểm dung lượng đĩa | High | Accept | Phase 1 |
| 13 | Lọc tier không khả thi như đặc tả | High | Accept | → plan cloud |
| 14 | Hạ tầng migration không tồn tại | High | Accept | → plan cloud |
| 15 | Không có panel-toggle tái dùng được | High | Accept | Phase 5 |
| — | Always-tar phá hợp đồng công khai; đề xuất dò file-vs-thư-mục | Critical | **Escalated → chủ sở hữu giữ quyết định always-tar** | Phase 1 (ghi rõ chi phí) |

Medium đã gộp: symlink `assert_path_within`, `routes/file_transfer.rs` +
`payloads.rs` ngoài scope, `MAX_ENTRIES` đuổi job đang chạy, watermark UUID vô
nghĩa, sai đường dẫn settings, claim CSS trùng lặp sai, đếm call site sai, TOCTOU
`sniff_tar`, `kind:"announcement"` sớm, xin quyền OS không thử lại.

### Whole-Plan Consistency Sweep
- Files reread: plan.md + 7 phase files (tái tạo toàn bộ, không vá cục bộ)
- Decision deltas checked: 3 (always-tar giữ nguyên; Phase 5 cloud tách ra; store
  mở rộng thay vì tạo mới)
- Reconciled stale references: 10 claim sai đã sửa trong bảng "Sự thật đã kiểm chứng"
- Unresolved contradictions: 0

## Open questions

- Job registry (Phase 2) có nên persist ra disk để sống qua **restart app**, hay chỉ
  sống trong tiến trình (phục hồi được sau reload webview, mất sau khi thoát app)?
  Kế hoạch hiện tại: chỉ trong tiến trình — `kill_all_streams` đã chạy ở
  `ExitRequested`, nên job không sống qua lần thoát nào.
