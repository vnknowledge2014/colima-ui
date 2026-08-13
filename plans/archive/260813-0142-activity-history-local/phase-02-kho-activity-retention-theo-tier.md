---
phase: 2
title: "Kho activity + retention theo tier"
status: completed 2026-08-13
priority: P2
dependencies: [1]
effort: "1 ngày"
---

# Phase 2: Kho activity + retention theo tier

## Overview

`activity.db` và hàm `record()`. Chưa ai gọi nó ở phase này — phase 3 mới cắm.
Tách ra để kho và cách ghi được kiểm chứng trước khi có 30 chỗ gọi vào.

## Requirements

**Functional**
- Ghi một hành động: cái gì, lên đối tượng nào, kết quả ra sao, lúc nào.
- Retention theo tier: Free ngắn, Pro đầy đủ.
- Đọc có lọc: theo loại, theo đối tượng, theo khoảng thời gian.

**Non-functional**
- Ghi thất bại **không bao giờ** làm hỏng hành động đang chạy.
- Không giá trị bí mật nào chạm đĩa.
- Ghi phải rẻ: nằm trên đường của mọi thao tác người dùng.

## Architecture

```
commands/activity.rs
  ├─ activity.db  — kho thứ tư, vòng đời riêng (xem plan.md)
  │   activity_log(id, ts, kind, verb, target_kind, target, target_name,
  │                actor, outcome, detail, duration_ms)
  ├─ pub fn record(entry: ActivityEntry)      ← nuốt lỗi, không trả Result
  ├─ pub fn query(filter) -> Vec<ActivityEntry>
  └─ retention: chạy khi ghi, không dựng thread riêng
```

### Vì sao `record()` không trả `Result`

Người gọi nó đang ở giữa `remove_container`. Không có gì hữu ích để làm với một
lỗi ghi log, và cách duy nhất dùng sai là để nó lan lên và làm hỏng thao tác.
Lỗi ra `eprintln!`. Cùng khuôn với `security_scan.rs` khi `record_audit` lỗi.

### Các cột, và vì sao

| Cột | Vì sao có |
|---|---|
| `kind` | `destructive` \| `lifecycle` \| `task` \| `config` — lọc theo nhóm là câu hỏi đầu tiên người dùng hỏi |
| `verb` | `prune`, `remove`, `start`… — cái đã làm |
| `target_kind` + `target` | `image`/`sha256:…` — id ổn định để truy vết |
| `target_name` | `nginx:1.25` — tên người đọc được, có thể đã biến mất khỏi máy |
| `actor` | `user` \| `app` — phân biệt "tôi restart" với "self-heal restart". **Chốt 2026-08-13: có ghi hành động của app** |
| `outcome` | `ok` \| `failed` \| `denied` — **thất bại cũng phải ghi** |
| `duration_ms` | Chỉ có nghĩa với `task`; NULL cho phần còn lại |
| `detail` | Câu ngắn đã redact. Không phải argv thô |

**Không** có cột `raw_command`. Argv đầy đủ mời gọi việc lưu `-e PASSWORD=…`;
`detail` là câu tóm tắt do người gọi soạn, đi qua `redact::redact` (`src-tauri/src/redact.rs:262`) trước khi
lưu (kho cục bộ vẫn nằm trong bundle chẩn đoán người dùng gửi đi).

### Retention

| Tier | Giữ |
|---|---|
| Free | 7 ngày **hoặc** 500 bản ghi, cái nào tới trước — chốt 2026-08-13 |
| Pro | 365 ngày, trần 50 000 bản ghi |

Kiểm entitlement bằng `metrics_store::entitled_now_cached()` — cùng hàm mà
`alerts` và `security_watch` dùng.

**Hết hạn Pro không xoá dữ liệu đã có.** Retention chạy tới hạn Free sẽ cắt cụt
lịch sử người ta đã trả tiền để có. Khi rơi xuống Free: ngừng giữ thêm dài hạn,
nhưng chỉ prune tới ngưỡng Pro. Ranh giới là **tầm nhìn và xuất dữ liệu** (phase
4), không phải xoá.

Retention chạy **cơ hội** — mỗi N lần ghi, không phải thread riêng. Đây là dữ
liệu thưa; một thread nền cho việc này là phức tạp không mua được gì.

## Related Code Files

- Create: `src-tauri/src/commands/activity.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/lib.rs` (đăng ký lệnh đọc)
- Modify: `src-tauri/src/routes/` (route đọc, chế độ browser)

## Implementation Steps

1. Schema + `open()` với pragma WAL, theo khuôn `security_history.rs`.
2. `ActivityEntry` + `record()` nuốt lỗi. Test: DB không ghi được → `record`
   trả về bình thường, không panic.
3. `redact` áp lên `detail` **trong** `record`, không tin người gọi đã làm.
4. `query(filter)`: theo `kind`, `target`, khoảng `ts`, có `limit`.
5. Retention theo tier + test không xoá quá tay khi Pro hết hạn.
6. Lệnh đọc + route. Đọc **không** gate — Free vẫn phải xem được lịch sử ngắn.

## Tests

- Ghi rồi đọc lại → khớp từng trường.
- `detail` chứa `PASSWORD=hunter2` → trên đĩa không còn `hunter2`.
- Free: ghi 600 bản ghi → còn 500, cái cũ nhất bị cắt.
- Free: ghi bản ghi 8 ngày tuổi → bị cắt theo thời gian.
- Pro → Free: bản ghi 30 ngày tuổi **vẫn còn** (không cắt về ngưỡng Free).
- `record` khi thư mục DB không ghi được → không panic, không trả lỗi.
- `outcome = failed` đọc lại được, không bị lọc mất.

## Success Criteria

- [ ] `activity.db` là file riêng, không đụng `settings.db`/`metrics.db`/`security.db`.
- [ ] `record()` không trả `Result` và không panic trong mọi test lỗi.
- [ ] Không giá trị secret nào trên đĩa sau test redact.
- [ ] Retention đúng theo tier; hạ tier không xoá lịch sử cũ.

## Risk Assessment

| Rủi ro | Giảm thiểu |
|---|---|
| Ghi log chậm làm chậm thao tác người dùng | Một `INSERT` vào WAL. Đo trong test: 1000 lần ghi < 1s |
| Secret lọt vào `detail` | `redact` chạy trong `record`, không tin người gọi |
| Hạ tier xoá mất lịch sử đã trả tiền | Test tường minh; prune chỉ tới ngưỡng Pro |
| Bảng phình vô hạn nếu retention hỏng | Trần cứng theo số bản ghi, độc lập với trần thời gian |
