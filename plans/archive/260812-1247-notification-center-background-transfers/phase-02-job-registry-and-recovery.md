---
phase: 2
title: Job registry and recovery
status: completed
priority: P1
dependencies: []
effort: M
---

# Phase 2: Job registry and recovery

## Overview

Dựng bề mặt backend làm cho "chạy nền" có nghĩa thật: một registry biết job nào đang
chạy, một cách để frontend hỏi lại sau khi mất kết nối, và một hợp đồng huỷ nói sự
thật. Không có phase này, đóng modal chỉ đơn giản là **mất dấu job**.

Đây là phần bản plan đầu khẳng định sai — "đây là redesign frontend, không phải
backend". Red team truy ra đúng ba lỗ hổng dưới đây.

## Requirements

**Functional**
- Liệt kê được job đang chạy, qua cả IPC lẫn HTTP.
- Job được đăng ký **trước** khi tiến trình con spawn.
- `cancel` trả về sự thật, không nuốt kết quả.
- Job kết thúc giữ lại trạng thái cuối một khoảng ngắn để client vừa reconnect đọc
  được.

**Non-functional**
- Không đổi hợp đồng event `transfer.*` đang có — chỉ **thêm** đường truy vấn.
- Không persist ra disk: `kill_all_streams` đã chạy ở `ExitRequested`, job không
  sống qua lần thoát app nào.

## A. Cửa sổ mất event (finding #3)

`spawn_job` gọi `tokio::task::spawn_blocking` (`file_transfer.rs:224`) **trước** khi
`Ok(TransferStarted)` trả về (`:373`). Nếu spawn thất bại, `streaming_cmd.rs:262-269`
phát `transfer.failed` trong vài micro giây — trước khi frontend nhận được `jobId`
để đăng ký. Ở browser mode còn tệ hơn: HTTP response và `EventSource`
(`transferEvents.ts:61-78`) không có bảo đảm thứ tự nào.

Bản plan đầu định nghĩa `settleJob` với id lạ là no-op im lặng → entry kẹt "running"
vĩnh viễn.

**Sửa:** đăng ký job vào registry với trạng thái `starting` **trước** `spawn_blocking`,
và cho `transfer_list` trả cả trạng thái cuối của job vừa kết thúc trong ~60s. Client
nào bỏ lỡ event vẫn đối soát được. Phía frontend, `settleJob` với id lạ ghi entry
mới thay vì no-op (Phase 3).

### Đính chính: đăng-ký-trước-spawn **không** tự nó đóng cửa sổ huỷ

Bản đầu của phase này khẳng định như vậy. Code review chỉ ra là sai, và đúng: ghi
`cancel_requested` vào registry rồi gọi `cancel_stream` không giúp gì khi tiến trình
con chưa tồn tại — `cancel_stream` trả `false`, không ai đọc lại cờ đó, và job chạy
tới hết trong khi người dùng đã được báo "đã huỷ".

Cách sửa thật, gồm ba phần:
1. `streaming_cmd::arm_cancel` ghi ý định vào một tập `PENDING_CANCEL`.
2. `register` tiêu thụ tập đó: cờ `cancelled` của job **khởi tạo bằng `true`** nếu đã
   armed.
3. Cờ `true` chưa dừng được gì — vòng `wait_with_progress` chờ tiến trình chứ không
   chờ boolean — nên ngay sau `register` phải `kill_group` cú kill mà `cancel_stream`
   không giao được.

**Cửa sổ còn lại, nói thẳng:** giữa lúc `unregister` chạy và lúc `rename` hoàn tất,
một cancel sẽ không tìm thấy job trong `RUNNING`, nhưng registry vẫn báo `Running`
nên vẫn trả `Cancelled`. Job vẫn publish file. Cửa sổ này dài đúng một lần `rename`
trong cùng filesystem. Không đóng được nếu không đưa cancel vào trong `stream_to_file`
— chi phí không tương xứng. Ghi lại ở đây thay vì giả vờ là không có.

## B. Không có đường phục hồi (finding #4)

- Không có command/route nào liệt kê job.
- `active_stream_count` (`streaming_cmd.rs:188`) chỉ dùng cho test và shutdown.
- `kill_all_streams` chỉ chạy ở `ExitRequested` (`lib.rs:362-368`).
- Job id là `AtomicU64` process-local (`file_transfer.rs:32`).

Reload webview → job mồ côi: không nhìn thấy, không huỷ được, vẫn ghi vào đĩa.

**Sửa:** thêm
```rust
#[tauri::command] pub async fn transfer_list() -> Result<Vec<TransferSnapshot>, ColimaError>
// + GET /api/transfers  (đi qua token auth như mọi route khác)
```
`TransferSnapshot { job_id, kind, status, bytes, total_estimate, started_at, target_label }`.

`target_label` là nhãn đã làm sạch để hiển thị (tên image, tên container), **không**
phải đường dẫn host thô — xem mục D.

## C. Huỷ nói sự thật (finding #11)

`register` xảy ra sau `File::create` + `spawn` (`streaming_cmd.rs:259-279`), nên có
cửa sổ mà job đã chạy nhưng chưa huỷ được. `cancel_stream` trả `false` im lặng
(`:155-165`), và bản plan đầu ở Phase 3 vứt luôn giá trị đó — trạng thái optimistic
"cancelling" sẽ không bao giờ được giải quyết.

**Sửa:** đăng ký trước khi spawn (chung với mục A); `cancel_transfer` trả một enum
phân biệt `Cancelled` / `AlreadyFinished` / `UnknownJob` thay vì `bool`, để UI hiển
thị đúng thay vì treo.

## D. Redaction trên đường transfer (finding #7)

Claim "mọi text đi qua `redact()`" trong bản plan đầu là **sai**. Không có `redact`
nào trong đường emit Rust; `transfer.failed` mang stderr thô của Docker
(`file_transfer.rs:299-307`, `streaming_cmd.rs:296`). `globalToast.ts:120` lưu
`error` chưa redact. `redact.ts:69` chỉ che `/Users|/home`.

Việc này quan trọng hơn ở đây so với chỗ khác vì Phase 6 sẽ đẩy text đó ra
Notification Center của hệ điều hành — rời khỏi tiến trình app.

**Sửa:** áp `crate::redact` cho `error` trong payload `transfer.failed` tại nguồn
(`file_transfer.rs`), không phải ở frontend. Redact tại nguồn là chỗ duy nhất phủ
được cả hai transport.

## E. Nợ kỹ thuật thừa kế từ Phase 1

Phase 1 chuyển sink sang `.part` + `rename`. Hai hệ quả được ghi nhận ở đó và xử lý
ở đây, vì cả hai đều thuộc về registry:

1. **`unregister` chạy trước `rename`** (`streaming_cmd.rs`). Nghĩa là
   `active_stream_count() == 0` không còn hàm ý "output đã publish" — một test hoặc
   một client đối soát ngay tại thời điểm đó có thể thấy job biến mất trước khi file
   đích xuất hiện. Registry của phase này có trạng thái tường minh, nên job chỉ
   chuyển sang `success` **sau** khi rename xong.
2. **`overwrite: false` vẫn có TOCTOU**: `resolve_destination` kiểm tồn tại lúc
   start, `rename` ghi đè lúc kết thúc. Cửa sổ giữa hai mốc không được bảo vệ. Với
   registry, một job có thể từ chối khởi động khi một job đang chạy đã nhắm cùng
   đích — đó là chỗ duy nhất biết đủ để trả lời.

## Related Code Files

- Create: `src-tauri/src/transfer_registry.rs` (hoặc thêm vào `file_transfer.rs`
  nếu nhỏ — quyết định lúc code, đừng tách sớm)
- Modify: `src-tauri/src/commands/file_transfer.rs` — đăng ký trước spawn, redact,
  `transfer_list`
- Modify: `src-tauri/src/streaming_cmd.rs` — thứ tự register/spawn, enum huỷ
- Modify: `src-tauri/src/routes/file_transfer.rs`, `routes/mod.rs` — `GET /api/transfers`
- Modify: `src-tauri/src/commands/mod.rs`, `src-tauri/src/lib.rs` — đăng ký command
- Modify: `docs/api.md` — endpoint mới

## Implementation Steps

1. Registry: map `job_id → TransferSnapshot`, có TTL ~60s cho job đã kết thúc.
2. Đổi thứ tự trong `spawn_job`: chèn vào registry (`starting`) rồi mới `spawn_blocking`.
3. `streaming_cmd`: `register` trước `File::create`; enum kết quả huỷ.
4. `transfer_list` command + route, đi qua token auth như các route khác.
5. Redact `error` tại nguồn trước khi emit `transfer.failed`.
6. Test: job kết thúc vẫn liệt kê được trong TTL; huỷ job không tồn tại trả
   `UnknownJob`; spawn thất bại vẫn để lại snapshot đọc được.
7. `cargo test`, `pnpm lint:rust`.

## Success Criteria

- [x] `transfer_list` liệt kê job đang chạy qua cả IPC lẫn HTTP.
- [x] Job spawn thất bại vẫn xuất hiện trong `transfer_list` với trạng thái lỗi.
      — đăng ký trước spawn; `JobGuard` phủ cả trường hợp task chết không qua nhánh
      kết thúc nào.
- [x] Job kết thúc còn đọc được ~60s sau đó.
      — `a_finished_transfer_is_still_readable_afterwards`, Docker thật.
- [x] `cancel_transfer` phân biệt được ba kết quả, không trả `bool`.
      — cả ba nhánh có test end-to-end trong
      `a_cancel_issued_before_the_child_exists_still_stops_the_job`.
- [~] `transfer.failed` không mang đường dẫn host chưa redact. — **chỉ đạt một
      phần, nói thẳng.** `redact()` che phân đoạn tài khoản trong home directory và
      các dạng bí mật đã biết; nó **không** xoá đường dẫn tuyệt đối bất kỳ, nên
      `Cannot create /Volumes/T7/img.tar` đi qua nguyên vẹn. Chấp nhận được với
      thông báo hiện cho chính người đã chọn đường dẫn đó. Ràng buộc kéo theo: Phase
      6 **không được** đưa trường này vào notification của hệ điều hành.
- [x] `GET /api/transfers` từ chối request thiếu token. — đi qua cùng lớp auth như
      mọi route khác.

## Kết quả

vitest 188/188 · `cargo test --lib` 304/304 · integration 9/9 với Docker thật ·
`svelte-check` 0/0 · eslint sạch · clippy `-D warnings` sạch.

Code review bắt được ba thứ, đã sửa hết:

- **Cửa sổ huỷ chưa đóng** — xem mục đính chính ở trên. Sửa bằng
  `arm_cancel`/`PENDING_CANCEL` + kill ngay sau `register`, và ghi lại phần cửa sổ
  còn sót thay vì tuyên bố đã hết.
- **Mutex poisoned làm mất job im lặng** — `list()` trả rỗng khi lock poisoned sẽ
  khiến client kết luận mọi transfer đã xong. Đổi sang `into_inner()`: map không có
  bất biến nào một panic phá được.
- **Rò rỉ claim đích khi task chết giữa chừng** — entry kẹt `Running` không bao giờ
  bị prune và giữ chỗ đường dẫn đó tới hết phiên. Thêm `JobGuard` settle khi drop.

## Risk Assessment

- **Registry rò rỉ bộ nhớ** nếu TTL không dọn. Giảm nhẹ: dọn khi chèn, không cần
  timer riêng.
- **TTL 60s là số tuỳ chọn** — đủ cho reload webview, không đủ nếu người dùng để máy
  ngủ. Chấp nhận: sau TTL, job vắng mặt = đã xong, và UI diễn giải đúng như vậy.
- **Redact tại nguồn có thể che mất thông tin cần cho debug.** `errorLog` vẫn giữ bản
  đã redact như mọi lỗi khác — không có đường nào trong app hiện hiển thị bản thô.
