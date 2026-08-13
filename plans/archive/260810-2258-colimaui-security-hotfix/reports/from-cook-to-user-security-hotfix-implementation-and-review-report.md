# Hotfix bảo mật — báo cáo thực thi

- Ngày: 2026-08-11
- Plan: `plans/260810-2258-colimaui-security-hotfix/` — 3/3 phase completed
- Nhánh: `dev` — **chưa commit, chưa push** (theo yêu cầu)

## Kết quả kiểm thử

| Kiểm tra | Kết quả |
|---|---|
| `cargo test --lib` | **27/27 pass** (0 trước đó — chưa có test Rust nào) |
| `vitest src/lib/redact.test.ts` | **13/13 pass** |
| `cargo check --all-targets` | 0 lỗi |
| `tsc --noEmit` | 137 lỗi, **toàn bộ có sẵn từ trước**; 0 lỗi ở file đã sửa |

**Lưu ý môi trường:** `vitest` với jsdom hỏng trên máy này (Node 20.19 + jsdom30/undici8 cần Node ≥22) — hỏng cả với test tôi không đụng, tức là có sẵn. CI dùng Node 22. Test redact chạy bằng `--environment node`.

## Đã sửa

### Phase 1 — Secret Redaction
- `src-tauri/src/redact.rs` + `src/lib/redact.ts` (mới): che theo **vị trí** (query param/header credential — phủ cả provider chưa biết) và theo **hình dạng** (5 định dạng key + token của app). Digest Docker được giữ nguyên có chủ đích.
- Key Gemini rời khỏi URL → header `x-goog-api-key` (2 chỗ Rust + 1 chỗ TS).
- Toàn bộ đường lỗi trong `ai_chat.rs` bọc `redact_err`.
- **Choke point `globalToast()`** — phủ ~114 call site bằng một chỗ sửa, và chặn luôn đường lỗi tự động đẩy sang AI diagnostics.
- Sửa kèm: cắt chuỗi phản hồi theo byte gây panic với UTF-8 → cắt theo ký tự.

### Phase 2 — Input Validation
- `is_valid_profile_name`, `is_valid_resource_name`, `ensure_valid_profile`, `assert_path_within`.
- 20 guard ở entry point của `colima.rs`, `lima.rs`, `networks.rs`, `volumes.rs`, `routes/k8s.rs`.
- `instance_reader.rs`: chặn path traversal + containment check, **log cảnh báo** thay vì âm thầm bỏ qua.
- Charset chọn theo *an toàn*, không ép RFC 1123 — Colima chấp nhận underscore (issue abiosoft/colima#745), siết chặt sẽ phá profile hiện có.

### Phase 3 — Auth Hardening
- `auth.rs` viết lại: allowlist `QUERY_TOKEN_PATHS` thay cho `ends_with("/stream")`, parse query param thay substring, so khớp thời gian hằng số, tách `is_authorized` để test được (9 test).
- `resolveApiBase()` dò port 11420-11429 (khớp dải server thật); xoá bản hardcode trùng ở `Terminal.svelte`, sửa cả URL SSE ở `dataPoller.ts` và `api/k8s.ts`.

## Sai lệch so với plan (3 mục, đều đã được chốt)

| Mục plan | Vì sao huỷ |
|---|---|
| Bỏ `ai_api_key` khỏi `get_all_settings` | Frontend gọi LLM trực tiếp nên **cần** key — bỏ đi là AI chat chết. Gốc vấn đề nằm ở `/api/auth/token` |
| Siết `/api/auth/token` | Browser mode bắt buộc có bootstrap không xác thực; mọi cơ chế chặn tiến trình khác cũng chặn trình duyệt |
| Thu hẹp `https://*` | Chính nó khiến **6/9 provider** chạy được; Tauri capabilities là compile-time, không mở rộng lúc chạy |

Cả 3 đã ghi vào `docs/architecture.md` → "Accepted risks", kèm hướng sửa thật.

## Code review — 7 finding, đã xử lý

| # | Finding | Xử lý |
|---|---|---|
| 2 | URL SSE vẫn hardcode 11420 ở `dataPoller.ts`, `api/k8s.ts` → browser mode đứng im không báo lỗi | **Đã sửa** |
| 3 | `Authorization: Basic ...` chỉ che chữ "Basic", lộ credential | **Đã sửa** + test |
| 5 | Guard gắn nhầm vào `lima_info` (param không dùng) → chặn nhầm caller hợp lệ | **Đã gỡ** |
| 7 | `resolveApiBase` cache vĩnh viễn kết quả thất bại; dò tuần tự 10 port | **Đã sửa** — không cache khi thất bại, dò song song, timeout 1.5s |
| 1 | `assert_path_within` là code chết | **Đã dùng** ở `instance_reader.rs` |
| 4 | `read_instance` âm thầm giấu profile tên lạ | **Đã log cảnh báo** |
| 6 | 8 file bị xoá ngoài phạm vi | **Không phải do tôi** — xem mục dưới |

Reviewer xác nhận: 20/20 guard đúng scope/đúng kiểu lỗi; `x-goog-api-key` đúng ở cả 3 điểm; `QUERY_TOKEN_PATHS` phủ đủ route SSE thật; `is_valid_resource_name` không chặn nhầm tên Docker hợp lệ (kể cả volume ẩn danh 64-hex); redact không phá digest/tag/output kubectl.

## Cần bạn xử lý

**8 file bị xoá trong working tree mà không thuộc phạm vi hotfix:** `bug_background.mov`, `bug_background.mp4`, `fix_errors.py`, `refactor_closures{,2,3}.py`, `refactor_errors{,2}.py`.

Không phải do phiên làm việc này — lệnh `rm` duy nhất tôi chạy là xoá 2 thư mục plan cũ. Đầu phiên `git status` chỉ có `?? pnpm-lock.yaml`. Trông giống dọn dẹp có chủ đích. **Chưa động vào, chưa commit.** Bạn quyết định giữ hay `git restore` trước khi commit hotfix.

## Câu hỏi chưa giải quyết

1. Phát hành 0.1.11 ngay, hay gộp vào bản kế tiếp?
2. Có cần khuyến cáo người dùng hiện tại xoay API key AI không? (tuỳ vào việc key đã từng lọt vào issue công khai chưa)
3. 8 file bị xoá ở trên — giữ hay khôi phục?
4. `docs/architecture.md` "Accepted risks" nêu 2 hướng sửa thật (đẩy LLM call qua Rust; desktop app truyền token cho browser) — có lên plan riêng không?
