---
phase: 1
title: Error Contract v2
status: completed
priority: P1
effort: 6-9 ngày
dependencies: []
---

# Phase 1: Error Contract v2

## Overview

**Đây không phải phase "thêm toast".** Toast đã có: 114 call site trên 15 file. Vấn đề thật là:

1. Lỗi hiện là **chuỗi phẳng** — không có mã lỗi, không có gợi ý khắc phục, không phân biệt "chưa cài" với "chưa chạy" với "lệnh thất bại".
2. Hai mode trả lỗi **khác kiểu**: Tauri ném string, HTTP ném `Error`.
3. Lỗi hiển thị thô ra UI (`Containers.svelte:96` render `String(e)`) — vừa xấu vừa rò rỉ (đã xử lý ở plan hotfix bảo mật).

## Ràng buộc kỹ thuật quyết định thiết kế

`src-tauri/src/error.rs:4-13`:
```rust
#[derive(Debug, Serialize, Clone)]
#[serde(tag = "type", content = "message")]
pub enum ColimaError {
    CommandFailed(String), Validation(String), NotFound(String),
    Internal(String), Network(String), Unknown(String),
}
```

Đây là enum **newtype với payload String**. Thêm field vào variant sẽ biến `message` từ string thành object, và `src/lib/api/client.ts:39-43` sẽ render `[object Object]`. **Không thể sửa additively** — giả định của plan v1 sai.

Thêm nữa: **115 điểm trong `src-tauri/src/routes/*` gọi `err(e.to_string())`** (ví dụ `routes/volumes.rs:10-14`), làm phẳng lỗi ngay tại tầng HTTP. Plan v1 hoàn toàn bỏ sót `routes/`.

## Requirements

**Functional**
- Hợp đồng lỗi v2 tường minh: `{ code, title, detail, command?, exit_code?, hint?, doc_id? }`.
- Hai mode trả **cùng một shape**.
- Chuẩn hoá tại **một choke point duy nhất** — `src/lib/api/client.ts` — thay vì sửa 114 call site.
- Toast lỗi hiện `title` + `hint`; chi tiết đầy đủ mở trong panel riêng.
- `hint` liên kết được tới bài Knowledge Base (Phase 5).

**Non-functional**
- Mọi chuỗi lỗi đi qua `redact()` của plan hotfix bảo mật trước khi rời Rust.
- Không hiển thị đường dẫn tuyệt đối chứa tên người dùng khi có thể rút gọn thành `~`.
- Thay đổi không được làm vỡ browser mode — đây là chỗ plan v1 suýt hỏng.

## Architecture

```
src-tauri/src/error.rs
  ColimaErrorV2 { code: ErrorCode, title, detail, command, exit_code, hint, doc_id }
  ErrorCode: NotInstalled | NotRunning | PermissionDenied | Timeout
           | CommandFailed | Network | Validation | NotFound | Unknown
  ── enum struct-variant hoặc struct phẳng, KHÔNG dùng tag/content newtype ──
       │
       ├─ Tauri IPC  → serialize trực tiếp
       └─ routes/*   → helper err_v2(e) thay cho err(e.to_string())   ← 115 điểm
                        │
                        ▼
src/lib/api/client.ts  normalizeError()  ← CHOKE POINT DUY NHẤT
                        │  luôn trả AppError cùng shape cho cả 2 mode
                        ▼
src/lib/errorReporter.ts  reportError(err, ctx)
                        ▼
globalToast (đã có) + ErrorDetailPanel (mới)
```

**Quyết định:** không giữ tương thích ngược cho `ColimaError` v1 — app chưa có nhiều người dùng, và giữ 2 hợp đồng song song tốn hơn là đổi dứt điểm. Xem Unresolved Question #1 ở plan.md.

## Related Code Files

- Modify: `src-tauri/src/error.rs` — định nghĩa v2
- Modify: `src-tauri/src/helpers.rs` — `error_from_output(cmd_desc, output)` map stderr → code + hint
- Modify: `src-tauri/src/routes/*.rs` — **115 điểm** đổi `err(e.to_string())` → `err_v2(e)`
- Modify: `src-tauri/src/commands/{containers,colima,compose,volumes,networks,kubernetes,lima}.rs`
- Modify: `src/lib/api/client.ts` — `normalizeError()` (choke point)
- Create: `src/lib/errorReporter.ts` + test
- Create: `src/store/errorLog.svelte.ts`, `src/components/ErrorDetailPanel.svelte`
- Modify: `src/lib/globalToast.ts` — action button + gộp trùng + giới hạn 3
- Modify: `src/pages/Containers.svelte` — bỏ `String(e)` tại `:96` và các chỗ tương tự
- Modify: `src/locales/*` (4 locale)

## Implementation Steps

1. Chốt shape v2 và ghi vào `docs/architecture.md` trước khi code.
2. Định nghĩa `ColimaErrorV2` trong `error.rs`. Chạy `cargo check` — trình biên dịch sẽ chỉ ra toàn bộ điểm gãy.
3. `helpers::error_from_output` map ≥6 mẫu stderr thật sang `code` + `hint` (ví dụ `Cannot connect to the Docker daemon` → `NotRunning` + "Colima chưa chạy — bấm Start").
4. Sửa `commands/containers.rs` trước làm mẫu (773 dòng, nhiều lệnh nhất), rồi 6 module còn lại.
5. **Sửa `routes/*`** — 115 điểm. Cơ học nhưng bắt buộc; đây là thứ làm browser mode hoạt động.
6. `normalizeError()` ở `client.ts`: nhận cả lỗi Tauri (string/object) lẫn lỗi HTTP, trả `AppError` đồng nhất.
7. `errorReporter.ts` với `reportError` và `withErrorReport`; nâng cấp `globalToast.ts`.
8. `ErrorDetailPanel.svelte` + `errorLog.svelte.ts` — lịch sử lỗi trong phiên, copy được (đã redact).
9. Rà 114 call site toast hiện có: chỗ nào truyền chuỗi thô thì đổi sang `AppError`.
10. i18n 4 locale.

## Tests / Validation

- Rust unit: `error_from_output` map đúng ≥6 mẫu stderr.
- Rust unit: serialize v2 ổn định (snapshot test) — chống hồi quy khi thêm variant.
- TS unit: `normalizeError` với input từ **cả 2 mode** — đây là test quan trọng nhất phase này.
- TS unit: `errorReporter` gộp trùng, giới hạn 3 toast.
- Thủ công: tắt Colima → start/stop/remove container ở **cả desktop và browser mode** → toast giống hệt nhau.
- Thủ công: xác nhận không còn chỗ nào render `[object Object]` hay JSON thô.

## Phát hiện lúc thực thi làm thay đổi cách làm (rẻ hơn plan nhiều)

**108/108 điểm dựng `ColimaError` đều là `ColimaError::from(String)`.** Enum typed cũ gần như chỉ để trang trí — mọi lỗi thực tế rơi vào `Unknown`. Hệ quả: đặt phần **phân loại vào `From<String>`** thì toàn bộ 108 điểm ở `commands/` tự động có lỗi phân loại mà không sửa dòng nào.

**`err()` đổi thành `err(impl Into<ColimaError>)`** → **115 điểm `err(e.to_string())` biên dịch nguyên vẹn**, vẫn có lỗi phân loại đầy đủ. Plan tính 115 sửa cơ học; thực tế là **1 sửa**. Phần "cơ học nhưng bắt buộc" ở bước 5 gần như biến mất.

## Sai lệch so với plan

| Plan nói | Thực tế làm | Lý do |
|---|---|---|
| `ColimaErrorV2` có field `title` | **Không có `title` ở Rust** | Rust phát `code` ổn định + `detail` kỹ thuật; wording do i18n quyết định. Rust không nên chọn ngôn ngữ cho người dùng. Giữ `hint` tiếng Anh làm fallback cho CLI/log |
| Đổi tên thành `ColimaErrorV2` | Giữ tên `ColimaError`, đổi *hình dạng* | v2 là hợp đồng, không phải tên; đổi tên tạo churn vô ích ở 130 điểm |
| Sửa 114 call site toast sang `AppError` | **Cầu tương thích**: `AppErrorException.toString()` | 114 site vẫn `String(e)` và giờ hiển thị *tốt hơn* trước, không cần quét. Chỉ chuyển 3 site ở `Containers.svelte` sang `reportError` làm mẫu. Đây là điểm chịu tải — có test bảo vệ |

## Việc còn lại của phase (chưa làm, có chủ đích)

- Chuyển nốt ~111 call site còn lại sang `reportError` để có hint + nút Chi tiết. Không bắt buộc: cầu tương thích khiến chúng vẫn đúng và đã redact. Nên làm dần theo trang khi động vào.

## Success Criteria

- [x] `ColimaError` có `code`, `detail`, `command`, `exit_code`, `hint`, `doc_id` (`title` chuyển sang i18n — xem Sai lệch).
- [x] 115 điểm trong `routes/*` trả lỗi có cấu trúc — đạt được bằng đổi chữ ký `err()`, không phải sửa 115 chỗ.
- [x] `normalizeError()` là choke point duy nhất; **có test khẳng định 2 mode cho kết quả giống hệt**.
- [x] Toast hiện title + detail + hint; panel chi tiết mở được, nội dung đã redact, copy được.
- [x] Không chỗ nào render lỗi thô — kể cả `TerminalInstance.svelte` (chỗ đọc `data.error` trực tiếp, do review bắt được).
- [x] 6 nhóm lỗi phổ biến có `hint` + `doc_id`, dịch đủ 4 locale.
- [x] Test: 33 Rust (+6), 96 TS (+17). `tsc` giữ nguyên baseline 137, 0 lỗi mới.

## Code Review — 8 finding, đã xử lý

| # | Finding | Sev | Xử lý |
|---|---|---|---|
| 1 | `TerminalInstance.svelte:85` đọc thẳng `data.error` → `[object Object]` ở browser mode | Critical | Đã sửa dùng `toAppException` |
| 2 | Đuổi toast khỏi `_active` nhưng UI vẫn hiện → repeat tạo bản sao thứ hai | High | Đã sửa: thêm `onToastRemove`, `dismissToast` phát sự kiện |
| 3 | Toast phát ra khi `ToastContainer` chưa mount → rò rỉ `_active`, message đó **bị chặn vĩnh viễn** cả phiên | High | Đã sửa: `globalToast` tự sở hữu timer, không phụ thuộc component |
| 4 | `reportError` chỉ hiện title, mất stderr — thụt lùi so với `String(e)`, và làm AI diagnostics kém đi | High | Đã sửa: dùng `errorMessage` (title + detail) |
| 5 | `classify()` khớp quá rộng: `"timeout"`, `"not found"`, `"certificate"` | Medium | Đã siết + thêm 2 test case âm |
| 6 | Escape không đóng panel (handler trên backdrop không bao giờ được focus) | Medium | Đã chuyển sang `<svelte:window>` |
| 7 | `resolveApiBase` là scope creep từ plan hotfix | Medium | Ghi nhận — đã nằm trong working tree từ plan trước |
| 8 | `AIPanelSettings.svelte:95`, `Models.svelte:176` so khớp chuỗi lỗi → hỏng vì text giờ đã localize | Medium | Đã sửa: rẽ nhánh theo `code` |

## Risk Assessment

| Rủi ro | Mức | Giảm thiểu |
|---|---|---|
| **Đổi `ColimaError` phá browser mode** — chính xác điều plan v1 định làm | Cao | Sửa `routes/*` nằm trong phạm vi bắt buộc; test cả 2 mode là điều kiện merge |
| 115 + 114 điểm sửa quá lớn cho một PR | Cao | Chia: (a) định nghĩa v2 + helpers, (b) commands, (c) routes, (d) frontend. Mỗi phần một PR |
| Lỗi chưa redact lọt vào panel chi tiết | Cao | Plan hotfix bảo mật chặn phase này; redact ở tầng Rust, không ở UI |
| Ước lượng vẫn lạc quan | TB | 6-9 ngày đã tính 4 locale i18n và 2 nhóm sửa cơ học; theo dõi sau PR (a) rồi hiệu chỉnh |
