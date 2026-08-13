---
phase: 2
title: Input Validation
status: completed
priority: P1
effort: 2 ngày
dependencies: []
---

# Phase 2: Input Validation

## Overview

`src-tauri/src/validation.rs` có validator cho container id, nhưng **không có validator cho tên profile/instance Colima**, trong khi tên profile được đẩy thẳng vào argv và vào đường dẫn file.

## Bằng chứng

| Vị trí | Vấn đề |
|---|---|
| `src-tauri/src/validation.rs` | Không tồn tại `is_valid_profile_name` |
| `src-tauri/src/commands/colima.rs:93-95`, `:176-178`, `:208-210` | Tên profile được push thẳng vào argv |
| `src-tauri/src/instance_reader.rs:76` | Tên profile được join vào đường dẫn filesystem |

Hai kịch bản:
1. **Hôm nay:** tên profile bắt đầu bằng `-` được `colima` diễn giải như một cờ dòng lệnh, không phải giá trị.
2. **Sau khi P0 Phase 5 xong:** tên profile chứa `../` cho phép ghi file ngoài `~/.colima/`.

Đây là lý do plan P0 Phase 3 (tray) và Phase 5 (config) đều phải chờ phase này.

## Requirements

- `is_valid_profile_name()` trong `validation.rs`, dùng allowlist ký tự (không phải blocklist).
- Áp dụng tại **mọi** điểm nhận tên profile: Tauri command, HTTP route, tray menu event id (sắp có), deep link (sắp có).
- Đường dẫn dựng từ input người dùng phải được canonicalize và khẳng định nằm trong thư mục gốc mong đợi.
- Rà soát các input khác cùng loại: tên network, tên volume, tên compose project, namespace k8s.

## Related Code Files

- Modify: `src-tauri/src/validation.rs` — thêm `is_valid_profile_name`, `is_valid_resource_name`, `assert_path_within`
- Modify: `src-tauri/src/commands/colima.rs` — validate trước mọi lần dựng argv
- Modify: `src-tauri/src/commands/lima.rs`, `networks.rs`, `volumes.rs`, `kubernetes.rs` — rà cùng lớp vấn đề
- Modify: `src-tauri/src/instance_reader.rs` — canonicalize + assert prefix trước khi đọc/ghi
- Modify: `src-tauri/src/routes/*` — validate ở tầng HTTP, không dựa vào frontend
- Create: test cho validation

## Implementation Steps

1. Xác định bộ ký tự hợp lệ thật của tên profile Colima (đọc tài liệu/mã nguồn Colima, không đoán).
2. Viết `is_valid_profile_name` bằng allowlist; từ chối chuỗi rỗng, chuỗi bắt đầu bằng `-`, `.`, chứa `/`, `\`, null byte, khoảng trắng đầu/cuối.
3. Viết `assert_path_within(base, candidate)` — canonicalize cả hai rồi so tiền tố; dùng cho mọi đường dẫn dựng từ input.
4. Áp validation ở đầu mỗi command trong `colima.rs`; trả lỗi validation rõ ràng.
5. Rà `lima.rs`, `networks.rs`, `volumes.rs`, `kubernetes.rs` tìm cùng mẫu lỗi; sửa những chỗ tìm được.
6. Áp validation ở tầng route HTTP — **không** giả định frontend đã lọc.
7. Test với chuỗi độc: `-rf`, `../../etc/passwd`, `a\0b`, chuỗi rất dài, unicode nhìn giống dấu gạch.

## Tests / Validation

- Unit: `is_valid_profile_name` — ≥12 case xấu và ≥5 case tốt.
- Unit: `assert_path_within` chặn `..`, symlink trỏ ra ngoài, đường dẫn tuyệt đối.
- Integration: gọi HTTP route với tên profile độc → bị từ chối ở backend, không phụ thuộc frontend.
- Thủ công: tên profile hợp lệ có ký tự lạ nhưng được Colima chấp nhận vẫn hoạt động (không siết quá tay làm hỏng người dùng thật).

## Success Criteria

- [ ] `is_valid_profile_name` tồn tại và được áp ở mọi điểm nhận input.
- [ ] Không tên profile nào đi vào argv hoặc đường dẫn mà chưa qua validation.
- [ ] `assert_path_within` bảo vệ mọi thao tác file dựng từ input.
- [ ] Validation ở tầng HTTP route, không chỉ ở frontend.
- [ ] Test hồi quy chạy trong CI.

## Risk Assessment

| Rủi ro | Giảm thiểu |
|---|---|
| Siết quá tay làm hỏng profile hợp lệ đang tồn tại của người dùng | Xác định bộ ký tự từ tài liệu Colima thật; test với profile có sẵn trước khi merge |
| Bỏ sót đường vào (tray, deep link chưa tồn tại) | Ghi rõ trong plan P0/thương mại rằng mọi input mới phải qua validator này |
| Canonicalize thất bại khi file chưa tồn tại | Canonicalize thư mục cha, không phải file đích |
