---
phase: 7
title: i18n tests docs
status: completed
priority: P2
dependencies:
  - 1
  - 2
  - 3
  - 4
  - 5
  - 6
effort: S
---

# Phase 7: i18n tests docs

## Overview

Đóng đuôi: khoá dịch 4 locale, rà test hồi quy, cập nhật tài liệu ở chỗ hành vi
người dùng thật sự đổi.

**Không được cắt.** Phase 1 đổi hợp đồng công khai (`copy-out` giờ trả `.tar`) và
Phase 2 thêm endpoint mới — đây là tài liệu bắt buộc, không phải đánh bóng. Bản plan
đầu để việc bump `docs/api.md` ở phase cuối cùng có thể cắt; đã chuyển phần
`docs/api.md` cho `.tar` vào **Phase 1** và endpoint mới vào **Phase 2**, ngay tại
chỗ thay đổi. Phase này chỉ còn phần i18n, test tổng, và docs kiến trúc.

## Requirements

- Mọi chuỗi mới có ở `en/vi/ja/zh`.
- Test phủ **thay đổi hành vi**, không phủ dòng code.

## i18n (finding Medium)

Namespace `notifications.*` **chưa tồn tại** trong `en.json` — bản plan đầu ghi là
"đã có", sai. Tạo mới.

`t()` nhận `{ default: ... }` (đã kiểm chứng đúng), nên khoá thiếu **không làm vỡ
giao diện** — chính vì thế phải rà bằng script chứ không bằng mắt. Script đó cũng
chưa tồn tại; viết một script nhỏ so khoá giữa 4 file, đặt cùng chỗ với
`scripts/check-css-tokens.mjs` đã có, và nối vào `package.json` như `check:i18n`.

## Related Code Files

- Create: `scripts/check-i18n-keys.mjs`
- Modify: `package.json` — `check:i18n`
- Modify: `src/locales/en.json`, `vi.json`, `ja.json`, `zh.json`
- Modify: `docs/frontend.md` — Notification Center, transfer chạy nền
- Modify: `docs/architecture.md` — luồng event + đối soát qua job registry

## Implementation Steps

1. Viết `check-i18n-keys.mjs`: liệt kê khoá thiếu/thừa giữa 4 locale, exit code khác 0
   khi lệch.
2. Gom khoá mới từ Phase 1-6 dưới `notifications.*` và `transfer.*`.
3. Dịch 4 locale; giữ chuỗi ngắn, dùng thuật ngữ đã có trong file locale hiện tại.
4. Rà test tổng:
   - `src/store/notifications.test.ts` (Phase 3)
   - `src/components/notifications/NotificationPanel.test.ts` (Phase 5)
   - `src/components/transfer/TransferDialog.test.ts` — cập nhật cho hành vi mới
   - Rust unit: `require_archive_name`, `sniff_tar`, registry TTL, enum huỷ
   - `src-tauri/tests/file_transfer_against_docker.rs` — copy-out `.tar`, huỷ không
     phá file cũ
5. `pnpm test`, `pnpm check`, `pnpm lint`, `cargo test`, `pnpm lint:rust`.
6. Cập nhật `docs/frontend.md` + `docs/architecture.md`.

## Success Criteria

- [x] `pnpm check:i18n` xanh — không khoá nào lệch giữa 4 locale (392 khoá/file).
- [x] `pnpm test`, `pnpm check`, `pnpm lint` xanh.
- [x] `cargo test`, `pnpm lint:rust` xanh.
- [x] `docs/frontend.md` không còn mô tả progress nằm trong modal.
- [x] `docs/architecture.md` mô tả đúng luồng event + đối soát.

## Kết quả

`scripts/check-i18n-keys.mjs` + `pnpm check:i18n`. Nó kiểm **hai** thứ, và thứ
thứ hai mới là thứ bắt được nợ thật:

1. **Lệch giữa các locale** — file này có khoá file kia không có.
2. **Khoá code dùng mà `en.json` không định nghĩa.** Kiểm (1) không thấy được loại
   này: một khoá vắng mặt ở **cả bốn** file trông hoàn toàn nhất quán, trong khi mọi
   ngôn ngữ đều render `default` tiếng Anh. Đây chính là cách 10 khoá trôi mất khỏi
   mọi bản dịch mà không ai thấy gì sai khi test bằng tiếng Anh.

### 26 khoá đã thêm vào cả 4 locale

16 khoá của Phase 4-6 (`notifications.*`, `transfer.start_background`,
`transfer.copy_out_is_archive`) — và **10 khoá có sẵn từ trước**, không phải của
plan này: `sidebar.colima_instances` và 9 khoá `volumes.*`.

Bốn khoá `volumes.*` không chỉ thiếu mà **không thể dịch được**: chúng truyền
template literal JS làm `default`
(``t("volumes.remove_confirm", { default: `Remove volume "${name}"?` })``), nên
chuỗi đã được nội suy bằng tiếng Anh trước khi `t()` nhìn thấy. Đã chuyển sang tham
số `{name}` / `{count}` / `{names}` mà `t()` vốn hỗ trợ.

**Đây là mở rộng phạm vi có chủ đích.** Lý do: thêm một linter rồi để nó bỏ qua đúng
loại lỗi nó vừa phát hiện thì linter đó vô nghĩa. Chi phí là 4 call site cơ học
trong `Volumes.svelte`, không đổi hành vi.

### Docs

- `docs/frontend.md`: thêm mục "Notifications and background transfers" — dialog
  đóng lúc **bắt đầu**, event là đường nhanh còn `transferApi.list()` là sự thật, và
  entry không mang đường dẫn host (đó là thứ cho phép `formatEntryForClipboard` hứa
  nội dung an toàn để dán vào issue).
- `docs/architecture.md`: thêm mục "Background transfers" — vì sao registry tồn tại
  *cạnh* `streaming_cmd` chứ không nằm trong nó, quy tắc `.part` + rename, và giới
  hạn thật của `redact` cùng hệ quả với notification của OS.
- `docs/api.md` đã cập nhật ngay tại Phase 1 và 2, không dồn về đây — hợp đồng công
  khai đổi ở đâu thì tài liệu đổi ở đó.

## Risk Assessment

- **Bản dịch máy sai sắc thái** ja/zh — giữ chuỗi ngắn, tái dùng thuật ngữ có sẵn.
- **`check:i18n` bắt cả khoá cũ đang lệch** từ trước phase này. Nếu vậy: báo cáo, sửa
  riêng, đừng nới lỏng script để nó xanh.
