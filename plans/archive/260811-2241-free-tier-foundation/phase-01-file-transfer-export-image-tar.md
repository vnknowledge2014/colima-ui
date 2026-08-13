---
phase: 1
title: "File transfer + Export image TAR"
status: done
priority: P2
dependencies: ["prereq:P1", "prereq:P4"]
effort: ""
---

# Phase 1: File transfer + Export image TAR

## Overview

GUI cho ba thao tác `docker` đã có: copy file vào/ra container, và xuất image thành TAR. Giá trị nằm ở chỗ người dùng không phải nhớ cú pháp `docker cp container:/path ./path` và không phải đoán container id.

> **Sửa sau red-team 2026-08-11.** Phase này chờ tiên quyết P1 (token hardening) và P4 (streaming command). Hai đính chính bên dưới.

## Đính chính tiền đề

| Bản đầu nói | Thực tế |
|---|---|
| `tauri-plugin-dialog` "đã có trong capabilities" | **Sai.** Vắng ở `capabilities/default.json`, `Cargo.toml`, `package.json`. Thêm nó là thay đổi trust boundary và phase này sở hữu việc đó. |
| Dùng `Command` bình thường là đủ | **Sai.** `run_cmd` dùng `Command::output()` — buffer toàn bộ stdout vào RAM (`helpers.rs:98-133`). `docker save` 2 GB sẽ nằm trọn trong bộ nhớ. Dùng `run_cmd_streaming` từ tiên quyết P4. |
| `validation.rs` cần API confinement mới | **Không.** `assert_path_within:169-195` đã viết tử tế (canonicalize parent, chống `..` và symlink). Dùng nguyên, base là thư mục người dùng chọn qua dialog. |

**Lưu ý:** `contains_shell_injection` (`validation.rs:7-21`) là denylist metachar cho lệnh exec, **không phải** cơ chế bảo vệ đường dẫn. Đừng dùng nhầm chỗ.

## Requirements

**Functional**
- Copy host → container, và container → host, chọn đường dẫn bằng dialog hệ thống.
- Export một hoặc nhiều image thành `.tar` (`docker save`), có thanh tiến độ.
- Import ngược lại từ `.tar` (`docker load`). Không phải "cho đối xứng" — nó là nửa còn lại của một trường hợp dùng thật (chuyển image sang máy không có mạng), và phải có tiêu chí nghiệm thu riêng như mọi thứ khác.
- Hiển thị dung lượng ước tính trước khi export để người dùng không ghi 8 GB vào ổ đầy.

**Non-functional**
- Thao tác dài không được block UI: chạy nền, phát tiến độ qua SSE.
- Huỷ giữa chừng phải xoá file `.tar` dang dở, không để lại rác.

## Architecture

```
UI (Images.svelte / Containers.svelte)
  └─ dialog chọn path (tauri-plugin-dialog — PHẢI THÊM, xem đính chính)
      └─ POST /api/containers/cp | /api/images/save | /api/images/load
          └─ commands/file_transfer.rs
              ├─ assert_path_within(base = thư mục người dùng chọn, candidate)
              ├─ run_cmd_streaming(...)  ← tiên quyết P4, KHÔNG dùng run_cmd
              └─ sse.rs → publish_sse_event("transfer.progress", {...})
```

**Về dialog và uỷ quyền:** dialog hệ thống xác nhận *ý định của người dùng ở UI*. Nó **không** phải cơ chế uỷ quyền cho route HTTP — route `/api/containers/cp` vẫn gọi được trực tiếp bằng token. Đó là lý do phase này chờ tiên quyết P1.

Đặt module mới `commands/file_transfer.rs` thay vì nhét vào `containers.rs`/`images.rs`: nó cắt ngang cả hai và có vòng đời riêng (job chạy nền, huỷ được).

**Bảo mật — điểm dễ sai nhất của phase này:** đường dẫn phía container do người dùng gõ tay. Phải chạy qua `validation.rs` và không bao giờ nội suy thẳng vào chuỗi shell. Dùng `Command::args()`, không dùng `sh -c`.

## Related Code Files

- Create: `src-tauri/src/commands/file_transfer.rs`
- Create: `src-tauri/src/routes/file_transfer.rs`
- Create: `src/components/transfer/TransferDialog.svelte`
- Create: `src/lib/api/transfer.ts`
- Modify: `src-tauri/src/commands/mod.rs`, `src-tauri/src/routes/mod.rs`, `src-tauri/src/api_server.rs`, `src-tauri/src/lib.rs` (đăng ký command)
- Modify: `src/pages/Images.svelte` (nút Export/Import), `src/pages/Containers.svelte` (nút Copy files)
- Modify: `src/locales/{en,vi,ja,zh}.json`

## Implementation Steps

1. `file_transfer.rs`: định nghĩa `TransferJob { id, kind, source, dest, state }` và một registry `Mutex<HashMap<String, JobHandle>>` để huỷ được.
2. Ba command: `container_cp(container_id, src, dest, direction)`, `image_save(image_ids, dest_tar)`, `image_load(tar_path)`.
3. Validate path trước khi spawn: từ chối `..`, từ chối path rỗng, từ chối ghi đè file đang tồn tại trừ khi người dùng xác nhận.
4. Đọc stdout/stderr của `docker save` theo dòng, ước lượng tiến độ theo bytes đã ghi ra file đích (docker không báo % nên đo file size là cách trung thực nhất).
5. Phát `transfer.progress` và `transfer.done` / `transfer.failed` qua SSE.
6. Route HTTP tương ứng + đăng ký ở `api_server.rs`.
7. `TransferDialog.svelte`: chọn nguồn/đích, hiện tiến độ, nút Huỷ.
8. Gắn nút vào Images (Export/Import) và Containers (Copy files).
9. Thêm chuỗi vào 4 locale.

## Success Criteria

- [x] Copy được file 100 MB vào và ra container, nội dung khớp checksum.
- [x] Export 1 image → `.tar`, `docker load` lại được bằng CLI.
- [x] **Import**: `.tar` do `docker save` bên ngoài tạo ra → load được trong app, image chạy được.
- [x] Huỷ giữa chừng: file `.tar` dang dở bị xoá, không còn tiến trình `docker save` mồ côi.
- [x] Path chứa `../` bị từ chối qua `assert_path_within`, có thông báo rõ, không lệnh nào được spawn.
- [x] UI không đứng trong suốt export 2 GB.
- [x] **RSS của app không tăng theo kích thước image khi export** (bằng chứng đã dùng streaming, không phải `Command::output()`).
- [x] `tauri-plugin-dialog` được thêm đúng cách: `Cargo.toml`, `package.json`, và quyền tương ứng trong `capabilities/default.json`.

## Risk Assessment

| Rủi ro | Giảm thiểu |
|---|---|
| Path injection qua đường dẫn trong container | `Command::args()`, không `sh -c`; validate qua `validation.rs` |
| Ghi đè file người dùng | Kiểm tra tồn tại + hỏi trước khi ghi |
| Job mồ côi khi app đóng giữa chừng | Kill toàn bộ job trong registry ở hook shutdown |
| Đầy đĩa khi export image lớn | Hiện dung lượng ước tính trước; kiểm tra dung lượng trống, cảnh báo nếu thiếu |

## Kết quả thực thi — 2026-08-12

Tiên quyết P1 (token hardening) và P4 (streaming) đều đã xong nên phase này chạy được ngay.

### Bằng chứng

- `cargo test --lib`: **240/240 pass** (13 test mới ở `commands/file_transfer.rs`).
- **5/5 tiêu chí end-to-end verify với docker thật** (`src-tauri/tests/file_transfer_against_docker.rs`, gate `#[ignore]` vì chạm daemon thật):
  - export image → `docker load` của CLI **chấp nhận** file tar;
  - tar do **CLI `docker save` tạo** → import được, image dùng được sau đó;
  - file 100 MB đi vào rồi ra khỏi container, **SHA256 khớp**;
  - huỷ giữa chừng → tar dở **bị xoá**, `pgrep` không còn `docker save` mồ côi;
  - RSS **không** tăng theo kích thước image khi export image 1.76 GB (tăng < 64 MB).
- `svelte-check`: về đúng baseline 143 lỗi/36 warning/36 file — **0 lỗi và 0 warning** từ file của phase này.
- `pnpm run build` pass; `vitest run` 139/139.
- `tauri-plugin-dialog` thêm đủ bốn chỗ: `Cargo.toml`, `package.json`, `.plugin(tauri_plugin_dialog::init())` ở `lib.rs`, và quyền ở `capabilities/default.json`.

### Lệch khỏi plan, có lý do

1. **Không dựng registry job thứ hai.** Plan bước 1 yêu cầu `Mutex<HashMap<String, JobHandle>>` trong `file_transfer.rs`. Registry của `streaming_cmd` (tiên quyết P4) đã sở hữu mọi tiến trình con, huỷ theo id, giết cả process group và được dọn ở `ExitRequested`. Thêm registry nữa là hai nguồn sự thật cho cùng một tập tiến trình. `cancel_transfer` uỷ quyền thẳng cho nó.
2. **`container_cp(.., direction)` tách thành hai command.** `copy_to_container` và `copy_from_container` có tham số khác nhau thật: chiều vào nhận một `hostPath` đã tồn tại (chỉ đọc), chiều ra nhận `destDir` + `fileName` (ghi, phải confine). Một hàm với nửa số tham số không dùng sẽ che mất chính điểm bảo mật đó.
3. **`destDir` và `fileName` là hai trường riêng.** Đây là điểm quan trọng nhất: `assert_path_within(base, candidate)` với base lấy từ chính parent của candidate thì **luôn đúng** — vô nghĩa. Confinement chỉ có nghĩa khi base tới từ nguồn khác (thư mục người dùng chọn). Nhờ vậy `fileName = "../x"` bị từ chối thật, và có test chứng minh.
4. **`streaming_cmd` phải nhận `Command` dựng sẵn.** `run_cmd_streaming(program, args)` tự dựng Command nên bỏ mất `get_runtime_cmd()` — tức là hỏng với podman/containerd (`colima nerdctl`) và không áp PATH. Thêm `run_streaming(job_id, cmd, label, ...)`; `run_cmd_streaming` giờ chỉ là wrapper. Đây là lỗi của P4 mà chỉ lộ ra khi có caller thật.
5. **Sự kiện phải phát trên hai kênh.** `publish_sse_event` chỉ tới browser mode; app desktop nghe Tauri event (`dataPoller.ts` dùng `listen`, chỉ browser mới mở `EventSource`). Job phát cả `publish_sse_event` lẫn `app.emit`. Không có AppHandle toàn cục nên command Tauri nhận `AppHandle` rồi truyền vào job; route HTTP truyền `None`.
6. **Quyền dialog theo least privilege**: `dialog:allow-open` + `dialog:allow-save`, **không** dùng `dialog:default` (kèm cả ask/confirm/message — không cần).
7. **Browser mode không bị chết**: không có file picker thì ô đường dẫn nhập tay, thay vì biến tính năng thành desktop-only.
8. **Kiểm tra dung lượng trống**: từ chối khi free < estimate, thông báo cả hai con số để người dùng chọn thư mục khác. Plan ghi "cảnh báo"; từ chối là lựa chọn an toàn hơn ghi đầy đĩa, và người dùng vẫn hành động được.

### Chưa verify

- **"UI không đứng khi export 2 GB"**: đã verify ở mức hợp đồng — command trả `jobId` ngay, việc nặng nằm trên `spawn_blocking`, tiến độ chảy qua sự kiện, nút Huỷ luôn bấm được. **Chưa** click thật trên GUI đang chạy.
- Nhánh từ chối vì thiếu dung lượng chưa có test (khó dựng ổ đầy); `available_bytes` thì có test.
- `transferEvents.ts` mở `EventSource` riêng ở browser mode thay vì dùng chung kết nối của `dataPoller` (module đó không có bus và đang được người khác sửa). Nếu sau này có bus dùng chung thì nên chuyển sang.

**Bổ sung 2026-08-12 — verify tương tác ở tầng component.** Không click được cửa sổ thật trong môi trường này (lý do ở `plans/reports/from-sequential-thinking-to-owner-260812-1233-gui-verification-limit-and-component-interaction-coverage-report.md`), nhưng phần *logic* của việc click thì mount và bấm được — cùng kỹ thuật đã bắt lỗi `effect_update_depth_exceeded` ở `GraphCanvas`.

8 test ở `src/components/transfer/TransferDialog.test.ts`: Start bị chặn tới khi các ô đủ nghĩa (export không chọn image, copy thiếu một đầu); `destDir` và `fileName` truyền **rời nhau** đúng như confinement cần; lỗi start hiện **tại chỗ** chứ không chỉ là toast; tiến độ của job khác **không** làm nhảy thanh của dialog này; Huỷ gọi đúng `jobId`; job bị huỷ được coi là **xong**, không phải thất bại.

Còn lại chưa phủ: bố cục thị giác, hành vi ở tầng OS (file picker native), và cảm giác khi dùng.
