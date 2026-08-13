---
phase: 1
title: Secret Redaction
status: completed
priority: P1
effort: 2-3 ngày
dependencies: []
---

# Phase 1: Secret Redaction

## Overview

API key của nhà cung cấp AI đang lọt vào chuỗi lỗi mà người dùng nhìn thấy và copy đi.

## Bằng chứng

| Vị trí | Vấn đề |
|---|---|
| `src-tauri/src/commands/ai_chat.rs:174`, `:477` | Key Gemini được đặt **trong URL** (`?key=...`) |
| `src-tauri/src/commands/ai_chat.rs:180` | Lỗi request — bao gồm cả URL — được format thẳng vào message trả về |
| `src/pages/Containers.svelte:96` | UI render `String(e)` trực tiếp |
| `src-tauri/src/routes/kb.rs:13` | Trả `ai_api_key` cho bất kỳ ai có bearer token |

Kịch bản không cần kẻ tấn công: gọi Gemini lỗi → thông báo lỗi chứa URL kèm key → người dùng copy dán lên GitHub issue.

## Requirements

- Một hàm `redact()` duy nhất, áp dụng ở **mọi** đường ra: chuỗi lỗi, log, telemetry sau này, crash report sau này.
- Che: query string của URL, `key=`, `token=`, `api_key=`, header `Authorization`/`Bearer`, và các key khớp mẫu của từng provider.
- Ưu tiên **allowlist chỗ được phép lộ** hơn là blocklist chỗ phải che, ở những nơi khả thi.
- Chuyển key Gemini từ query string sang header nếu API hỗ trợ; nếu không thì che ở tầng lỗi.
- Không log key ở bất kỳ mức log nào.

## Related Code Files

- Create: `src-tauri/src/redact.rs` + test
- Modify: `src-tauri/src/commands/ai_chat.rs` — bỏ key khỏi URL nếu được; redact trước khi dựng lỗi
- Modify: `src-tauri/src/routes/kb.rs` — không trả `ai_api_key` ra response
- Modify: `src-tauri/src/error.rs` — redact tại điểm dựng lỗi
- Modify: `src-tauri/src/helpers.rs` — redact stderr trước khi đưa vào lỗi
- Modify: `src/pages/Containers.svelte` — bỏ render `String(e)` thô
- Create: `src-tauri/tests/` — test hồi quy

## Implementation Steps

1. Viết `redact.rs` với bộ mẫu, kèm test dùng key giả có hình dạng thật của từng provider (Anthropic, OpenAI, Google, Groq, Mistral, DeepSeek, OpenRouter, Together).
2. Kiểm tra API Gemini có nhận key qua header không (`x-goog-api-key`); nếu có thì chuyển, bỏ hẳn key khỏi URL.
3. Áp `redact()` tại mọi điểm dựng lỗi trong `ai_chat.rs`, `helpers.rs`, `error.rs`.
4. Bỏ `ai_api_key` khỏi response của `routes/kb.rs`; nếu frontend cần biết "đã cấu hình key chưa" thì trả boolean.
5. Rà toàn bộ `src/` tìm chỗ render lỗi thô ra UI; thay bằng chuỗi đã chuẩn hoá.
6. Thêm test hồi quy: dựng lỗi từ URL có chứa key → khẳng định output không chứa key.

## Tests / Validation

- Unit: `redact()` che đúng 8 định dạng key provider + bearer token + query string.
- Unit: lỗi dựng từ `ai_chat` không chứa key (test cho cả nhánh thành công lẫn thất bại).
- Thủ công: cấu hình key sai có chủ ý → gây lỗi → kiểm tra toast, log, và clipboard đều sạch.
- `grep` toàn bộ log runtime sau một phiên dùng AI để xác nhận không có key.

## Sai lệch so với plan (quyết định lúc thực thi, 2026-08-10)

**Bước 4 ("bỏ `ai_api_key` khỏi response của `routes/kb.rs`") đã bị huỷ.** Plan giả định key chỉ dùng ở backend. Thực tế frontend gọi thẳng LLM (`src/lib/llmProviders.ts`) nên **cần** key: `AiChatPanel.svelte:62`, `Instances.svelte:84`, `settingsStore.svelte.ts:13`. Bỏ key khỏi response sẽ làm AI chat chết hoàn toàn.

Gốc vấn đề không nằm ở endpoint này mà ở `api_server.rs:231` — `/api/auth/token` cấp bearer token **không cần xác thực**. Ai lấy được token thì đọc được mọi thứ, không riêng key. Đó là Phase 3.

Quyết định (người dùng chốt): giữ nguyên hành vi hiện tại, sửa đúng gốc ở Phase 3. Rủi ro còn lại: bất kỳ tiến trình cục bộ nào lấy được token đều đọc được API key — chấp nhận cho tới khi Phase 3 xong.

**Việc phát sinh ngoài plan:** cắt chuỗi phản hồi theo **byte** (`&resp_text[..200]`) trong `ai_chat.rs` gây panic khi cắt giữa ký tự UTF-8 nhiều byte. Đã đổi sang cắt theo ký tự trong cùng lần sửa, vì đó chính là biểu thức đang được bọc redact.

**Điểm đặt redact ở frontend:** đặt trong `globalToast()` thay vì rải ở từng call site. Đó là nơi cuối cùng mọi thông báo đi qua, **và** nó chuyển tiếp text lỗi sang `_errorListeners` — tức là đẩy thẳng sang AI diagnostics. Redact ở đây chặn cả rò rỉ ra màn hình lẫn rò rỉ sang bên thứ ba, phủ ~114 call site bằng một chỗ sửa.

## Success Criteria

- [x] `redact()` tồn tại ở cả Rust (`src-tauri/src/redact.rs`) và TS (`src/lib/redact.ts`), có test cho 5 định dạng key phổ biến + che theo vị trí cho provider chưa biết.
- [x] Key Gemini không còn nằm trong URL — chuyển sang header `x-goog-api-key` ở cả Rust (2 chỗ) và TS (1 chỗ).
- [x] ~~`routes/kb.rs` không trả key~~ → huỷ, xem mục "Sai lệch" ở trên. Chuyển sang Phase 3.
- [x] Mọi thông báo lỗi ra UI đi qua redact (choke point `globalToast`).
- [x] Test hồi quy: 8 test Rust + 11 test TS, chạy trong CI.
- [x] Digest ảnh Docker (64 ký tự hex) **không** bị redact nhầm — có test bảo vệ.

## Risk Assessment

| Rủi ro | Giảm thiểu |
|---|---|
| Redact quá tay làm mất thông tin cần cho debug | Che giá trị, giữ tên tham số (`key=<redacted>`) |
| Bỏ sót định dạng key của provider mới | Che luôn theo vị trí (mọi query string, mọi header auth), không chỉ theo mẫu key |
| Đổi Gemini sang header làm hỏng luồng đang chạy | Test trước với key thật; nếu API không hỗ trợ thì giữ URL + che |
