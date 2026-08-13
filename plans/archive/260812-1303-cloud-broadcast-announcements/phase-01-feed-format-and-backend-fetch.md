---
phase: 1
title: Feed format and backend fetch
status: completed
priority: P3
dependencies: []
effort: M
---

# Phase 1: Feed format and backend fetch

## Overview

Định nghĩa file feed và đường lấy nó. Fetch nằm ở Rust, frontend gọi API của mình —
CSP không phải nới, và chỉ **một** URL được phép thay vì `https://*`.

## Feed sống ở đâu

`announcements.json` ở gốc repo, nhánh `main`, đọc qua
`https://raw.githubusercontent.com/vnknowledge2014/colima-ui/main/announcements.json`.

Xuất bản là commit + push; duyệt nội dung là code review — không cần thêm quy trình
nào. Đánh đổi đã biết: `raw.githubusercontent.com` cache khoảng 5 phút và không cam
kết SLA. Với nhịp poll 6 giờ thì cả hai đều không đáng kể.

## Định dạng

```jsonc
{
  // Bump khi hình dạng đổi. Client chỉ đọc phiên bản nó hiểu và bỏ qua phần còn
  // lại — một app cũ gặp feed mới phải im lặng, không phải vỡ.
  "version": 1,
  "announcements": [
    {
      "id": "2026-08-security-advisory-tar",   // ổn định, do người đặt
      "publishedAt": "2026-08-12T00:00:00Z",
      "expiresAt": null,
      "severity": "info",                       // info | warning | critical
      "audience": null,                         // null | "free" | "pro"
      "minVersion": null,                       // semver, bao gồm
      "maxVersion": null,
      "title": { "en": "…", "vi": "…", "ja": "…", "zh": "…" },
      "body":  { "en": "…" },                   // tuỳ chọn; plain text
      "linkUrl": "https://github.com/vnknowledge2014/colima-ui/releases/tag/v0.2.0"
    }
  ]
}
```

**`id` do người đặt, không phải UUID.** Red team chỉ ra watermark trên
`gen_random_uuid()` là vô nghĩa vì UUID không có thứ tự; ở đây client lưu **tập id
đã đọc**, nên thứ tự không cần — nhưng một id đọc được vẫn giúp khi debug.

**`title`/`body` là `jsonb` đa ngôn ngữ** vì app có 4 locale. Một cột text sẽ ép
tiếng Anh cho tất cả.

## Fetch ở Rust

```rust
#[tauri::command]
pub async fn announcements_fetch() -> Result<AnnouncementFeed, ColimaError>
// + GET /api/announcements
```

Ràng buộc, mỗi cái vì một lý do:

- **URL là hằng số**, không nhận tham số. Không có cách nào để frontend biến lệnh
  này thành SSRF.
- **Timeout ngắn** (~10s) và **giới hạn body** (~256 KB). Một feed hỏng hoặc một
  MITM không được phép treo app hay ăn hết RAM.
- **Không gửi gì kèm**: không user id, không machine id, không cookie, không header
  tuỳ biến. Chỉ một GET trần.
- **Lỗi là `Ok(feed rỗng)` hay `Err`?** → `Err`, và frontend nuốt nó. Trộn "không có
  announcement" với "không lấy được" ở tầng này sẽ khiến tầng trên không phân biệt
  được, mà Phase 2 cần phân biệt để không xoá nội dung đã hiển thị.
- **Không cache ra đĩa.** Feed nhỏ và poll thưa; một cache là thêm trạng thái để sai.

## Related Code Files

- Create: `announcements.json` — feed khởi tạo, mảng rỗng
- Create: `src-tauri/src/commands/announcements.rs`
- Create: `src-tauri/src/routes/announcements.rs`
- Modify: `src-tauri/src/commands/mod.rs`, `src-tauri/src/routes/mod.rs`
- Modify: `src-tauri/src/lib.rs` — đăng ký command
- Modify: `src-tauri/src/api_server.rs` — `GET /api/announcements`
- Modify: `docs/api.md` — endpoint mới

## Implementation Steps

1. `announcements.json` với `{"version": 1, "announcements": []}` — feed rỗng hợp lệ
   là trạng thái mặc định đúng, và cho phép test đường đi trước khi có nội dung.
2. Struct `AnnouncementFeed` / `Announcement` với `serde`, `deny_unknown_fields`
   **tắt** — feed mới thêm trường không được làm vỡ app cũ.
3. Command + route; dùng `reqwest` đã có qua `tauri-plugin-http` hoặc dependency sẵn.
4. Timeout + giới hạn kích thước + từ chối `version` không hiểu.
5. Test: feed hợp lệ parse đúng; feed có trường lạ vẫn parse; `version: 99` bị từ
   chối rõ ràng; body quá lớn bị cắt.

## Success Criteria

- [x] `GET /api/announcements` trả feed; đi qua token auth như mọi route khác.
- [x] Feed có trường chưa biết → vẫn parse (app cũ không vỡ vì feed mới).
- [x] `version` lớn hơn client hiểu → từ chối, không đoán.
- [x] Host không tới được → `Err` rõ ràng, không phải feed rỗng giả.
- [x] Body vượt giới hạn → từ chối, không nạp hết vào RAM. Kiểm **hai lần**:
      `content-length` trước khi tải, rồi độ dài thật sau — header là một lời khai,
      không phải bảo đảm.
- [x] Request không mang user id / machine id / cookie. Client dựng không cookie
      store: một jar là cách dễ nhất để điều đó thôi đúng.
- [x] `cargo test` 310/310, clippy `-D warnings` sạch.

## Kết quả

6 test mới cho `parse_feed`, tất cả xanh. Hai test đáng chú ý vì chúng khoá lại hai
hướng ngược nhau:

- **Trường lạ → vẫn parse.** Nếu từ chối, một lần sửa feed có thể làm câm mọi
  announcement với người chưa cập nhật — kể cả cái advisory bảo họ cập nhật.
- **`version` lạ → từ chối.** Đó là tín hiệu hình dạng đã đổi; đoán còn tệ hơn không
  hiện gì.

`severity` để kiểu `String` chứ không phải enum, vì lý do thứ nhất: một từ vựng mới
phải tới được client (nơi hạ cấp thứ nó không nhận ra) thay vì làm hỏng cả tài liệu
ở tầng parse.

### Trạng thái thật của feed

`https://raw.githubusercontent.com/.../main/announcements.json` hiện trả **404** vì
`announcements.json` mới chỉ nằm ở working tree, chưa push lên `main`. Đã kiểm:
`package.json` trên cùng nhánh trả 200, nên URL đúng hình dạng và repo tới được.

Đây là đường lỗi đã thiết kế, không phải sự cố: 404 → `Err` → client giữ nguyên nội
dung đang hiển thị. Feed sẽ sống ngay khi file lên `main`.

## Risk Assessment

- **Feed bị chiếm** (repo bị compromise) → nội dung độc hại tới mọi client. Giảm
  nhẹ nằm ở Phase 3: allowlist `linkUrl`, plain text bắt buộc, giới hạn độ dài.
  Ghi rõ ở đây vì đó là mô hình đe doạ của **phase này** dù biện pháp ở phase sau.
- **`raw.githubusercontent.com` chết** → không có announcement, app vẫn chạy đủ.
  Chấp nhận được với P3.
