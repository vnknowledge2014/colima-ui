---
phase: 3
title: API Auth Hardening
status: completed
priority: P2
effort: 2-3 ngày
dependencies: []
---

# Phase 3: API Auth Hardening

## Overview

Gia cố tầng auth của HTTP server. Mức độ khẩn cấp thấp hơn Phase 1-2 (server bind loopback), nhưng cần làm trước khi P0 mở thêm bề mặt streaming và trước khi plan thương mại thêm deep link.

## Bằng chứng

| Vị trí | Vấn đề |
|---|---|
| `src-tauri/src/auth.rs:38-43` | So sánh token bằng substring, không phải so khớp chính xác thời gian hằng số; đường dẫn có hậu tố `/stream` bỏ qua kiểm tra |
| `src-tauri/src/api_server.rs:231` | `/api/auth/token` trả token mà không cần xác thực (chỉ chặn bằng CORS) |
| `src-tauri/capabilities/default.json` | Cho phép `https://*` — quá rộng cho một app chỉ gọi vài nhà cung cấp AI |
| `src/lib/api/client.ts:7` vs `src-tauri/src/api_server.rs:255` | Client hardcode port 11420, server quét 11420-11429 → browser mode hỏng khi port bị chiếm |

## Requirements

- So sánh token: parse header đúng chuẩn, so khớp chính xác, thời gian hằng số.
- Danh sách route miễn auth phải là **allowlist tường minh**, không phải so khớp theo mẫu đường dẫn.
- `/api/auth/token`: siết thêm ngoài CORS (CORS không phải cơ chế kiểm soát truy cập cho client không phải trình duyệt).
- Thu hẹp allowlist `https://*` trong capabilities xuống đúng các host nhà cung cấp AI đang dùng.
- Client phát hiện được port thật của server thay vì hardcode.

## Related Code Files

- Modify: `src-tauri/src/auth.rs` — parse + so khớp hằng số + allowlist route
- Modify: `src-tauri/src/api_server.rs` — siết `/api/auth/token`, ghi port thật ra nơi client đọc được
- Modify: `src-tauri/capabilities/default.json` — thu hẹp host
- Modify: `src-tauri/tauri.conf.json` — CSP `connect-src` khớp với danh sách provider thật
- Modify: `src/lib/api/client.ts` — phát hiện port
- Modify: `src/lib/env.ts` nếu cần
- Create: test cho auth middleware

## Implementation Steps

1. Viết lại `auth.rs`: tách header `Authorization`, kiểm tra tiền tố `Bearer `, so sánh phần còn lại bằng hàm thời gian hằng số.
2. Thay logic bỏ qua theo hậu tố đường dẫn bằng allowlist route tường minh (hiện chỉ `/api/health`).
3. Rà toàn bộ route streaming/SSE/WebSocket xem có route nào đang lọt qua nhờ mẫu `/stream`.
4. `/api/auth/token`: yêu cầu thêm điều kiện ngoài CORS — ví dụ chỉ phục vụ trong N giây đầu sau khi khởi động, hoặc yêu cầu một nonce ghi ra file cục bộ mà chỉ tiến trình cùng người dùng đọc được. Chốt phương án trước khi code.
5. Thu hẹp allowlist host trong `capabilities/default.json` và `connect-src` trong CSP xuống đúng các host mà `src/lib/llmProviders.ts` thực sự gọi.
6. Sửa lệch port: server ghi port thật (ví dụ vào `~/.colima-ui/port`), client đọc; hoặc bỏ hẳn cơ chế quét port.
7. Thêm token vào bộ mẫu `redact()` của Phase 1.

## Tests / Validation

- Unit: auth middleware — thiếu header, sai scheme, token là tiền tố của token đúng, token là hậu tố, token đúng.
- Unit: route allowlist — chỉ `/api/health` qua được khi không có token.
- Integration: mọi route SSE/WS đều yêu cầu auth.
- Thủ công: chiếm port 11420 → browser mode vẫn kết nối đúng port thay thế.
- Thủ công: gọi host AI ngoài allowlist → bị CSP/capabilities chặn.

## Đính chính bằng chứng (kiểm chứng lúc thực thi)

Reviewer nói quá ở một điểm, cần ghi lại cho đúng:

- **"hậu tố `/stream` bỏ qua kiểm tra"** — sai. Route kết thúc bằng `/stream` chuyển sang xác thực bằng query param, **vẫn cần token**. Không phải lỗ hổng bỏ qua auth.
- **"so sánh token bằng substring"** — đúng một nửa. Đường header (`auth.rs:49` cũ) so khớp **chính xác**; chỉ đường SSE (`:40` cũ) dùng `query.contains("token=<expected>")`. Muốn qua vẫn phải biết token, nên cũng không phải bypass.

Vấn đề thật, và là lý do vẫn nên sửa:
1. **Phân loại route theo mẫu** — route mới nào tình cờ kết thúc bằng `/stream` sẽ âm thầm đổi cơ chế auth. Plan P0 sắp thêm bề mặt streaming, nên đây là bẫy bảo trì thật.
2. So khớp không hằng số (token 256-bit trên loopback — rủi ro biên).
3. Parse query bằng substring thay vì tách tham số.

Mức độ đúng: **gia cố phòng thủ theo chiều sâu**, không phải lỗ hổng khai thác được.

## Sai lệch so với plan (quyết định người dùng chốt, 2026-08-11)

**Bước 4 (`/api/auth/token`) và bước 5 (thu hẹp `https://*`) đã huỷ.** Cả hai đều không sửa được mà không mất tính năng:

- Browser mode **bắt buộc** có điểm bootstrap không xác thực để tab trình duyệt lấy token. Mọi cơ chế chặn tiến trình cục bộ khác cũng chặn luôn trình duyệt.
- `https://*` trong capabilities chính là thứ khiến **6/9 provider** hoạt động (OpenRouter, Groq, Together, Mistral, DeepSeek + endpoint tự cấu hình). Frontend gọi LLM qua `@tauri-apps/plugin-http` — bỏ qua CSP, dùng capabilities. Capabilities là **compile-time**, không mở rộng lúc chạy được, nên thu hẹp là mất vĩnh viễn.

Cả hai đã ghi vào `docs/architecture.md` → mục "Accepted risks", kèm hướng sửa thật (đẩy LLM call qua Rust; desktop app truyền token cho browser).

## Success Criteria

- [x] So sánh token là so khớp chính xác, **thời gian hằng số** (`constant_time_eq`).
- [x] Route dùng query-token là **allowlist tường minh** (`QUERY_TOKEN_PATHS`), không theo mẫu đường dẫn. Có test hồi quy cho route `/stream` không nằm trong danh sách.
- [x] Query token được **parse theo tham số**, không substring. Có test cho trường hợp `?redirect=...token=<secret>`.
- [x] Hai cơ chế auth không chồng lấn: header không dùng được ở route SSE và ngược lại (có test).
- [x] ~~`/api/auth/token` có kiểm soát ngoài CORS~~ → huỷ, ghi vào docs.
- [x] ~~`https://*` được thu hẹp~~ → huỷ, ghi vào docs.
- [x] Browser mode hoạt động khi port mặc định bị chiếm (`resolveApiBase()` dò 11420-11429, khớp dải server thật). Xoá luôn bản hardcode trùng trong `Terminal.svelte`.
- [x] Token của app nằm trong bộ redact (cả Rust và TS), có test.

## Risk Assessment

| Rủi ro | Giảm thiểu |
|---|---|
| Siết `/api/auth/token` làm hỏng browser mode | Chốt phương án ở bước 4 trước khi code; test browser mode ngay sau khi đổi |
| Thu hẹp host chặn nhầm provider người dùng tự cấu hình (Ollama, OpenRouter, endpoint riêng) | Cho phép cấu hình thêm host trong Settings, mặc định hẹp |
| Sửa cơ chế port phá luồng khởi động | Giữ 11420 làm mặc định; cơ chế phát hiện chỉ là dự phòng |
