---
phase: 4
title: Transfer UI to background
status: completed
priority: P1
dependencies:
  - 2
  - 3
effort: M
---

# Phase 4: Transfer UI to background

## Overview

Dialog thu về đúng vai trò: nhận input, Start, đóng. Progress đi vào store. Một
subscription duy nhất ở tầng app, có reconnect và đối soát — không có hai thứ sau
thì "chạy nền" chỉ là mất dấu job.

## Requirements

**Functional**
- Bấm Start → dialog đóng ngay, job chạy tiếp.
- Lỗi **đồng bộ** lúc start (thiếu chỗ trống, file đã tồn tại, path sai) giữ dialog
  mở. Chỉ `transfer.failed` mới đi ra notification.
- Mất kết nối SSE rồi nối lại → không entry nào kẹt "running".
- Mở app khi đã có job chạy sẵn (reload webview) → job xuất hiện lại.

**Non-functional**
- Đúng **một** `EventSource` cho cả phiên ở browser mode.
- Nhãn nút phải nói rõ hành vi mới.

## A. Một subscription, có reconnect (finding #10)

`transferEvents.ts:60-79` **không có `onerror` và không reconnect** — khác hẳn
`dataPoller.ts:165-177` vốn có. Ở phạm vi per-dialog điều đó ẩn đi vì mỗi dialog mở
một EventSource mới; nâng lên phạm vi app thì một cú rớt ghim job ở "running" vĩnh
viễn.

Thêm vào đó, SSE là kênh **lossy có chủ đích**: `broadcast::channel(64)`
(`sse.rs:47`) trong khi `POLL_INTERVAL` 200ms (`streaming_cmd.rs:33`) cho ~5 event/s
mỗi job. `transferEvents.ts` bỏ qua `stream-lagged`.

**Sửa:**
1. `onerror` + backoff reconnect theo đúng mẫu `dataPoller.ts` đã dùng.
2. Sau **mỗi lần** (re)connect: gọi `transfer_list` (Phase 2) và `reconcile()` vào
   store. Đây mới là thứ sửa được cả lag lẫn rớt kết nối — reconnect đơn thuần không
   lấy lại event đã mất.
3. Xử lý `stream-lagged` như một tín hiệu phải đối soát ngay.

Đối soát là nguồn sự thật, event chỉ là đường nhanh. Đảo lại thứ tự ưu tiên đó là
lỗi thiết kế của bản plan đầu.

## B. Đóng cửa sổ mất event lúc start (finding #3)

Phase 2 đã đăng ký job trước khi spawn, nên `transfer_list` luôn biết. Phía frontend:
- `pushNotification` ngay khi `TransferStarted` về.
- `settleJob` với id chưa biết vẫn tạo entry (Phase 3) — phòng trường hợp
  `transfer.failed` về trước cả response.

## C. Dialog

- Bỏ `subscribeTransfer` khỏi dialog; bỏ khối progress, `failure`, nút "Cancel
  transfer" (chuyển sang Phase 5).
- Nhãn nút: **"Start in background"**.
- Copy-out: thêm một dòng nói rõ kết quả là archive `.tar` — hành vi mới từ Phase 1,
  không để người dùng tự phát hiện.
- Lỗi đồng bộ hiện tại chỗ, dialog không đóng.

## Related Code Files

- Modify: `src/lib/transferEvents.ts` — onerror/backoff, `stream-lagged`, hook đối soát
- Modify: `src/lib/api/transfer.ts` — thêm `list()`
- Modify: `src/App.svelte` — đăng ký một lần, nối vào store
  (**lưu ý**: `ToastContainer` đang mount hai lần ở `:203` và `:244` — lỗi có sẵn,
  ghi nhận, đừng nhân bản kiểu đó cho panel)
- Modify: `src/components/transfer/TransferDialog.svelte` — start + đóng
- Modify: `src/components/transfer/TransferDialog.test.ts` — dialog đóng sau start;
  lỗi validate **không** đóng dialog

## Implementation Steps

1. `transferApi.list()` gọi `transfer_list` / `GET /api/transfers`.
2. `transferEvents.ts`: onerror + backoff; callback `onReconnect` để chủ gọi đối soát.
3. `App.svelte`: subscribe một lần; đối soát lúc mount và mỗi lần reconnect.
4. Dialog: bỏ subscribe/progress; start rồi `onClose()`; giữ lỗi đồng bộ tại chỗ.
5. Nhãn nút + dòng giải thích `.tar` cho copy-out.
6. Test: đóng sau start; lỗi validate giữ dialog; mock reconnect → đối soát chạy.

## Success Criteria

- [x] Bấm Start → dialog đóng trong cùng frame; job chạy tới xong.
- [x] Lỗi validate lúc start giữ dialog mở, hiện thông báo tại chỗ.
- [x] Mở dialog 5 lần ở browser mode chỉ tạo **một** `EventSource`. — dialog không
      còn subscribe; đúng một call site `subscribeTransfer` trong toàn bộ src.
- [x] Ngắt mạng giữa job rồi nối lại → entry cập nhật đúng trạng thái cuối, không kẹt.
- [x] Reload webview giữa lúc job chạy → job xuất hiện lại sau khi mount.
      — nhánh Tauri cũng phát `onDesync` một lần sau khi gắn listener, vì Tauri
      không replay gì đã emit trước đó.
- [x] Dialog copy-out nói rõ kết quả là `.tar`.

## Kết quả

vitest 217/217 (9 test dialog viết lại + 10 test lớp nối) · `svelte-check` 0/0 ·
eslint sạch · `cargo test --lib` 304/304 không hồi quy.

`src/lib/transferNotifications.ts` là lớp nối event→store, tách khỏi cả `App.svelte`
lẫn store: quy tắc đối soát test được mà không cần mount app, và store không phải
biết gì về transport.

### Ba lỗi High từ code review

1. **Đua đối soát kết thúc nhầm transfer đang sống.** `reconcileJobs` đánh dấu mọi
   job đang chạy vắng mặt trong snapshot là `ended`. Một request gửi đi *trước* khi
   job bắt đầu nhưng trả về *sau* sẽ khai tử job đó. Sửa: snapshot mang mốc `asOf`
   lấy **trước** khi gửi request, và chỉ job có `startedAt` **nghiêm ngặt sớm hơn**
   mới bị kết luận. Cùng ms là mơ hồ → bỏ qua, tốn thêm một vòng đối soát còn hơn
   bỏ rơi một transfer đang ghi đĩa. Thêm in-flight guard + nhịp tối thiểu 2s.
2. **Một transfer thành hai entry.** Backend spawn trước khi trả id, nên `settleJob`
   có thể tạo entry trước; `startJob` sau đó tạo entry thứ hai vì job cố tình không
   bao giờ được gộp. Sửa: `startJob` nhận nuôi entry cùng `jobId` nếu đã có.
3. **Đường dẫn host rò vào notification.** Title của import dùng `tarPath`, detail
   dùng `${destDir}/${fileName}` — mâu thuẫn với hợp đồng "entry không mang đường
   dẫn host" mà `formatEntryForClipboard` dựa vào để nói với người dùng rằng nội
   dung an toàn để dán vào issue. Sửa: chỉ dùng tên file.

Ba Medium: refresh danh sách image chuyển từ lúc đóng dialog (giờ là lúc *bắt đầu*,
chưa có gì mới) sang lúc transfer thật sự xong; subscribe chuyển lên trước `await`
đầu tiên trong `onMount` để teardown sớm không rò; và một byte NUL thật lọt vào
`notifications.svelte.ts` khiến git/grep coi file là binary — đổi sang escape.

## Risk Assessment

- **Đối soát chạy quá thường xuyên** khi mạng chập chờn → backoff đảm nhận; đối soát
  là một GET nhỏ, không đắt.
- **Chênh lệch event vs đối soát** (event nói done, list vẫn thấy running trong TTL):
  quy tắc dứt khoát — trạng thái kết thúc thắng trạng thái đang chạy, bất kể đến từ
  đâu.
