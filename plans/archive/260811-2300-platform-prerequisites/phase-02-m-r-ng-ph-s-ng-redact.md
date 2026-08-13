---
phase: 2
title: Mở rộng phủ sóng redact
status: completed
priority: P1
dependencies: []
effort: ''
---

# Phase 2: Mở rộng phủ sóng redact

## Overview

`redact.rs` hiện phủ tốt **secret trong URL và header** — đúng mối đe doạ nó được viết ra để chặn (chuỗi lỗi `reqwest` mang key trong query param). Nó **không** phủ mối đe doạ mới mà diagnostic bundle mang lại: **log container**, nơi secret xuất hiện dưới dạng `KEY=VALUE` chứ không phải `?key=`.

## Bối cảnh: phần đã đúng, và một quyết định phải giữ

Module hiện tại có hai luật: redact theo **vị trí** (query param / header) và theo **hình dạng** (`sk-`, `AIza`, `gsk_`, `hf_`, `gh[pousr]_`, `colima-<64hex>`).

Nó **cố ý từ chối** khớp entropy tổng quát, có lý do được ghi rõ: một pattern 64-hex sẽ nuốt luôn digest image Docker, làm hỏng chẩn đoán cho mọi người để bảo vệ vài provider. Có test `preserves_docker_image_digests` bảo vệ quyết định này.

**Quyết định đó vẫn đúng và phải giữ.** Phase này thêm pattern **cụ thể**, không nới lỏng thành entropy matching.

## Requirements

Bốn lớp cần bổ sung:

| Lớp | Ví dụ lọt hiện nay | Nguồn |
|---|---|---|
| `KEY=VALUE` kiểu env | `POSTGRES_PASSWORD=hunter2` | log container, `docker inspect`, compose file |
| AWS access key | `AKIAIOSFODNN7EXAMPLE` + secret key đi kèm | log ứng dụng |
| JWT | `eyJhbGciOi...` ba đoạn base64 ngăn bởi dấu chấm | log auth |
| PEM private key | `-----BEGIN ... PRIVATE KEY-----` khối nhiều dòng | mount nhầm, log |

Cộng thêm: **đường dẫn home** `/Users/<tên>` → `/Users/<user>`. Không phải secret, nhưng là dữ liệu cá nhân đi vào issue GitHub công khai.

**Non-functional**
- Không được có false positive làm hỏng chẩn đoán. Digest, tag image, tên container, ID phải sống sót.
- `redact()` chạy trên mọi dòng log của bundle — độ phức tạp phải tuyến tính, không backtracking.

## Architecture

Thêm vào `redact.rs`, giữ nguyên cấu trúc hai luật:

```rust
// Luật 1 mở rộng (theo vị trí): KEY=VALUE khi KEY chứa từ nhạy cảm.
// Danh sách từ khoá đóng, KHÔNG phải heuristic — cùng tinh thần QUERY_PARAM.
static ENV_ASSIGNMENT: ... =
  r"(?im)^(\s*(?:export\s+)?[A-Z0-9_]*(?:PASSWORD|PASSWD|SECRET|TOKEN|APIKEY|API_KEY|
     PRIVATE_KEY|ACCESS_KEY|CREDENTIAL)[A-Z0-9_]*\s*[=:]\s*)\S+"

// Luật 2 mở rộng (theo hình dạng): thêm 3 pattern đặc trưng.
AKIA[0-9A-Z]{16}
eyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}
(?s)-----BEGIN [A-Z ]*PRIVATE KEY-----.*?-----END [A-Z ]*PRIVATE KEY-----

// Riêng: đường dẫn home
/Users/[^/\s:]+   →  /Users/<user>
```

**Điểm cần cẩn thận với `ENV_ASSIGNMENT`:** neo `^` theo dòng (`(?m)`) để không nuốt giữa câu. Tên biến phải chứa từ khoá nhạy cảm — `DATABASE_URL=postgres://user:pw@host` **không** khớp và cần một pattern riêng cho URL có credential (`://user:pass@`).

**AWS secret key** (40 ký tự base64) cố ý **không** thêm: nó không có tiền tố đặc trưng và sẽ khớp nhầm nhiều thứ. Bắt `AKIA...` là đủ để cảnh báo; giá trị đi kèm thường nằm trong một `KEY=VALUE` đã bị luật 1 bắt.

## Related Code Files

- Modify: `src-tauri/src/redact.rs` (pattern mới + test)
- Không sửa file nào khác — mọi caller đã đi qua `redact()`

## Implementation Steps

1. Viết test **trước**, cho từng lớp trong bảng requirements, kèm test âm tính cho từng false positive đáng lo.
2. Thêm `ENV_ASSIGNMENT` + `URL_CREDENTIAL` vào luật 1.
3. Thêm 3 pattern vào `KEY_SHAPES`.
4. Thêm bước thay `/Users/<tên>`, đặt **sau** các luật khác (thứ tự quan trọng: đừng để nó cắt path bên trong một chuỗi đã redact).
5. Chạy toàn bộ test hiện có — `preserves_docker_image_digests` và `leaves_ordinary_errors_untouched` phải vẫn xanh.
6. Kiểm thử thực tế: chạy `docker logs` trên một container postgres có `POSTGRES_PASSWORD`, đưa qua `redact()`, khẳng định không lọt.

## Success Criteria

- [ ] `POSTGRES_PASSWORD=hunter2` trong log container: bị che.
- [ ] `postgres://user:pw@host/db`: phần credential bị che, host còn đọc được.
- [ ] `AKIA...`, JWT, khối PEM: bị che.
- [ ] `/Users/longnd/...` → `/Users/<user>/...`.
- [ ] Digest `sha256:...` **không** bị đụng — test cũ vẫn xanh.
- [ ] Tên container, tag image, ID: không bị đụng.
- [ ] Toàn bộ test cũ trong `redact.rs` xanh, không sửa test nào để cho qua.

## Risk Assessment

| Rủi ro | Giảm thiểu |
|---|---|
| **False positive làm hỏng chẩn đoán** — chính rủi ro module đã tránh có chủ ý | Pattern đặc trưng, danh sách từ khoá đóng, không entropy matching; test âm tính cho digest/tag/id |
| Regex backtracking làm chậm bundle lớn | `regex_lite` không có backtracking; tránh `.*?` lồng ngoài trường hợp PEM đã giới hạn |
| Vẫn còn lớp secret chưa nghĩ tới | Đây là lý do bundle ở Free P2 cho người dùng **xem toàn bộ trước khi gửi** — redact là lớp phòng thủ, không phải lời bảo đảm |
| Che quá tay khiến bug report vô dụng | Giữ tên biến/param nhìn thấy được, chỉ che giá trị — nguyên tắc `MASK` hiện có |

## Đã implement — 2026-08-12

Thiết kế ở mục Architecture bên trên **đã lỗi thời**. Một luật `ENV_ASSIGNMENT`
duy nhất không sống nổi qua review; nó tách thành năm, và thứ tự áp dụng trở
thành thứ quan trọng nhất.

### Sáu chỗ lệch so với plan, và vì sao

| Plan viết | Thực tế | Lý do |
|---|---|---|
| Một `ENV_ASSIGNMENT`, neo `^` theo dòng | Năm luật: `ASSIGN_EQ`, `ASSIGN_FLAG`, `ASSIGN_JSON`, `ASSIGN_YAML_UPPER`, `ASSIGN_YAML_LOWER` | `docker inspect` phát env trong mảng JSON giữa dòng → neo `^` bỏ sót. Và một luật case-blind cho dấu `:` ăn mất chẩn đoán trong văn xuôi (`Error saving credentials: …`, `services.db.secrets: must be a list`) |
| PEM nằm trong `KEY_SHAPES` (chạy sau) | `PEM_BLOCK` tách riêng, **chạy đầu tiên** | PEM chứa khoảng trắng (`BEGIN RSA PRIVATE KEY`). Luật gán chạy trước sẽ cắt cụt ở khoảng trắng đó, mất `-----BEGIN`, khiến pattern PEM không khớp nữa và **toàn bộ thân khoá lọt nguyên**. Over-redaction gây under-redaction ngay luật sau |
| `\s*` quanh dấu phân cách | `[ \t]` | `\s` vắt qua xuống dòng: `secret:` cuối dòng nuốt token đầu dòng kế |
| Giá trị `\S+` không giới hạn | `[^\s,;"'<>&})\]]{1,512}` | Thiếu `<>` thì luật khớp lại chính `<redacted>` rồi xoá mọi thứ phía sau. Thiếu `&` thì một tham số query nuốt tham số kế. Không chặn độ dài thì một `PASSWORD=` nuốt cả megabyte bundle |
| `URL_CREDENTIAL` userinfo `+` | `*` | `redis://:password@host` (username rỗng) là dạng chuẩn của Redis, `+` bỏ sót hoàn toàn |
| "Không sửa file nào khác" | Có sửa `src/lib/redact.ts` | Có **hai** module redact; `crashReporter.ts` dùng bản frontend, và stack trace JS chứa đường dẫn tuyệt đối. Không đồng bộ thì tên tài khoản lọt qua đúng một trong hai đường crash |

### Ngoài phạm vi plan, đã thêm

- `ASSIGN_FLAG` cho cờ CLI cách nhau bằng khoảng trắng (`redis-server --requirepass …`). Tiền tố `--` là thứ khiến dấu cách an toàn ở đây.
- Từ khoá bổ sung: `PASSPHRASE`, `REQUIREPASS`, `SALT`, `DSN`, và `_KEY` (bắt `SUPABASE_SERVICE_ROLE_KEY`, `SUPABASE_ANON_KEY`; dấu gạch dưới giữ nó khỏi khớp `MONKEY`).
- `HOME_PATH` phủ cả `/home/` — app điều khiển một VM Linux, đường dẫn guest xuất hiện trong log. Trừ `Shared`/`Guest` vì đó không phải tên tài khoản.

### Kiểm chứng

- `redact.rs`: 40 test (11 mới cho các lớp secret, 10 mới khoá đúng từng finding của review, phần còn lại có sẵn). Toàn bộ xanh.
- `redact.test.ts`: 16 test xanh (chạy `--environment node`; môi trường jsdom mặc định hỏng sẵn do jsdom 30 + Node 20, không liên quan thay đổi này).
- `cargo test --lib`: 181 pass, 0 fail. `cargo check`: 4 warning = baseline. `pnpm check`: 143 errors/36 warnings = baseline.

### Chưa làm

**Bước 6 (kiểm chứng thực tế trên `docker logs` của container postgres thật) chưa
chạy** — Docker/colima không chạy trên máy lúc implement. Toàn bộ success criteria
đạt bằng test, nhưng bước này tồn tại để bắt định dạng log ngoài dự đoán, nên nó
vẫn là khoảng trống thật. Chạy khi có engine.
