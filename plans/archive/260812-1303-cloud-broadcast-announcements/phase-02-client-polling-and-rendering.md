---
phase: 2
title: "Client polling and rendering"
status: completed
priority: P3
dependencies: [1]
effort: "M"
---

# Phase 2: Client polling and rendering

## Overview

Poll feed, lọc phía client, đẩy vào notification store đã có dưới dạng
`kind: "announcement"`.

## Poll, không phải realtime

Announcement đổi vài lần mỗi tháng. Poll lúc khởi động app và mỗi 6 giờ. Không có
kênh đẩy nào ở đây và cũng không cần: `raw.githubusercontent.com` là file tĩnh.

## Lọc theo tier — không khả thi như đặc tả gốc (finding #13)

Bản plan đầu ghi "lọc theo tier dùng entitlement đã resolve offline". Đối chiếu code:

- `pro.svelte.ts` đánh dấu `subscription` là **display-only** và **null hoá** nó khi
  đọc thất bại.
- Chỉ `paid: boolean` là bền. Không có khái niệm `pro_teams` ở frontend.

Nếu làm nguyên bản, một lỗi IPC thoáng qua sẽ **giấu mất một advisory `critical`
khỏi người dùng đang trả tiền** — kiểu hỏng tệ nhất của một kênh cảnh báo.

**Quy tắc dứt khoát:**

1. Lọc `audience` chỉ dùng `paid: boolean`. `audience: "pro"` hiện khi `paid === true`.
2. **`severity: "critical"` bỏ qua mọi bộ lọc audience.** Cảnh báo bảo mật không được
   phụ thuộc vào việc đọc entitlement có thành công hay không.
3. Không đọc được entitlement → coi `paid` là chưa xác định: hiện mọi thứ trừ nội
   dung `audience: "pro"` không phải `critical`. Thà thừa một release note còn hơn
   thiếu một advisory.

## So sánh version

`minVersion`/`maxVersion` so sánh **semver**, không phải lexicographic — nếu không
`"1.10.0"` sẽ nhỏ hơn `"1.9.0"` và ẩn mất advisory. Viết một hàm so sánh nhỏ, không
kéo dependency cho ba con số.

## Đã đọc thì đừng hiện lại

Lưu **tập id đã đọc** trong `src/lib/settingsStore.svelte.ts` (SQLite, đã persist).
Đây là ngoại lệ có chủ đích với nguyên tắc "notification store không persist":
announcement hiện lại mỗi lần mở app sẽ bị coi là spam. Chỉ id được lưu, không lưu
nội dung.

Cắt tập đó theo `MAX` (vài trăm id) để nó không phình vô hạn; id cũ nhất rơi ra
trước, và một announcement đã hết hạn thì cũng không còn trong feed để hiện lại.

## Store

`NotificationKind` thêm `"announcement"`. Phase 3 của plan trước đã cố tình **bỏ**
nó ra ("plan cloud tách riêng sẽ tự thêm khi cần") — giờ là lúc đó.

Announcement **không** gộp như message: hai announcement khác nhau là hai tin, và
`id` từ feed là danh tính thật.

## Related Code Files

- Create: `src/lib/api/announcements.ts`
- Create: `src/lib/announcements.ts` — poll, lọc, so sánh semver
- Create: `src/lib/announcements.test.ts`
- Modify: `src/store/notifications.svelte.ts` — `kind: "announcement"`
- Modify: `src/components/notifications/NotificationItem.svelte` — nhánh render
- Modify: `src/App.svelte` — kích hoạt poll
- Modify: `vite.config.ts`, `vitest.config.ts`, `src/vite-env.d.ts` — `__APP_VERSION__`

`settingsStore.svelte.ts` **không** phải sửa: nó đã là key/value chung, nên tập id
đã đọc chỉ là một key (`announcements.read_ids`, JSON array) chứ không phải một
trường mới. Ít bề mặt hơn một chút so với dự kiến.

## Implementation Steps

1. `announcements.ts` client gọi `GET /api/announcements` / command.
2. Chọn locale theo `i18n`, fallback `en` khi thiếu.
3. Lọc audience theo quy tắc ba điểm ở trên; `critical` luôn lọt.
4. So sánh semver cho min/max version.
5. Poll lúc mount + `setInterval` 6h; huỷ interval khi teardown.
6. Đẩy announcement chưa đọc vào store; lưu id đã đọc.
7. Lỗi mạng → nuốt im lặng, log mức debug. **Không toast.**
8. Test: `critical` hiện khi đọc entitlement lỗi; semver `1.10.0` > `1.9.0`; id đã
   đọc không hiện lại; feed lỗi không xoá nội dung đang hiển thị.

## Success Criteria

- [x] Announcement mới hiện trong một lần poll.
- [x] Đã đọc → không hiện lại sau restart.
- [x] Ngắt mạng → không toast lỗi; entry local không bị đụng.
- [x] `audience: "pro"` không hiện cho Free.
- [x] Đọc entitlement thất bại → advisory `critical` **vẫn hiện**.
- [x] `minVersion: "1.9.0"` không ẩn advisory với app `1.10.0`.
- [x] Announcement không bị gộp với nhau hay với message.
- [x] `pnpm test` 262/262, `pnpm typecheck`, `pnpm check`, `pnpm lint`, `pnpm build` xanh.

## Kết quả

25 test mới. Vài chỗ hình dạng thật khác dự kiến, đáng ghi lại:

**Phiên bản app không tồn tại ở frontend.** Không có hằng số nào để so với
`minVersion`. Thêm `__APP_VERSION__` qua `define` của Vite, đọc từ `package.json`
— cùng con số `tauri.conf.json` đang ship, nên không có nguồn thứ hai để lệch.
`vitest.config.ts` là config riêng chứ không kế thừa `vite.config.ts`, nên phải
khai `define` ở cả hai chỗ.

**Công tắc tắt làm luôn ở phase này, không đợi Phase 3.** Phase 2 bật một kênh
mạng thật; để nó chạy không lối tắt cho tới phase sau là để một khoảng thời gian
không có cách nào tắt. `announcementsEnabled()` đọc `announcements.enabled`
(mặc định bật) và chặn **trước** `fetch`. Phase 3 chỉ còn thêm UI và
`docs/telemetry.md`.

**`linkUrl` cố tình chưa render.** Feed đã mang nó về và store đã giữ nó, nhưng
không có markup nào đọc tới — đó là finding #5 (Critical) và nó thuộc Phase 3.
Đưa link ra trước phần validate scheme/host sẽ đúng bằng việc chưa làm gì cả.

**Ba chỗ sửa sau review:**

- `App.svelte` gán teardown **trước** chuỗi `await`, dạng cờ "đừng khởi động".
  Poll bắt đầu sau bốn lần await; một unmount tới trước sẽ để lại interval 6 giờ
  không ai dọn. Chính file này đã ghi bất biến đó cho transfer rồi.
- `pollAnnouncements` dùng chung một promise khi đang chạy. Poll khởi động và
  một nhịp interval chồng nhau thì cả hai đọc tập id trước khi ai kịp ghi.
- `selectAnnouncements` khử trùng lặp trong chính một feed: announcement không
  bao giờ được gộp, nên một `id` lặp sẽ ra hai dòng giống hệt.

## Risk Assessment

- **Phạm vi trườn sang notification theo user** — ranh giới: feed là **một file
  tĩnh dùng chung**. Không có chỗ nào để cá nhân hoá, và đó là tính năng chứ không
  phải hạn chế.
- **Announcement thành kênh quảng cáo** làm hỏng lòng tin → `severity` chỉ có ba giá
  trị vận hành, không có `promo`.
- **Đồng hồ máy sai** làm `expiresAt` xử lý sai. Chấp nhận: hệ quả xấu nhất là một
  tin cũ hiện thêm một lần.
