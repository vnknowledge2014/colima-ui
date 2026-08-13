---
phase: 5
title: Notification Center UI
status: completed
priority: P1
dependencies:
  - 3
  - 4
effort: M
---

# Phase 5: Notification Center UI

## Overview

Panel liệt kê notification, mở từ nút chuông có badge. Job đang chạy hiện inline với
progress và nút huỷ — đây là nơi progress bị lấy khỏi modal ở Phase 4 đi tới.

## Requirements

**Functional**
- Badge hiện số chưa đọc, ẩn khi bằng 0.
- Job đang chạy luôn trên cùng, có progress + huỷ.
- Entry lỗi có nút "Details" mở `ErrorDetailPanel` như toast đang làm.
- Mở panel = đánh dấu đã đọc phần đang nhìn thấy.
- "Clear all" chỉ xoá entry đã kết thúc.

**Non-functional**
- Esc đóng, focus trap, `aria-live` cho entry mới.
- Tôn trọng `prefers-reduced-motion` như `ToastContainer` đang làm.

## Sidebar: phải tự dựng panel-toggle (finding #15)

Bản plan đầu ghi "dùng cơ chế panel-toggle đã có" — **không có cơ chế nào cả**.
`Sidebar.svelte:306` là `{@const isPanelToggle = item.id === "ai-chat"}`, hardcode
đúng một id. Đồng thời bản plan đầu lại tự nói "đừng thêm vào `navGroups`" — hai câu
loại trừ nhau.

**Quyết định:** nút chuông đặt ở vùng header của sidebar, **ngoài** `navGroups`. Nó
không phải điểm đến điều hướng. Không tổng quát hoá `isPanelToggle` trong phase này —
một id hardcode thứ hai rẻ hơn một abstraction cho hai trường hợp. Khi có cái thứ ba
thì hẵng tổng quát hoá.

## Kiến trúc

```
Sidebar header (nút chuông + badge) → openPanel()
NotificationPanel.svelte
   └── NotificationItem.svelte
         ├ kind="job"     → progress + Cancel
         └ kind="message" → text + Details (nếu có error)
   footer: Mark all read · Clear all
```

Progress bar: CSS đang **chỉ** ở `TransferDialog.svelte:373-386` và Phase 4 xoá luôn
consumer đó. Bản plan đầu ghi "trùng lặp, cần tách ra `components.css`" — sai, không
có chỗ thứ hai. **Chuyển** style sang panel, không phải tách dùng chung.

## Huỷ: dùng kết quả thật (finding #11)

Phase 2 đổi `cancel_transfer` sang enum. Panel phải dùng nó:
- `Cancelled` → chuyển sang `cancelling`, chờ `transfer.done { cancelled: true }` xác
  nhận (backend là nguồn sự thật).
- `AlreadyFinished` → cập nhật entry sang trạng thái cuối ngay.
- `UnknownJob` → entry đã lạc; đối soát lại qua `transferApi.list()`.

Bản plan đầu vứt giá trị trả về → trạng thái "cancelling" treo mãi.

## Related Code Files

- Create: `src/components/notifications/NotificationPanel.svelte`
- Create: `src/components/notifications/NotificationItem.svelte`
- Create: `src/components/notifications/NotificationPanel.test.ts`
- Modify: `src/components/Sidebar.svelte` — nút chuông + badge ở header
- Modify: `src/components/Icons.svelte` — icon chuông
- Modify: `src/App.svelte` — mount panel (một lần)
- Modify: `src/store.svelte.ts` — `uiState` điều phối panel nào đang mở

## Implementation Steps

1. Icon chuông theo đúng cách các icon khác khai báo.
2. Nút + badge ở header sidebar, ngoài `navGroups`.
3. `NotificationItem.svelte`: một `{#if}` theo `kind` — hai biến thể chia chung khung
   hàng, tách component sẽ là DRY ngược.
4. `NotificationPanel.svelte`: job đang chạy lên đầu, còn lại theo timestamp giảm dần.
5. Huỷ dùng enum kết quả như trên.
6. Chuyển CSS progress bar từ dialog sang panel.
7. Mở panel này đóng panel AI — quản ở `uiState`, không để hai component tự thoả thuận.
8. Test: badge đếm đúng; huỷ gọi API và xử lý cả ba nhánh; clear-all giữ job đang chạy.

## Success Criteria

- [x] Job đang chạy nhìn thấy được từ mọi page, có progress và huỷ được.
- [x] Badge khớp `unreadCount`, về 0 sau khi mở panel. Entry đến **trong lúc panel
      đang mở** được đánh dấu đã đọc ngay — nếu không badge sẽ sáng sau lưng chính
      panel đang hiển thị nó.
- [x] "Clear all" không làm biến mất job đang chạy.
- [x] Huỷ một job vừa kết thúc → entry về trạng thái cuối, không treo "cancelling".
- [x] Esc đóng panel; Tab không thoát ra ngoài khi panel mở. Focus **được đưa vào**
      panel khi mở và **trả về** nút chuông khi đóng.
- [x] Mở panel notification đóng panel AI và ngược lại — cả hai chiều, và cả hai
      đường mở panel AI (`showPanel` lẫn nav item trong Sidebar).

## Kết quả

vitest 228/228 (11 test panel) · `svelte-check` 0/0 · eslint sạch · `cargo test --lib`
304/304 không hồi quy.

### Một lỗi Critical, đúng loại mà Phase 3 đã đặt ra để tránh

`cancel()` map `alreadyFinished` → `settleJob("success")`. Nhưng registry trả biến
thể đó cho **mọi** trạng thái kết thúc (`transfer_registry.rs`, `is_terminal()`) —
gồm cả `Failed`. Nghĩa là một export thất bại nhận dấu ✓ xanh. Đây đúng là cú đoán
mà Phase 3 thêm trạng thái `ended` để khỏi phải làm.

Sửa: không đoán. Retention 60s nghĩa là job vừa kết thúc **vẫn còn trong `list()`**,
nên gọi `reconcileTransfers()` là biết sự thật; chỉ khi vẫn chưa giải quyết được
(job đã quá hạn lưu) mới dùng `ended` — vốn có nghĩa chính xác là "đã xong, client
này không thấy xong thế nào".

### Bốn lỗi High

- **`cancelling` kẹt vĩnh viễn** khi request huỷ ném lỗi: không event nào tới,
  `reconcileJobs` từ chối hạ cấp, `clearFinished` từ chối xoá job còn chạy. Sửa:
  trả trạng thái về như cũ khi request không tới nơi.
- **`aria-live` trên `<ul>` làm ngập screen reader**: progress đổi ~5 lần/giây, mỗi
  lần là một thông báo xếp hàng ("polite" là hoãn, không phải gộp). Sửa: bỏ khỏi
  list, thay bằng một vùng `sr-only` chỉ chứa kết cục cuối cùng — progress không
  chạm tới nó.
- **Focus trap không bao giờ nhận focus**: không chỗ nào gọi `.focus()`, nên Tab
  đầu tiên rơi ra sau backdrop. Sửa: đưa focus vào panel khi mở, trả về nút chuông
  khi đóng.
- **`<aside aria-label>` chỉ là landmark**: trap + backdrop mà không `role="dialog"`
  / `aria-modal` là modal nửa vời — focus bị nhốt nhưng con trỏ ảo thì không.

Sáu Medium: coordination chiều ngược lại (mở panel AI không đóng notification —
và comment tôi viết trong `store.svelte.ts` mô tả sai lý do); badge sáng sau lưng
panel đang mở; sắp xếp hiển thị theo thứ tự chèn nên hàng vừa settle nhảy chỗ và
hiện giờ mới hơn hàng phía trên; thiếu `role="progressbar"`; và khoá locale
`notifications.*` chưa có ở cả 4 file → Phase 7.

## Risk Assessment

- **Chật chỗ ở sidebar header** — nút chuông là một icon, chấp nhận được.
- **Panel chồng AI panel** → bước 7.
- **Trạng thái "cancelling" treo** nếu backend không xác nhận: đặt timeout đối soát
  qua `transferApi.list()` thay vì tin tưởng vô hạn.
