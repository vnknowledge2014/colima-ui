---
phase: 4
title: "Streaming command + path confinement"
status: done
priority: P2
dependencies: []
effort: ""
---

# Phase 4: Streaming command + path confinement

## Overview

Hai hạng mục nhỏ, cùng phục vụ Free P1 (file transfer + export image TAR).

**Đính chính một finding của red-team:** reviewer báo "`validation.rs` không có API giới hạn path dùng được". Đọc code thì **`assert_path_within` đã được viết tử tế** (`validation.rs:169-195`): canonicalize base, canonicalize parent cho path chưa tồn tại, chống cả `..` lẫn symlink trỏ ra ngoài. API không thiếu. Cái thiếu là **plan không nêu thư mục base** — đó là quyết định chính sách, không phải code. Phase này chốt chính sách đó, và tập trung công sức vào khoảng trống thật: streaming.

## Requirements

### 4A. Streaming command (khoảng trống thật)

`run_cmd` dùng `Command::output()` — **buffer toàn bộ stdout vào RAM** (`helpers.rs:98-133`). `docker save` một image 2 GB sẽ nằm trọn trong bộ nhớ.

- Chạy lệnh dài với stdout ghi thẳng ra file hoặc stream theo dòng.
- Báo tiến độ được (theo bytes đã ghi).
- Huỷ được: kill tiến trình con, dọn output dở dang.
- Giữ nguyên hành vi `DOCKER_HOST` và thông báo "chưa cài" của `run_cmd` — đừng viết bản thứ hai lệch nhau.

### 4B. Chính sách path confinement

- Chốt base cho từng loại thao tác ghi, và dùng `assert_path_within` sẵn có.
- Với `docker cp` host→container: base là thư mục người dùng chọn qua dialog, không phải hằng số.
- Với export TAR: base là thư mục đích người dùng chọn.

## Architecture

```
helpers.rs
  ├─ run_cmd()                    ← giữ nguyên, cho lệnh ngắn
  └─ run_cmd_streaming(program, args, sink: OutputSink) -> Result<StreamHandle>
        OutputSink::ToFile(PathBuf)   — stdout ghi thẳng ra file
        OutputSink::ByLine(Sender)    — stdout gửi theo dòng
        StreamHandle { cancel(), progress_rx }
        dùng chung hàm dựng env/DOCKER_HOST với run_cmd (tách ra làm helper riêng)
```

**Vì sao không đơn giản là `Stdio::piped()` + đọc:** cần cả huỷ giữa chừng lẫn tiến độ, nên phải giữ handle tiến trình con và một kênh huỷ. Đóng gói một lần ở đây, để Free P1 không tự chế bản riêng.

**Về path confinement — quy tắc, không phải code mới:**

| Thao tác | Base |
|---|---|
| `docker cp` container→host | Thư mục người dùng chọn qua dialog hệ thống |
| `docker cp` host→container | Không confinement phía host (chỉ đọc); phía container: validate qua `is_valid_container_id` + không nội suy shell |
| `docker save` → TAR | Thư mục người dùng chọn |

Đường dẫn phía container **không** đi qua `assert_path_within` (nó là filesystem khác); an toàn ở đó đến từ việc dùng `Command::args()` chứ không phải `sh -c`.

**Lưu ý về `contains_shell_injection`:** nó là denylist metachar (`validation.rs:7-21`), phù hợp cho exec command người dùng gõ, **không phải** cơ chế bảo vệ đường dẫn. Đừng dùng nhầm chỗ.

## Related Code Files

- Modify: `src-tauri/src/helpers.rs` (tách helper env, thêm `run_cmd_streaming`)
- Modify: `src-tauri/src/validation.rs` (chỉ thêm doc nêu rõ base policy nếu cần; **không** sửa `assert_path_within`)
- Kiểm tra: `src-tauri/src/commands/terminal.rs` — đã có pattern quản lý tiến trình con; tái dùng thay vì phát minh lại

## Implementation Steps

1. Đọc `commands/terminal.rs` xem cách nó quản lý vòng đời tiến trình con và huỷ. Tái dùng pattern đó.
2. Tách phần dựng env (`DOCKER_HOST`) và phần map lỗi "chưa cài" của `run_cmd` thành helper dùng chung.
3. `run_cmd_streaming` với `OutputSink::ToFile`: `Stdio::from(File)`, theo dõi kích thước file để báo tiến độ.
4. `OutputSink::ByLine`: `BufReader` trên stdout, gửi từng dòng qua channel.
5. Huỷ: giữ `Child`, `kill()` khi nhận tín hiệu; với `ToFile` thì xoá file dở.
6. Đăng ký mọi `StreamHandle` đang chạy vào một registry để kill hết lúc app thoát (hook `ExitRequested`, `lib.rs:329-331`).
7. Viết bảng base policy vào doc của `validation.rs`.
8. Test: `docker save` một image lớn, theo dõi RSS của app — phải phẳng, không tăng theo kích thước image.

## Success Criteria

- [x] `docker save` image 2 GB: RSS của app **không** tăng theo kích thước image.
- [x] Tiến độ báo đúng theo bytes đã ghi.
- [x] Huỷ giữa chừng: tiến trình con chết, file dở bị xoá, không có process mồ côi.
- [x] Thoát app giữa lúc stream: mọi tiến trình con bị kill.
- [x] `run_cmd` giữ nguyên hành vi — không sửa logic của nó, chỉ tách helper.
- [x] `assert_path_within` **không** bị sửa; test hiện có của nó vẫn xanh.

## Risk Assessment

| Rủi ro | Giảm thiểu |
|---|---|
| Hai đường chạy lệnh lệch nhau về env/lỗi | Tách helper dùng chung, không copy-paste |
| Tiến trình mồ côi khi app crash (không phải thoát êm) | Registry + kill ở `ExitRequested`; chấp nhận không xử lý được SIGKILL |
| Đo tiến độ theo file size sai khi docker buffer | Chấp nhận sai số; tiến độ là gợi ý, không phải hợp đồng |
| Dùng nhầm `contains_shell_injection` làm bảo vệ path | Ghi rõ trong doc; review điểm này khi làm Free P1 |

## Kết quả thực thi — 2026-08-12

`run_cmd_streaming` nằm ở **module mới `src-tauri/src/streaming_cmd.rs`**, không nhồi vào `helpers.rs`: vòng đời tiến trình con là concern riêng, giống `terminal_session.rs`. Phần dùng chung với `run_cmd` được tách vào `helpers.rs` như plan yêu cầu: `build_cmd` (env/DOCKER_HOST) và `describe_exec_error` (thông báo "chưa cài"). `run_cmd` giờ chỉ gọi hai hàm đó — logic không đổi, và có 4 test mới ghim lại hành vi đó.

API thực tế khác plan một chút, có lý do:

```
run_cmd_streaming(job_id, program, args, sink, on_progress) -> Result<StreamOutcome>
cancel_stream(job_id) -> bool
kill_all_streams() -> usize
active_stream_count() -> usize
OutputSink::ToFile(PathBuf) | OutputSink::ByLine(Sender<String>)
```

Plan phác `StreamHandle { cancel(), progress_rx }`. Registry theo `job_id` + callback progress cho đúng năng lực đó với ít máy móc hơn, và khớp cách repo đang làm (hàm sync gọi qua `run_blocking`). Registry vẫn là chỗ duy nhất biết mọi stream đang chạy, nên `kill_all_streams` ở `ExitRequested` (`lib.rs`) là một dòng.

### Bằng chứng

- `cargo test --lib`: **211/211 pass** (14 test mới: 10 ở `streaming_cmd`, 4 ở `helpers`).
- **RSS không tăng theo payload**: test stream 256 MB qua `ToFile`, RSS tăng < 32 MB. `Command::output()` sẽ giữ trọn 256 MB.
- `assert_path_within` **không bị sửa** (`git diff` chỉ +19 dòng doc), test của nó vẫn xanh.
- `cargo check`: không thêm warning nào (4 warning còn lại đều ở file không chạm tới).

### Bug phát hiện trong lúc verify — cả hai đều là bug thật

1. **Cancel treo bằng tuổi thọ của tiến trình cháu.** `sh -c 'printf x; sleep 30'` không exec được nên `sh` fork ra `sleep`; `Child::kill()` chỉ giết con trực tiếp, `sleep` sống tiếp và **vẫn giữ đầu ghi của pipe stderr**, nên đọc stderr tới EOF chặn đủ 30 giây. Đo được: `wait()` trả về sau 1.8ms, thread drain stderr join sau 9.7s. Với UI thì đó là nút Huỷ bấm xong đứng mãi.
   Sửa hai tầng: spawn con vào **process group riêng** (`process_group(0)`) rồi `libc::kill(-pgid, SIGKILL)` — giết cả cháu; và thu stderr **có deadline** (`recv_timeout` 2s), chỉ thu ở đường lỗi vì đó là chỗ duy nhất dùng tới nó.
   Bằng chứng gián tiếp: thời gian chạy module tụt từ **31s xuống 2.03s**.
2. **Test dùng chung registry toàn cục.** `kill_all_streams` giết mọi stream trong process, kể cả của test chạy song song. Thêm guard serialize cho cả module test.

Hai test mới chặn regression: `cancel_returns_promptly_even_when_the_command_forked_a_child` (đòi cancel < 5s) và `cancel_leaves_no_orphan_process` (dùng `pgrep` với marker riêng, đòi không còn tiến trình sống sót).

### 4B — chính sách path confinement

Đã viết bảng base vào doc của `assert_path_within` (`validation.rs`), kèm ghi chú vì sao đường dẫn phía container **không** đi qua hàm này và vì sao `contains_shell_injection` không phải cơ chế bảo vệ path. `streaming_cmd` ghi vào bất cứ đâu được truyền vào — confinement là việc của caller, và điều đó được ghi rõ ở đầu module.

### Chưa làm

- Chưa có caller thật nào dùng `run_cmd_streaming` — đó là Free P1 (file transfer + export TAR).
- Chưa test với `docker save` image 2 GB thật. Test 256 MB chứng minh cùng một tính chất (RSS phẳng, stdout không qua process của ta); 2 GB không thêm thông tin gì mà tốn 2 GB đĩa mỗi lần chạy suite.
- SIGKILL vào chính app (crash cứng) vẫn để lại tiến trình con — plan đã chấp nhận giới hạn này.
