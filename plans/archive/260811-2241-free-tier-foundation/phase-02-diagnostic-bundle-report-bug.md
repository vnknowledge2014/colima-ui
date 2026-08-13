---
phase: 2
title: "Diagnostic bundle + Report bug"
status: done
priority: P1
dependencies: ["prereq:P1", "prereq:P2", "prereq:P3"]
effort: ""
---

# Phase 2: Diagnostic bundle + Report bug

## Overview

Gom bối cảnh lỗi thành một gói đã redact, cho người dùng **xem toàn bộ trước khi gửi**, rồi mở issue GitHub. Nó là nguồn dữ liệu lỗi thật nuôi Knowledge Bank mà plan Pro phase 1 dựa vào.

> **Sửa sau red-team 2026-08-11.** Phase này **không còn rẻ**. Hai thứ nó định tái dùng đều chưa tồn tại, và đã được tách thành hạng mục tiên quyết riêng.

## Đính chính tiền đề

| Bản đầu nói | Thực tế | Ai làm |
|---|---|---|
| "`crash.rs` chỉ ghi, cần thêm hàm đọc" | **Sai.** Panic hook chỉ `eprintln!` (`crash.rs:48-55`); không file nào được ghi. Persistence là feature chưa scope. | Tiên quyết P3 |
| "Bổ sung pattern redact **nếu** test lộ thiếu sót" | Không phải "nếu". `redact()` phủ secret trong URL/header, **không** phủ `KEY=VALUE` trong log container (`redact.rs:26`). `POSTGRES_PASSWORD=hunter2` lọt thẳng. Không có AKIA/JWT/PEM, không che `/Users/<tên>`. | Tiên quyết P2 |

Đây là phase mà một thiếu sót redact đi thẳng vào **issue GitHub công khai**, nên hai tiên quyết trên là điều kiện cứng, không phải khuyến nghị.

## Requirements

**Functional**
- Bundle gồm: phiên bản app/colima/docker, host specs, log container liên quan (giới hạn N dòng cuối), output `colima status`, crash gần nhất từ `crash.rs`, danh sách container đang chạy (tên + image, không env).
- Toàn bộ đi qua `redact()` trước khi hiện ra.
- Người dùng xem được nội dung đầy đủ trong app trước khi gửi; gửi là hành động thủ công.
- Nút "Mở issue GitHub" prefill tiêu đề + body; đính kèm bundle là thao tác riêng, người dùng tự làm.
- Signature hoá lỗi để nhóm các báo cáo trùng.

**Non-functional**
- Không tự động gửi bất cứ thứ gì. Không phụ thuộc consent telemetry — đây là hành động do người dùng khởi xướng.
- Bundle giới hạn dung lượng (đề xuất 5 MB) — cắt log từ cuối lên.

## Architecture

```
UI: nút "Report a problem" (Help.svelte + banner lỗi + DiagnosePanel)
  └─ POST /api/diagnostics/bundle
      └─ commands/diagnostics.rs
          ├─ thu thập: system.rs (version/specs), containers.rs (logs), crash.rs
          ├─ redact.rs::redact() trên MỌI trường trước khi trả về
          └─ trả về DiagnosticBundle { sections: Vec<Section>, signature }
  └─ UI hiện từng section, cho phép bỏ chọn section
  └─ "Lưu .zip" (ghi ra đĩa) | "Mở GitHub issue" (external-links.ts)
```

**Quyết định thiết kế:** backend trả về **dữ liệu có cấu trúc**, không trả file zip. UI hiện từng section và cho bỏ chọn; chỉ khi người dùng bấm Lưu mới đóng gói. Như vậy "xem trước khi gửi" là thật, không phải một ô checkbox tin tưởng.

Cần thêm dependency zip cho Rust (`zip` crate) — hoặc tránh hẳn bằng cách ghi ra một file `.md` phẳng. **Đề xuất: ghi `.md` phẳng**, không thêm dependency, dán thẳng vào issue được, và người đọc issue không phải giải nén.

## Related Code Files

- Create: `src-tauri/src/commands/diagnostics.rs`
- Create: `src-tauri/src/routes/diagnostics.rs`
- Create: `src/components/diagnostics/BundlePreview.svelte`
- Create: `src/lib/api/diagnostics.ts`
- Modify: `src-tauri/src/redact.rs` (bổ sung pattern nếu test lộ thiếu sót)
- Modify: `src-tauri/src/crash.rs` (thêm hàm đọc crash report gần nhất, hiện chỉ có ghi)
- Modify: `src/pages/Help.svelte` (điểm vào chính)
- Modify: `src/lib/external-links.ts` (mở URL GitHub new-issue)
- Modify: `src/locales/{en,vi,ja,zh}.json`

## Implementation Steps

1. Xác nhận tiên quyết P2 (redact mở rộng) và P3 (crash persistence) đã xong; `crash::latest_report()` đã tồn tại và trả dữ liệu thật.
2. `diagnostics.rs`: `struct Section { id, title, content, included_by_default }`, `struct DiagnosticBundle { sections, signature, app_version }`.
3. Collector cho từng section, mỗi cái tự chịu lỗi: một section fail thì ghi `"(không thu thập được: <lý do>)"`, không làm hỏng cả bundle.
4. Chạy `redact()` trên content của **mọi** section, không ngoại lệ, ngay tại điểm tạo Section — không để chỗ nào bypass.
5. `error_signature()` tái dùng từ `compose_diagnose.rs` (đã có, dòng 83) để sinh signature — cùng thuật toán nghĩa là bug report và KB nói cùng một ngôn ngữ.
6. Route `POST /api/diagnostics/bundle`.
7. `BundlePreview.svelte`: liệt kê section, checkbox bỏ chọn, xem full content, nút "Lưu .md" + "Mở GitHub issue".
8. Prefill GitHub issue: tiêu đề từ signature, body có template + nhắc người dùng đính kèm file vừa lưu.
9. Test redact: bộ fixture chứa AWS key, token bearer, password trong env, đường dẫn `/Users/<tên>` → khẳng định không lọt.

## Success Criteria

- [x] Bundle sinh ra từ một máy có secret thật: không chuỗi nào trong bộ fixture redact lọt ra.
- [x] Cụ thể: log của container postgres có `POSTGRES_PASSWORD` → mật khẩu bị che trong bundle.
- [x] `/Users/<tên>` không xuất hiện nguyên dạng trong bundle.
- [x] Section crash hiển thị crash thật từ `crash::latest_report()`, không phải chỗ trống.
- [x] Một section fail (ví dụ colima chưa cài) không làm hỏng bundle.
- [x] Bundle >5 MB bị cắt log từ cuối, có ghi chú rõ đã cắt bao nhiêu.
- [x] Mở được GitHub issue prefill từ trong app. — URL đã có test (`src/lib/external-links.test.ts`); **chưa click thật** trên GUI
- [x] Signature của cùng một lỗi trên 2 máy khác nhau là giống nhau.

## Risk Assessment

| Rủi ro | Giảm thiểu |
|---|---|
| **Rò rỉ secret qua log container** — rủi ro lớn nhất của phase | Redact tại điểm tạo Section, không phải trước khi hiển thị; test fixture bắt buộc; log env vars không bao giờ được đưa vào |
| Người dùng tưởng app tự gửi | Không có đường gửi tự động nào tồn tại trong code; nút ghi rõ "Lưu" và "Mở GitHub" |
| Bundle quá lớn không dán được vào issue | Giới hạn 5 MB + cắt log; hướng dẫn đính kèm file |
| Redact quá tay làm mất thông tin cần thiết | Hiện nội dung đã redact cho người dùng xem; họ tự quyết định có gửi không |

## Kết quả thực thi — 2026-08-12

Ba tiên quyết P1/P2/P3 đều Completed. Redact (P2) phủ tốt: home path, PEM, key shapes, `KEY=VALUE`, JSON/YAML, URL credential, bearer, query param.

### Bằng chứng

- `cargo test --lib`: **285/285 pass** (15 test mới ở `diagnostics`).
- **5/5 test trên máy thật + log container thật** (`src-tauri/tests/diagnostics_against_real_machine.rs`, gate `#[ignore]`):
  - bundle thật **không chứa tên tài khoản** ở bất kỳ section nào;
  - container thật in `POSTGRES_PASSWORD=hunter2` và `AWS_ACCESS_KEY_ID=AKIA…` → **cả hai bị che**, mà dòng `ready` vẫn còn (không redact quá tay);
  - container id không hợp lệ → báo trong section, **không** spawn lệnh nào;
  - đủ 6 section kể cả khi collector lỗi;
  - section crash phản ánh đúng thứ đang lưu.
- `svelte-check` về baseline 143/36, **0 lỗi từ file của phase**; `pnpm build` pass.

### Hai vấn đề phát hiện khi verify

1. **Redact payload nhiều MB rất chậm** — test 7 MB chạy **92 giây** (debug ~8s/MB). Bundle log lớn sẽ treo UI. Sửa gốc: chặn kích thước **trước** khi redact (`MAX_LOG_LINES = 5000`, `MAX_LOG_BYTES = 2 MB`), và chạy `Section::new` của log trên `spawn_blocking`. Cắt sau khi redact là trả tiền cho phần rồi vứt đi. Test module giờ chạy **0.14s**.
2. **Test "không construct Section trực tiếp" là giả**: nó đếm chuỗi `Section {` nên khớp cả `pub struct`, `impl` và doc — 11 thay vì 2. Xoá, thay bằng thứ có giá trị thật: `render_markdown` **redact lần nữa**, kèm test dựng `Section` thủ công chưa redact và khẳng định output vẫn sạch. Bundle đi vòng qua client rồi mới ghi đĩa, nên "mọi thứ app ghi ra đều đã redact" giờ đúng ở đường code, không phải đúng nhờ giả định về caller. Có test khẳng định `redact` idempotent.

### Quyết định thiết kế

- **Không thêm crate zip**: xuất `.md` phẳng — dán thẳng vào issue, người đọc không phải giải nén.
- **Section log mặc định KHÔNG tick**: to nhất và dễ chứa thứ riêng tư nhất, kể cả sau redact.
- **Không bao giờ thu env container** (`docker inspect`), chỉ tên + image + status.
- **GitHub issue prefill không nhét bundle vào URL**: GitHub từ chối query string quá ~8 KB, bundle thường lớn hơn. Body chỉ có template + chữ ký; người dùng dán phần đã copy.
- **Preview hiện full nội dung**, không cắt bớt — preview mà lược bớt thì không phải thứ để ra quyết định.
- `destDir` + `fileName` tách rời khi lưu, cùng quy tắc confinement như phase 1.

### Chưa verify

- Chưa click GUI thật: nút Report ở Help, tick/bỏ tick section, Copy, Save, Open GitHub — mới verify ở mức type + build + test backend.

**Bổ sung 2026-08-12 (a) — verify tương tác:** 8 test ở `src/components/diagnostics/BundlePreview.test.ts` mount dialog thật và bấm vào nó. Quan trọng nhất là hành vi liên quan quyền riêng tư: **bỏ tick một section thì nội dung đó thật sự biến khỏi thứ được copy và được lưu** — một checkbox trông như loại trừ mà không loại trừ thì tệ hơn là không có checkbox, và không test backend nào thấy được vì lựa chọn nằm ở frontend. Cũng phủ: log section mặc định không tick; nội dung chỉ hiện khi bấm Show (và hiện **đầy đủ**); Save chỉ gửi section đã tick; issue URL không nhét bundle vào; collector lỗi thì hiện lỗi chứ không phải dialog rỗng.

**Bổ sung 2026-08-12 — verify tương tác ở tầng component.** Không click được cửa sổ thật trong môi trường này (lý do ở báo cáo `plans/reports/from-sequential-thinking-to-owner-260812-1233-*`), nhưng phần *logic* của việc click thì mount và bấm được — cùng kỹ thuật đã bắt lỗi `effect_update_depth_exceeded` ở `GraphCanvas`. Còn lại chưa phủ: bố cục thị giác, hành vi ở tầng OS (file picker native, mở browser), và cảm giác khi dùng.

**Bổ sung 2026-08-12 (b):** URL prefill giờ có 5 test (`src/lib/external-links.test.ts`): đúng repo, prefill cả `title` và `body`, escape `& # %` trong chữ ký (không escape thì mọi thứ sau `&` thành tham số riêng và bị mất), và tổng URL dưới 8 KB — ngưỡng GitHub từ chối, tức lý do bundle được copy chứ không nhét vào URL.
- Tiêu chí ">5 MB bị cắt" verify qua `trim_to_tail` và `enforce_size_limit` ở mức đơn vị, không dựng bundle 5 MB thật (redact sẽ tốn hàng chục giây cho một thứ collector đã chặn từ đầu).
