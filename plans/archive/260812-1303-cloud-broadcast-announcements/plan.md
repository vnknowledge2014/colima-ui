---
title: Cloud broadcast announcements
description: >-
  Kênh một chiều vendor → client: release note, security advisory, thông báo bảo
  trì. Một file JSON tĩnh, fetch ở backend, không bảng và không RLS
status: completed
priority: P3
branch: dev
tags:
  - notification
  - cloud
blockedBy: []
blocks: []
created: '2026-08-12T11:53:40.403Z'
createdBy: 'ck:plan'
source: skill
---

# Cloud broadcast announcements

## Overview

Notification Center (đã xong, xem `plans/archive/260812-1247-.../`) hiện chỉ có nội
dung sinh ra tại máy người dùng. Plan này thêm kênh thứ hai: vendor thông báo
release, cảnh báo bảo mật, lịch bảo trì.

**Một chiều, đọc-only.** Không có dữ liệu nào của người dùng đi ra ngoài.

## Quyết định lớn: không dùng bảng Supabase

Bản plan đầu (viết ngay sau red-team) thiết kế một bảng `announcements` trong
Supabase với RLS đọc công khai. **Sai**, và tôi phát hiện khi bắt đầu Phase 1 chứ
không phải lúc lập kế hoạch:

`supabase/config.toml` mở đầu bằng *"This project uses Supabase for **auth and one
Edge Function only**. There are deliberately no tables and no migrations."* Và
`plans/260811-1930-subscription-teams-supabase-schema/phase-03` — đúng phase định
tạo bảng + RLS + migration — có trạng thái **cancelled**, kèm danh sách rủi ro việc
huỷ đó loại bỏ được:

- Hai rủi ro High về RLS (đọc chéo dữ liệu qua anon key công khai)
- Quy trình migration và rủi ro migration phá hoại trên project đang chạy
- *"the anon key ships in every copy of ColimaUI, and **whatever RLS permits, the
  public can read**"*

Red team của tôi có bắt được "hạ tầng migration không tồn tại" nhưng đọc nó thành
**thiếu sót cần bù**, chứ không phải **quyết định có chủ đích**.

**Hình dạng đúng:** announcement là nội dung công khai của vendor, đổi vài lần mỗi
tháng, ai cũng đọc được. Đó là một **file JSON tĩnh**, không phải một bảng quan hệ.
Xuất bản = commit một file. Không bảng, không RLS, không migration, không đảo quyết
định nào.

## Vì sao fetch ở backend chứ không ở webview

`tauri.conf.json` đặt `connect-src` khá chặt và **không** có GitHub. Ba lựa chọn:

| Cách | Đánh giá |
|---|---|
| Nới `connect-src` cho host feed | Nới CSP cho một tính năng P3 |
| Fetch từ webview qua plugin HTTP | Capability đang cho `https://*` — cả webview được gọi mọi host HTTPS |
| **Fetch ở Rust, expose qua API của mình** | CSP không đổi; **một** URL hardcode; browser mode chạy y hệt; đúng kiến trúc sẵn có |

Cách thứ ba cũng là cách mọi thứ khác trong app đang làm: frontend gọi API của mình,
backend nói chuyện với thế giới.

## Phases

| Phase | Name | Status |
|-------|------|--------|
| 1 | [Feed format and backend fetch](./phase-01-feed-format-and-backend-fetch.md) | Completed |
| 2 | [Client polling and rendering](./phase-02-client-polling-and-rendering.md) | Completed |
| 3 | [Harden links and settings](./phase-03-harden-links-and-settings.md) | Completed |

## Bất biến

- **Không dữ liệu người dùng rời máy.** Không job, không đường dẫn, không tên
  container/image. Request duy nhất là một GET không tham số.
- **Không bảng Supabase.** Quyết định `config.toml` giữ nguyên.
- **Không chặn gì.** Feed không tới được → Notification Center vẫn chạy đủ với nội
  dung local. Không toast lỗi: người dùng không làm gì sai và không sửa được.
- **Tắt là tắt hẳn.** Công tắc off → không có request nào, không phải lọc kết quả
  sau khi đã gọi.

## Điều phải nói thẳng về quyền riêng tư

Kể cả khi không gửi gì lên, việc *fetch* một file đã lộ **IP + thời điểm** cho bên
host feed. (Bản viết đầu ghi thêm "phiên bản app"; kiểm code thì `reqwest` không
đặt user agent nào, nên phiên bản không lộ — lọc theo version làm ở máy sau khi
file về.) Đó vẫn là dữ liệu telemetry theo đúng nghĩa. Hệ quả bắt buộc:

- `docs/telemetry.md` phải mô tả kênh này.
- Công tắc phải thật sự chặn request, không phải lọc kết quả.
- Không đính user id, không đính máy, không cookie.

## Findings kế thừa từ red-team 2026-08-12

| # | Finding | Sev | Phase |
|---|---------|-----|-------|
| 5 | `link_url` do vendor kiểm soát đưa thẳng vào `openExternal()`, hàm không validate scheme/host (`src/lib/external-links.ts:110-122`), `opener:default` đã cấp | Critical | 3 |
| 13 | Lọc tier không khả thi như đặc tả: `pro.svelte.ts` đánh dấu `subscription` display-only và null nó khi đọc lỗi; chỉ `paid: boolean` bền | High | 2 |
| 14 | Hạ tầng migration không tồn tại | High | **Không còn áp dụng** — không có migration nào nữa |

Kèm Medium: `title`/`body` chưa bắt buộc plain text trong khi repo có sẵn mẫu
`{@html} renderMarkdownHTML` (`AiChatPanel.svelte:490`); watermark trên UUID vô
nghĩa (UUID không có thứ tự).

## Acceptance criteria

- [x] Announcement mới hiện trong Notification Center trong vòng một lần poll.
- [x] Đã đọc → không hiện lại sau khi khởi động lại app.
- [x] Feed không tới được → không toast lỗi, notification local vẫn hoạt động đủ.
- [x] `audience: 'pro'` không hiện cho Free; lỗi đọc entitlement **không** giấu mất
      advisory `critical`.
- [x] Tắt công tắc → **0** request tới host feed. (Cơ chế xong ở Phase 2, UI ở
      Phase 3.)
- [x] `link_url` scheme/host lạ → không render link.
- [x] `body` chứa thẻ HTML → hiện ra như text, không thực thi.
- [x] Không request nào chứa đường dẫn, tên container, tên image, hay user id.
- [x] `docs/telemetry.md` mô tả đúng kênh này. Một đính chính: fetch lộ **IP +
      thời điểm**, không lộ phiên bản app — `reqwest` không đặt user agent nào.

## Open questions

Đã chốt trong lúc làm:

- **Feed sống ở đâu?** `announcements.json` ở gốc repo, nhánh `main`, đọc qua
  `raw.githubusercontent.com`. Xuất bản = commit.
- **Ai duyệt nội dung?** Code review, như mọi thay đổi khác trong repo.

Còn lại:

- **File chưa lên `main`.** URL feed hiện trả 404 — đúng đường lỗi đã thiết kế
  (client giữ nguyên nội dung đang hiển thị), nhưng sẽ không có announcement nào
  hiện cho tới khi `announcements.json` được push.
- **Domain vendor chưa có trong allowlist** (`external-links.ts`), vì chưa có
  domain. Khi có, thêm một dòng ở đó.
