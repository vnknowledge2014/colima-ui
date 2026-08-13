---
title: Activity history — nhật ký hành động cục bộ
description: >-
  Ghi lại những gì người dùng đã làm với máy của họ, trong một kho cục bộ. Không
  đồng bộ đi đâu cả.
status: completed — cả 4 phase xong 2026-08-13
priority: P2
branch: dev
tags:
  - activity
  - audit-log
  - local-first
  - privacy
blockedBy: []
blocks: []
created: '2026-08-12T18:46:46.378Z'
createdBy: 'ck:plan'
source: skill
---

# Activity history — nhật ký hành động cục bộ

## Overview

Ứng dụng đã ghi lại **kết quả** của bốn thứ (self-heal, alert, scan, compose
patch). Nó không ghi lại **hành động** — ai bấm prune, khi nào, xoá mất gì.

Đó là khoảng trống đáng vá: `prune_images` là hành động không hoàn tác được, chạy
xong không để lại dấu vết nào ngoài dung lượng đĩa đổi.

## Quyết định đã chốt với chủ dự án (2026-08-13)

| Câu hỏi | Chốt | Hệ quả |
|---|---|---|
| Lưu ở đâu | **Chỉ local** | Không đụng Supabase. Xem "Vì sao không sync" |
| Ghi cái gì | Cả 4 nhóm: phá huỷ, vòng đời, tác vụ dài, đổi cấu hình | Phase 3 |
| Kiến trúc | **Bảng mới, 4 kho cũ giữ nguyên** | Không sửa code vừa ship xong |
| Free/Pro | Free ngắn hạn, Pro dài hạn + lọc + xuất | Phase 2 (retention), phase 4 (UI) |

## Vì sao không sync lên Supabase

Không phải vì khó. Vì nó đảo ngược hai điều đã ghi thành văn:

| Điều đã ghi | Ở đâu | Sync sẽ phá vỡ thế nào |
|---|---|---|
| "No Supabase tables will be created" | `plans/260811-1930-.../phase-03-...md` — phase **cancelled** | Phải mở lại phase đã đóng, thêm RLS + migrations |
| Tên image / container / đường dẫn **không bao giờ rời máy** | `docs/telemetry.md` | Activity log **chính là** tập dữ liệu đó |
| "there is no RLS surface and no client write path" | `phase-07-verify-harden.md:32-37` — tiền đề của cả pass red-team | Mở đường ghi từ client là dựng lại đúng bề mặt tấn công đó |

Đã kiểm chứng 2026-08-13: `supabase/migrations` không tồn tại; `.from(` / `.rpc(`
xuất hiện **0 lần** trong `src/`.

Nếu sau này thật sự cần sync, nó là **một plan riêng** bắt đầu bằng việc viết lại
`docs/telemetry.md` — không phải một phase gắn vào đây.

## Hiện trạng — cái gì đã được ghi, cái gì chưa

| Đã ghi | Kho | Ghi cái gì |
|---|---|---|
| Self-heal | `heal_log` (settings.db) | rule, container, action, mode, outcome |
| Alert | `alert_events` (settings.db) | rule, container/image, value, threshold |
| Scan posture | `scan_runs` (security.db) | điểm, breakdown, pack, severity counts |
| Compose patch | `compose-backups/*.json` | fix id, path, explanation, undone |

| **Chưa ghi gì cả** | Vì sao đáng ghi |
|---|---|
| `prune_images`, `prune_volumes`, `prune_networks`, `system_prune` | Không hoàn tác được, xoá hàng loạt |
| `remove_container/image/volume/network`, `delete_instance` | Không hoàn tác được |
| `start/stop/restart/pause` container, `start/stop` instance | Ghép với metrics để trả lời "đêm qua nó chết vì sao" |
| `pull_image`, image save/load, copy to/from container | Có thời lượng và kết quả thành/bại |
| `apply_colima_config`, sửa alert/policy/heal rules | Trả lời "vì sao hôm nay khác hôm qua" |

## Phát hiện định hình plan: không có chokepoint

`docker_output` được **nhân bản 4 lần** — `containers.rs:51`, `volumes.rs:37`,
`networks.rs`, `compose.rs` — thân hàm gần như giống hệt. Colima và Lima đi
đường khác hẳn (`adapters/colima.rs:17 colima_cmd()`, `adapters/lima.rs:13`).

Nghĩa là hôm nay **không tồn tại một chỗ nào** để cắm việc ghi nhận. Đó là lý do
phase 1 tồn tại: gộp bốn bản sao thành một, không đổi hành vi, rồi mọi thứ sau đó
mới có chỗ bám. Gộp bốn bản sao là việc nên làm dù có plan này hay không.

## Phases

| Phase | Name | Status |
|-------|------|--------|
| 1 | [Chokepoint thực thi runtime](./phase-01-chokepoint-th-c-thi-runtime.md) | Completed |
| 2 | [Kho activity + retention theo tier](./phase-02-kho-activity-retention-theo-tier.md) | **Xong 2026-08-13** — `commands/activity.rs` + route đọc; 11 unit test |
| 3 | [Cắm ghi nhận vào các hành động](./phase-03-c-m-ghi-nh-n-v-o-c-c-h-nh-ng.md) | **Xong 2026-08-13** — 29 lệnh + test chống quên |
| 4 | [Lớp đọc hợp nhất + UI](./phase-04-l-p-c-h-p-nh-t-ui.md) | **Xong 2026-08-13** — tab `actions`, 5 nguồn, xuất Pro |

1 → 2 → 3 tuyến tính. 4 chờ 2 (đọc được ngay cả khi phase 3 mới cắm một phần).

## Nguyên tắc chi phối

1. **Ghi nhận không bao giờ làm hỏng hành động.** Ghi log thất bại thì hành động
   vẫn thành công; lỗi ra `eprintln!`, không ra người dùng. Tiền lệ:
   `security_scan.rs` khi `record_audit` lỗi.
2. **Ghi cả thất bại.** Một `prune` bị từ chối cũng là chuyện đáng biết. `outcome`
   là cột bắt buộc, không phải chỉ ghi khi thành công.
3. **Không ghi giá trị bí mật.** Argv có thể chứa `-e PASSWORD=...`. Đi qua
   `redact` (`src-tauri/src/redact.rs`) trước khi lưu — kho cục bộ vẫn nằm trong
   bundle chẩn đoán mà người dùng gửi đi.
4. **Free vẫn thấy được thứ quan trọng nhất.** Retention ngắn ≠ không có audit;
   hành động phá huỷ là an toàn cơ bản.
5. **Kho thứ tư, vòng đời thứ tư.** `activity.db` riêng — cùng lý lẽ đã dùng cho
   `security.db`: prune được, nhưng nhịp và tuổi thọ khác metrics.

## Acceptance chung

Mỗi mục ghi kèm bằng chứng, để người đọc sau không phải tin lời.

- [x] `prune_images` xong → có đúng một bản ghi, kèm outcome và số lượng xoá.
      — `a_successful_prune_reports_what_it_removed` (con số),
      `a_computed_detail_never_erases_the_reason_a_command_failed` (lý do khi
      hỏng). "Đúng một" là do cấu trúc: một lời gọi `record` duy nhất trong hàm,
      và `activity_coverage` canh nó tồn tại.
- [x] Ghi log hỏng (đĩa đầy, DB khoá) → hành động vẫn chạy, người dùng không thấy lỗi.
      — `a_store_that_cannot_be_written_to_fails_quietly`. Nửa "người gọi không
      bị ảnh hưởng" là bảo đảm **ở mức kiểu**: `record` trả `()`, không có kênh
      nào để lỗi đi ngược lên. Test lo nửa còn lại: lỗi đến dưới dạng `Err` để
      nuốt, không phải panic.
- [x] Không có giá trị biến môi trường nào trong `activity.db` sau khi
      `run_container` với `-e SECRET=...`.
      — `a_secret_in_the_detail_never_reaches_the_disk`; `redact` chạy **trong**
      `record`, không tin ba mươi chỗ gọi tự nhớ.
- [x] Free: quá hạn giữ → bản ghi cũ biến mất, bản ghi mới vẫn ghi.
      — `free_keeps_only_its_five_hundred_newest`,
      `free_drops_anything_older_than_a_week`.
- [x] Pro hết hạn → không mất dữ liệu đã có, chỉ mất tầm nhìn xa và xuất.
      — `losing_pro_does_not_delete_the_history_it_paid_for`,
      `pro_keeps_a_month_old_row_that_free_would_have_cut`.
- [x] `cargo clippy -D warnings` + `pnpm lint/typecheck/check` sạch.
      — 2026-08-13: clippy 0, `cargo test --lib` 484 passed, lint/typecheck/check
      0 lỗi.

### Chưa làm, cố ý — đọc trước khi mở lại plan này

1. **`limit` là theo từng nguồn rồi mới trộn.** Có bộ lọc thì đọc trần 500 mỗi
   kho nên không mất dòng; không lọc thì một nguồn ồn ào vẫn có thể làm cửa sổ
   thời gian co lại so với nguồn im. Sửa đúng là phân trang theo timestamp.
2. **`partial` mang nguyên văn lỗi DB ra client API.**
3. **Lọc theo đối tượng / khoảng thời gian có ở API nhưng chưa có UI** — mới chỉ
   lọc theo nhóm.
4. **Ngưỡng giữ của Free (7 ngày / 500)** là số chọn trước khi có dữ liệu thật.
   Đo rồi chỉnh.

## Dependencies

Không chặn bởi plan nào. Không chặn plan nào.

Chạm vào code mà `260811-2245-pro-tier-features` phase 3 (self-heal) và
`260812-1013-security-posture` phase 5 vừa ship, nhưng **chỉ đọc** — phase 4 đọc
`heal_log`/`alert_events`/`scan_runs` mà không sửa schema của chúng. Đó là lý do
chọn "bảng mới, kho cũ giữ nguyên".

## Unresolved

Không còn. Cả hai mục đã đóng trong Validation Session 1 — xem `## Validation Log`.

## Validation Log

### Session 1 — 2026-08-13

**Verification Results**
- Claims checked: 11
- Verified: 9 | Failed: 2 | Unverified: 0
- Tier: Standard (4 phase → Fact Checker + Contract Verifier)

| # | Claim | Kết quả |
|---|---|---|
| 1 | `docker_output` nhân bản 4 lần | VERIFIED — `containers.rs:51`, `volumes.rs:37`, `networks.rs:25`, `compose.rs:16` |
| 2 | Call site 25/6/6/7 | VERIFIED |
| 3 | colima/lima đi đường riêng | VERIFIED — `adapters/colima.rs:17`, `adapters/lima.rs:13` |
| 4 | `redact::redact_text` | **FAILED** — tên thật là `redact::redact` (`redact.rs:262`). Đã sửa phase 2 |
| 5 | `metrics_store::entitled_now_cached()` | VERIFIED — `metrics_store.rs:352` |
| 6 | "Timeout không đồng nhất giữa 4 bản sao" | **FAILED** — cả bốn đều `from_secs(10)`. Đã viết lại phase 1 |
| 7 | 30 lệnh mutating trong bảng phase 3 tồn tại | VERIFIED — không lệnh nào thiếu |
| 8 | `heal_log` / `alert_events` / `scan_runs` / compose-backups | VERIFIED |
| 9 | `Activity.svelte` + mục sidebar đã tồn tại | VERIFIED — phase 4 là tab, không phải trang mới |
| 10 | 437 test đang xanh | VERIFIED |
| 11 | Không có Supabase data table / data call | VERIFIED — `migrations` không tồn tại, `.from(`/`.rpc(` = 0 |

**Phát hiện kèm theo (không phải claim của plan):** `compose_up`
(`compose.rs:81`) chạy qua timeout **10 giây**, trong khi `docker compose up` có
thể pull image và mất vài phút. Bug có sẵn.

**Quyết định đã chốt**

| # | Câu hỏi | Chốt | Áp vào |
|---|---|---|---|
| 1 | Bug timeout `compose up` | **Giữ nguyên, mở việc riêng.** Phase 1 là refactor thuần; trộn bug fix vào diện tích 4 tệp làm cả hai khó review và khó rollback | phase 1 |
| 2 | Hạn giữ của Free | **7 ngày / 500 bản ghi.** Ship rồi đo, không tranh luận con số trước khi có dữ liệu | phase 2 |
| 3 | Ghi hành động do app khởi xướng | **Có, `actor = app`.** Người dùng cần phân biệt "tôi restart" với "nó tự restart"; phase 4 khử trùng khi hiển thị | phase 2, 3 |
| 4 | Cách chống quên ở phase 3 | **Giữ test quét mã nguồn.** Thô, và vỡ khi đổi tên hàm — nhưng nó vỡ **ồn ào** lúc CI, không im lặng như việc quên cắm | phase 3 |

**Sửa do verification**
- phase 1: viết lại toàn bộ mục timeout; sửa success criteria và bảng rủi ro; điền line number thật.
- phase 2: `redact_text` → `redact` (kèm `redact.rs:262`); chốt hạn Free; chốt `actor`.
- phase 3: chốt `actor: app`; chốt cách chống quên; thêm rủi ro "test quét mã nguồn vỡ".
- phase 4: đã chốt từ trước — `Activity.svelte` tồn tại nên đây là tab.

### Whole-Plan Consistency Sweep

Đã đọc lại `plan.md` + cả 4 phase sau khi propagate.

| Kiểm | Kết quả |
|---|---|
| `redact_text` còn sót trong thân plan | Sạch — 0 trong 4 tệp phase. 3 lần còn lại nằm trong chính Validation Log này, mô tả claim đã sửa |
| Claim "timeout không đồng nhất" còn sót | Sạch — chỉ còn ở phase 1 dưới dạng ghi nhận đã bị bác, và ở log này |
| Unresolved #1/#2 còn được nhắc như "chưa chốt" | Đã sửa: phase 2 bảng cột `actor` không còn trỏ tới "Unresolved #2" |
| Số 7 ngày/500 nhất quán giữa plan.md và phase 2 | Nhất quán |
| Phase 4 nói "trang mới" ở đâu không | Sạch — đã chốt là tab |

**Mâu thuẫn chưa giải quyết: không có.**
