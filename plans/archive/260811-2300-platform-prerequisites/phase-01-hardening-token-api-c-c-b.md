---
phase: 1
title: Hardening token API cục bộ
status: completed
priority: P1
dependencies: []
effort: ''
---

# Phase 1: Hardening token API cục bộ

## Overview

`GET /api/auth/token` nằm trong router **public** và trả thẳng bearer token cho bất kỳ ai gọi (`api_server.rs:255-257`, `routes/system.rs:261-264`). Đây là lỗ hổng **đang tồn tại trong code hiện tại**, không phải nợ của tính năng tương lai — nên phase này làm trước tất cả.

## Bối cảnh: phần đã đúng

`auth.rs` được xây tử tế và **không cần sửa**: token CSPRNG 32 byte, so sánh constant-time, parse query đúng cách (`?redirect=token%3Dx` không lọt), danh sách `QUERY_TOKEN_PATHS` tường minh thay vì so khớp hậu tố `/stream`. Có test hồi quy cho từng điểm.

Vấn đề nằm đúng một chỗ: cơ chế xác thực tốt đó bị vô hiệu bởi một endpoint phát token miễn phí.

## Requirements

**Functional**
- Tiến trình local tuỳ ý **không** lấy được token qua HTTP.
- Browser mode (người dùng mở `http://localhost:11420` trong trình duyệt) vẫn phải dùng được — đây là lý do endpoint tồn tại.
- Tauri mode không bị ảnh hưởng: nó lấy token qua Tauri command, không qua HTTP.

**Non-functional**
- Không phá vỡ `/api/health` (vẫn public, không trả gì nhạy cảm).

## Architecture

CORS (`api_server.rs:53-58`) chỉ ràng buộc **browser**, không ràng buộc `curl` hay tiến trình native. Nhận định "Protected by CORS (localhost-only origins)" trong docstring của `api_auth_token` là **sai về mô hình đe doạ** và phải sửa cùng code.

### Đã implement (2026-08-12) — không phải hướng A/B/C nào

Ba hướng đề ra lúc lập plan (A: tiêm token vào HTML server render; B: one-time
token; C: dialog xác nhận native) đều dựa trên một giả định sai: rằng server có
phục vụ trang cho browser mode.

**Nó không phục vụ gì cả.** Không `ServeDir`, không fallback, không Dockerfile,
không nginx. CORS cho phép `localhost:1420` = vite dev server. Browser mode là
bề mặt **phát triển**, không phải đường dùng của người dùng cuối. Hướng A do đó
bất khả thi, còn B và C giải sai bài toán.

Thêm một phát hiện: **Tauri mode cũng đang bootstrap qua chính lỗ hổng này.**
`getApiToken()` gọi `invoke("get_platform")` — không phải command token, nên
nhánh IPC luôn fail và rơi xuống endpoint HTTP công khai.

Thiết kế đã làm — đúng đề xuất `docs/architecture.md:168` đã ghi sẵn:

| Client | Lấy token thế nào |
|---|---|
| Desktop (Tauri) | `invoke("api_token")` qua IPC. Webview là app, đã nằm trong trust boundary; tiến trình local khác thì không. |
| Browser | URL fragment `#token=…`. Fragment không bao giờ gửi lên server → không vào access log, không vào `Referer`. Frontend đọc một lần rồi xoá khỏi thanh địa chỉ. |
| Server | `/api/auth/token` **bị xoá khỏi router**. Debug build in URL sẵn dùng ra stdout khi bind. |

Không có affordance "mở trong trình duyệt" trong app, vì không có gì để mở —
chủ ý, và được ghi rõ trong `auth.rs` để người sau không tưởng là thiếu sót.

## Related Code Files

- Modify: `src-tauri/src/api_server.rs:255-257` (router public)
- Modify: `src-tauri/src/routes/system.rs:261-264` (`api_auth_token` + docstring sai)
- Modify: `src-tauri/src/auth.rs` (nếu chọn hướng B/C — thêm state one-time hoặc approval)
- Modify: `src/lib/api/client.ts` (cách frontend lấy token ở browser mode)
- Kiểm tra: `docs/api.md` nếu có mô tả endpoint này

## Implementation Steps

1. Grep mọi nơi frontend gọi `/api/auth/token` — xác định browser mode thực sự phụ thuộc thế nào.
2. Chọn hướng A/B/C dựa trên kết quả bước 1.
3. Implement. Sửa docstring: CORS không phải cơ chế uỷ quyền chống tiến trình local.
4. Test: `curl http://127.0.0.1:11420/api/auth/token` phải thất bại (hoặc, với hướng A, endpoint không còn tồn tại).
5. Test hồi quy: browser mode vẫn đăng nhập và gọi API được.
6. Cập nhật `docs/api.md`.

## Success Criteria

- [ ] `curl` lấy token từ ngoài app: **thất bại**.
- [ ] Browser mode vẫn hoạt động đầy đủ.
- [ ] Tauri mode không đổi hành vi.
- [ ] Docstring không còn tuyên bố CORS là cơ chế bảo vệ.
- [ ] Toàn bộ test hiện có trong `auth.rs` vẫn xanh (không sửa `auth.rs` trừ khi chọn hướng B/C).

## Risk Assessment

| Rủi ro | Giảm thiểu |
|---|---|
| Phá browser mode của người dùng đang dùng | Bước 1 bắt buộc: đo mức phụ thuộc trước khi chọn hướng |
| Hướng A cần server render trang, đụng build pipeline | Khảo sát trước; nếu tốn kém thì chọn C |
| Tưởng đã vá xong nhưng còn đường khác lấy token | Grep toàn repo mọi nơi token bị trả ra qua HTTP, không chỉ endpoint này |
