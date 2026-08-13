---
phase: 3
title: Lưu crash report xuống đĩa
status: completed
priority: P2
dependencies:
  - 2
effort: ''
---

# Phase 3: Lưu crash report xuống đĩa

## Overview

`crash.rs` redact panic message rồi `eprintln!` — hết (`crash.rs:48-55`). Không có file nào được ghi. Docstring `:13-15` nói rõ việc truyền đi là để sau.

Nghĩa là "đọc crash gần nhất" ở Free P2 không phải thêm một hàm đọc, mà là **xây persistence từ đầu**.

## Bối cảnh: phần đã đúng

Hook redact đã đúng chỗ: `redacted_report()` chạy `redact()` **trước khi** chuỗi đi bất cứ đâu, và có test chứng minh key trong URL panic bị che. Giữ nguyên nguyên tắc này — file ghi ra phải là chuỗi đã redact, không bao giờ ghi bản thô rồi redact lúc đọc.

Phụ thuộc phase 2: crash message có thể mang secret dạng `KEY=VALUE`, nên nên có phủ sóng mới trước khi bắt đầu ghi xuống đĩa vĩnh viễn.

## Requirements

**Functional**
- Ghi report đã redact vào file trong thư mục app data.
- Giữ N crash gần nhất (đề xuất 10), xoay vòng.
- Đọc được crash gần nhất kèm timestamp.
- Kèm thông tin bối cảnh tối thiểu: phiên bản app, OS, arch. Không kèm gì khác — panic hook chạy trong trạng thái tiến trình đã hỏng.

**Non-functional**
- Hook panic **không được panic**. Mọi lỗi I/O nuốt im lặng; không `unwrap`, không `expect`.
- Không cấp phát lớn, không khoá async, không I/O mạng trong hook.
- Ghi phải là thao tác đồng bộ đơn giản — hook có thể chạy khi runtime tokio đã chết.

## Architecture

```
crash.rs
  ├─ redacted_report()            ← giữ nguyên
  ├─ crash_dir() -> PathBuf       ← app data dir, tạo nếu chưa có
  ├─ write_report(&str)           ← MỚI: ghi đồng bộ, nuốt lỗi
  │     file: crash-<unix_ts>.log
  ├─ rotate(keep: usize)          ← MỚI: xoá file cũ nhất khi quá N
  └─ latest_report() -> Option<CrashReport>   ← MỚI, cho Free P2
        struct CrashReport { ts, content }
```

**Vì sao `std::fs` chứ không phải tokio:** hook chạy trong ngữ cảnh panic, có thể là bất kỳ thread nào, có thể sau khi runtime đã shutdown. `std::fs::write` là thứ duy nhất chắc chắn dùng được.

**Vì sao một file mỗi crash chứ không phải append một file:** append cần đọc-sửa-ghi hoặc giữ handle; một file mỗi crash là ghi nguyên tử một lần, và xoay vòng chỉ là xoá file theo tên.

## Related Code Files

- Modify: `src-tauri/src/crash.rs`
- Kiểm tra: `src-tauri/src/path_util.rs` — đã có helper lấy app data dir chưa; nếu có thì dùng lại, đừng viết bản thứ hai

## Implementation Steps

1. Grep `path_util.rs` tìm helper thư mục app data hiện có. Dùng lại nếu có.
2. `crash_dir()`: tạo thư mục nếu thiếu; trả `Option`, không panic.
3. `write_report()`: `std::fs::write`, mọi lỗi `let _ =`.
4. Gắn vào hook: `eprintln!` **giữ nguyên** (hữu ích khi chạy từ terminal), thêm ghi file.
5. `rotate(10)`: liệt kê file khớp `crash-*.log`, sắp theo tên (timestamp trong tên nên sắp chuỗi = sắp thời gian), xoá phần dư.
6. `latest_report()`: file mới nhất, parse timestamp từ tên.
7. Test: gọi thẳng `write_report` + `latest_report` (không thực sự panic); test xoay vòng với 12 file giả.
8. Test thủ công một panic thật, xác nhận file xuất hiện và nội dung đã redact.

## Success Criteria

- [ ] Panic thật → file crash xuất hiện trong app data dir, nội dung đã redact.
- [ ] Key API trong panic message không có trong file.
- [ ] Ghi 12 crash → còn đúng 10 file, 10 cái mới nhất.
- [ ] Thư mục app data không ghi được (chmod 000) → app **không** crash thêm lần nữa, chỉ mất report.
- [ ] `latest_report()` trả đúng crash mới nhất kèm timestamp.
- [ ] Test cũ trong `crash.rs` vẫn xanh.

## Risk Assessment

| Rủi ro | Giảm thiểu |
|---|---|
| Hook panic trong lúc xử lý panic → abort xấu | Không `unwrap`/`expect`; `std::fs` đồng bộ; test kịch bản thư mục không ghi được |
| Ghi bản thô rồi định redact sau | Chỉ có một đường ghi, nhận chuỗi từ `redacted_report()`; không có API ghi bản thô |
| Crash file tích tụ vô hạn | Xoay vòng 10 file, là success criteria |
| Secret lớp mới lọt vào file vĩnh viễn | Phụ thuộc phase 2 — làm phase 2 trước |

## Đã implement — 2026-08-12

18 test trong `crash.rs`, 205 test toàn crate, warning đúng 4 baseline.

### Bước 4 của plan là sai, và nó là lỗi nghiêm trọng nhất phase này

Plan viết: *"`eprintln!` **giữ nguyên**, thêm ghi file"*. Làm đúng thế thì hỏng.

`eprintln!` **panic** khi ghi thất bại, và panic bên trong panic hook là double
panic → `SIGABRT` ngay, không unwind, không report. Reviewer tái hiện được: với
stderr là pipe đã chết (`colima-ui 2>&1 | head`), tiến trình thoát **134** và
**không file nào được ghi**.

Nghĩa là đúng kịch bản phase này tồn tại để phục vụ — người dùng không có
terminal, cần biết hôm qua app chết vì gì — lại là kịch bản mất report.

Sửa: **ghi file trước, stderr sau**, và dùng `writeln!(stderr(), …)` (trả
`Result`) thay vì `eprintln!`. Thêm `catch_unwind` bọc thân hook: xấu nhất là
mất một report, không phải biến crash thành abort.

Đo lại sau khi sửa, cùng repro:

| Kịch bản | Trước | Sau |
|---|---|---|
| stderr bình thường | exit 101, có file | exit 101, có file |
| stderr là pipe chết | **exit 134 (ABRT), không file** | có file, không abort |

### Một hồi quy tôi tự tạo ra trong lúc refactor DRY

Tách `app_data_dir()` vào `path_util.rs` (caller thứ ba xuất hiện — hai bản cũ ở
`knowledge_bank.rs` và `subscription/cache.rs`). Nhưng `env::var("HOME")` trả
`Ok("")` khi biến được set nhưng rỗng, và `PathBuf::from("").join(".colima-ui")`
là đường dẫn **tương đối** `.colima-ui`.

Hệ quả: knowledge bank và crash report rơi vào thư mục app được khởi chạy từ đó,
im lặng, mỗi lần một chỗ khác. Bản cũ dựng bằng `format!` nên ra `/.colima-ui`
(tuyệt đối, tạo thất bại, panic to rõ ràng). Sửa bằng `.filter(|h| !h.is_empty())`.
Kiểm chứng: `HOME=''` và `HOME` unset đều ra `/tmp/.colima-ui`.

### Năm sửa khác từ review

| Vấn đề | Sửa |
|---|---|
| Hai crash cùng giây ghi đè nhau — mà crash loop đúng là lúc report quan trọng nhất | Thêm số thứ tự: `crash-<ts>-<seq>.log` |
| `.max()` từ vựng trên tên chưa validate: `crash-zzz.log` thắng, che mất report thật, trả `ts: 0` | Parse timestamp **trong bộ lọc**, sắp theo `u64` đã parse. Gỡ luôn lỗi đảo thứ tự sau năm 2286 và lỗi `trim_*_matches` cắt lặp |
| File không đọc được ở vị trí mới nhất → trả `None`, báo "không có crash" cho người đang có | Duyệt từ mới nhất, lấy cái đầu tiên đọc được |
| Thư mục tên `crash-*.log` tính vào hạn mức nhưng không xoá được → kẹt vĩnh viễn ở 11 file | Lọc `is_file()` |
| Payload khổng lồ (`panic!("{:?}", huge)`) nhân bản qua 13 regex rồi ghi nguyên, ×10 file | Cắt ở 64 KiB **trước** khi redact, cắt đúng ranh giới ký tự |

Thêm: file crash đặt quyền `0600` — đã redact không có nghĩa là công khai được.

### Lệch khỏi plan, có lý do

- **`crash_dir()` trả `PathBuf` chứ không `Option`** như plan viết. Việc tạo thư
  mục xảy ra trong `write_report_in` và đã nuốt lỗi ở đó; trả `Option` chỉ đẩy
  cùng một phép kiểm tra lên một tầng.
- **Tách `handle_panic(dir, ts, …)`** khỏi closure của hook. Đóng gói
  `crash_dir()` bên trong closure khiến toàn bộ phần lắp ráp — redact, header
  phiên bản, ghi — chỉ kiểm chứng được bằng cách làm sập app thật.
- **Sửa thêm 2 file ngoài danh sách** (`knowledge_bank.rs`, `subscription/cache.rs`)
  vì refactor DRY. Cả hai có test bao phủ, 205 test xanh.

### Chưa làm

`latest_report()` chưa có caller — nó là deliverable cho Free P2 (diagnostic
bundle), phase đó chưa chạy.
