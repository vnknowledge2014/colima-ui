---
phase: 6
title: OS-native notifications
status: completed
priority: P3
dependencies:
  - 3
  - 5
effort: S
---

# Phase 6: OS-native notifications

## Overview

Job dài chạy nền thì người dùng chuyển cửa sổ khác — badge trong app không ai thấy.
Bắn notification hệ điều hành khi job kết thúc, chỉ khi cửa sổ không focus.

**Cắt được.** Phase 1-5 đã giải quyết trọn hai vấn đề gốc. Phase này là tiện ích.

## Requirements

**Functional**
- Chỉ desktop (Tauri); browser mode no-op, không lỗi.
- Chỉ bắn khi `document.hasFocus() === false`.
- Chỉ cho sự kiện kết thúc (`done`/`failed`), không cho progress.
- Công tắc trong Settings, mặc định bật.
- Xin quyền lần đầu **có job kết thúc**, không xin lúc khởi động; user từ chối thì
  không hỏi lại trong phiên nhưng vẫn thử lại ở phiên sau (finding Medium: bản đầu
  chỉ xin khi mất focus rồi không bao giờ thử lại).

**Non-functional**
- Nội dung tối giản: "Export finished", **không** dán full path.
- Dựa vào redact tại nguồn từ Phase 2 — đây là lý do Phase 2 phải làm việc đó, vì
  text này rời khỏi tiến trình app và có thể vào lịch sử của OS.

## Đường dẫn thật (finding Medium: bản đầu ghi sai)

- Settings store: `src/lib/settingsStore.svelte.ts` — **không** phải `src/store/`.
- `PrivacySettings.svelte` **không tồn tại**. Công tắc đặt vào section Settings hiện
  có, theo mẫu `SettingsSection` mà repo đã dùng.
- `tauri-plugin-notification` chưa có trong `Cargo.toml` (đang có
  opener/http/updater/deep-link/dialog) — xác nhận đúng.

## Chiều import (finding Medium)

Phase 3 đặt luật một chiều `lib → store`. `osNotify` là `lib`, store gọi nó thì
ngược chiều. **Sửa:** store không gọi `osNotify`. Chỗ đã subscribe event ở
`App.svelte` (Phase 4) gọi cả hai — `settleJob()` và `osNotify()` — cạnh nhau. Store
vẫn không biết gì về Tauri và vẫn test được ở jsdom.

## Related Code Files

- Create: `src/lib/osNotify.ts`
- Modify: `src-tauri/Cargo.toml` — `tauri-plugin-notification = "2"`
- Modify: `src-tauri/src/lib.rs` — đăng ký plugin
- Modify: `src-tauri/capabilities/default.json` — permission
- Modify: `package.json` — `@tauri-apps/plugin-notification`
- Modify: `src/App.svelte` — gọi `osNotify` cạnh `settleJob`
- Modify: `src/lib/settingsStore.svelte.ts` — cờ bật/tắt
- Modify: section Settings hiện có — công tắc

## Implementation Steps

1. Thêm plugin cả hai phía, đăng ký ở `lib.rs`, cấp permission.
2. `osNotify.ts`: guard `isRunningInTauri()`, import động
   `@tauri-apps/plugin-notification` (theo mẫu import động đã dùng trong
   `transferEvents.ts`), kiểm quyền, gửi.
3. `document.hasFocus()` để quyết định có bắn không.
4. Cờ trong `settingsStore.svelte.ts`; đọc trước khi gửi.
5. Nối ở `App.svelte`, chỉ cho sự kiện kết thúc — không nối vào `pushNotification`
   chung (sẽ bắn cho mọi toast lỗi vặt).
6. Test `osNotify` với `isRunningInTauri` giả lập false → no-op.

## Success Criteria

- [x] Job xong khi app ở background → hiện notification OS.
- [x] App đang focus → không hiện.
- [x] Tắt trong Settings → không hiện, không xin quyền.
- [x] Browser mode → không lỗi console; Settings nói rõ đây là tính năng của bản desktop.
- [x] Từ chối quyền → app chạy bình thường, không hỏi lại trong phiên đó.
- [x] Nội dung không chứa đường dẫn host. — có test khẳng định trực tiếp: một lỗi
      `Cannot create /Volumes/backup-2024/img.tar: Permission denied` chỉ đi ra
      ngoài dưới dạng tên transfer.

## Kết quả

vitest 237/237 (7 test `osNotify` + 2 test quyền riêng tư ở lớp wiring) ·
`svelte-check` 0/0 · eslint sạch · `cargo build` + clippy `-D warnings` sạch với
plugin mới.

**Ràng buộc từ Phase 2 được giữ.** Review truy hết mọi đường mà một đường dẫn host
có thể tới notification của OS và không tìm ra đường nào: `e.error` chỉ vào `detail`
trong app; title của import là `baseName(tarPath)` (bỏ thư mục); copy-in/out dùng
đường dẫn **trong container**; nhánh reconcile dùng `targetLabel` mà phía Rust chỉ
đặt bằng tên image / `file_name()` / `container_path`, không bao giờ là `dest`.

Permission chỉ hỏi khi **có thứ để hiện**, không hỏi lúc khởi động — một hộp thoại
không kèm ngữ cảnh là hộp thoại người dùng không thể quyết định đúng.

### Hai Medium từ review

- **Notification thứ hai bị nuốt** khi hai transfer kết thúc cùng lúc: cái thứ hai
  thấy `permissionAsked` đã true và thoát, dù quyền được cấp ngay sau đó. Sửa: chia
  sẻ promise của lần hỏi đang mở.
- **Công tắc Settings hiển thị sai** khi component mount trước lúc settings load
  xong (snapshot một lần → hiện "bật" cho giá trị đã lưu là "tắt", rồi lần bấm sau
  ghi đè bằng giá trị người dùng chưa từng thấy). Sửa: `$derived` đọc xuyên store.

### Ghi nhận, không sửa

`document.hasFocus()` trả false đúng cho các ca cần thiết (minimise, Space khác,
app khác đang ở trước). Chỉ ca bị cửa sổ khác che hoàn toàn là không phát hiện được,
và bù nó đòi thêm capability window mới — không tương xứng.

### Ngoài kế hoạch, theo yêu cầu chủ sở hữu

Empty state của Notification Center đổi từ dòng chữ "Nothing to report." sang một
inbox rỗng có icon ở giữa: panel là một *nơi*, và một nơi trống nên **trông** trống
thay vì thông báo rằng nó trống. Icon `Inbox` là glyph riêng chứ không dùng lại
chuông — chuông là nút vừa bấm để tới đây, lặp lại nó đọc như một lỗi.

## Risk Assessment

- **Rò rỉ qua Notification Center của OS** — giảm nhẹ bằng nội dung tối giản + redact
  tại nguồn (Phase 2). Nếu Phase 2 bị cắt, phase này **phải** cắt theo.
- **Tăng permission surface** — lý do phase này ở P3 và cắt được.
