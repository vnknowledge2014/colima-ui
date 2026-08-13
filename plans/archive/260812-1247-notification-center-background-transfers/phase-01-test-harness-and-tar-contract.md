---
phase: 1
title: Test harness and tar contract
status: completed
priority: P1
dependencies: []
effort: L
---

# Phase 1: Test harness and tar contract

## Overview

Nối `pnpm test` (chưa tồn tại) rồi sửa dứt điểm hợp đồng `.tar` ở tầng Rust. Phase
này **không phụ thuộc gì** và không đụng tới notification — nó là sửa lỗi độc lập,
mergeable riêng.

## Requirements

**Functional**
- Export chỉ chấp nhận tên file `.tar`.
- Import từ chối file không phải TAR trước khi gọi runtime.
- Copy-out luôn sinh ra một `.tar` hợp lệ, kể cả khi nguồn là thư mục.
- Huỷ hoặc thất bại không được xoá/truncate file đã tồn tại từ trước ở đích.
- Copy-out có kiểm tra dung lượng đĩa như export đã có.

**Non-functional**
- `pnpm test` chạy được.
- Không đường nào trong `file_transfer.rs` đưa giá trị người dùng vào vị trí cờ.

## 0. Nối test harness (finding #1)

`package.json` **không có script `test`** — chỉ `dev/build/check/typecheck/lint/
lint:rust/check:tokens/preview/tauri/prepare`. Vitest đã cài và `vitest.config.ts`
đã có, nhưng không cách nào chạy. Mọi acceptance criterion "test xanh" của plan này
vô nghĩa cho tới khi sửa.

```json
"test": "vitest run",
"test:watch": "vitest"
```

Làm bước này **trước tiên**, không phải cuối cùng.

## A. Ép đuôi `.tar` khi export (finding: B1)

`resolve_destination()` chỉ kiểm `require_plain` + `assert_path_within` + tồn tại.
`dialog.save({ defaultPath })` (`TransferDialog.svelte:114`) không có `filters`, nên
tên không đuôi lọt qua.

- Rust: `require_archive_name(file_name)` — chỉ chấp nhận đuôi `.tar`. Từ chối
  `.tar.gz`/`.tgz`/`.zip` kèm thông báo rõ: `docker save` ghi tar **không nén**, gắn
  đuôi nén là mô tả sai nội dung.
- Frontend: thêm `filters: [{ name: "TAR archive", extensions: ["tar"] }]`, và tự
  thêm `.tar` vào ô input để người dùng thấy tên cuối cùng trước khi bấm.

## B. Sniff TAR khi import (finding: B2, #6)

`require_existing_file()` chỉ gọi `is_file()`.

- Đọc 262 byte đầu; TAR ustar có `b"ustar"` tại offset 257.
- gzip (`1f 8b`) → thông báo riêng: giải nén trước.
- Không khớp nhưng đuôi là `.tar` và đọc được 512 byte → cho qua (GNU tar cũ).

**Giới hạn phải ghi đúng, không thổi phồng:** đây là kiểm tra **nhầm lẫn**, không
phải cổng bảo mật. Có TOCTOU giữa lúc sniff và lúc `docker load` đọc file. Success
criteria dưới đây viết theo đúng mức đó.

## C. Copy-out → sink `.tar` (finding: B3, #2)

`start_copy_from_container()` chạy `docker cp <id>:<path> <dest>`. Nguồn là thư mục
→ docker ghi ra cây thư mục, kéo theo hai lỗi:
- `watch: Some(dest)` đo `fs::metadata(dest).len()` = kích thước dirent (~64-96 byte).
  Progress đứng im.
- `cleanup_on_abort` gọi `std::fs::remove_file` — **thất bại trên thư mục**, kết quả
  bị nuốt bằng `let _ =`. Huỷ để lại cây sao chép dở, im lặng.

**Sửa:**
```rust
vec!["cp".into(), format!("{}:{}", container_id, container_path), "-".into()]
// JobPaths { sink: Some(dest_tar), ..Default::default() }
```
`streaming_cmd` tự dọn `ToFile` sink khi huỷ → xoá `cleanup_on_abort` thủ công, bug
`remove_file`-trên-thư-mục biến mất theo.

### Chi phí đã được chủ sở hữu chấp nhận

Đây là **thay đổi phá vỡ hợp đồng công khai**. Reviewer đề xuất dò file-vs-thư-mục
để tránh; chủ sở hữu giữ quyết định always-tar. Nên phải trả đủ giá, không giấu:

- Viết lại `src-tauri/tests/file_transfer_against_docker.rs:199-213` — test hiện
  assert `file_size(&returned) == 100*1024*1024` và so sánh sha256. Với `.tar` cả
  hai đều sai. Test mới: `tar -tf` liệt kê được, và sha256 của **nội dung sau khi
  giải nén** khớp.
- Tên `"returned.bin"` trong test đó sẽ bị `require_archive_name` từ chối → đổi
  thành `"returned.tar"`.
- Cập nhật `docs/api.md:178` cho `/api/containers/cp/from`.
- UI phải nói rõ ở dialog copy-out rằng kết quả là archive.

## D. `--` terminator (finding #9)

Luận điểm an toàn của bản plan đầu **sai**. `is_valid_container_id` cho phép `-`
đứng đầu (bộ ký tự có `-`, không có luật ký tự đầu), và `reject_flag_like` chỉ áp
cho `container_path`. Một container id bắt đầu bằng `-` đổi nghĩa lệnh.

- Chèn `--` trước các tham số vị trí ở mọi lệnh trong `file_transfer.rs`.
- `-` (đích stdout) là hằng số do code sinh, đặt **sau** `--`, không đi qua
  `reject_flag_like`.

## E. Không phá file có sẵn (finding #8)

`streaming_cmd.rs:259` dùng `File::create` — **truncate** file đã tồn tại — rồi
`remove_file` vô điều kiện khi huỷ/lỗi (`:267,276,288,297`). Với `overwrite: true`
là mode hợp lệ, người dùng huỷ giữa chừng sẽ mất file cũ mà không được cảnh báo.

- Ghi vào file tạm cùng thư mục (`<name>.tar.part`), `rename` sang đích khi thành
  công. Huỷ/lỗi chỉ xoá file tạm.
- `rename` trong cùng filesystem là atomic → không còn cửa sổ nào file đích ở trạng
  thái nửa vời.

## F. Kiểm dung lượng đĩa cho copy-out (finding #12)

`available_bytes` guard hiện chỉ có ở `start_image_save` (`file_transfer.rs:348-357`).
`docker cp <id>:/ -` ghi không giới hạn vào đĩa host. Không có total ước lượng được
cho copy-out, nên: từ chối khi free space dưới một ngưỡng tối thiểu, và huỷ job kèm
lỗi rõ ràng khi đĩa đầy giữa chừng.

## Related Code Files

- Modify: `package.json` — script `test`
- Modify: `src-tauri/src/commands/file_transfer.rs` — A, B, C, D, E, F
- Modify: `src-tauri/src/streaming_cmd.rs` — E (temp file + rename)
- Modify: `src-tauri/src/routes/file_transfer.rs`, `src-tauri/src/routes/payloads.rs`
  — đường HTTP có token auth, **bản plan đầu bỏ sót** (finding Medium)
- Modify: `src-tauri/tests/file_transfer_against_docker.rs` — viết lại copy-out
- Modify: `src/components/transfer/TransferDialog.svelte` — filters + tự thêm `.tar`
- Modify: `docs/api.md` — hợp đồng ba endpoint

## Implementation Steps

1. Thêm script `test` vào `package.json`; xác nhận `pnpm test` chạy suite hiện có.
2. `require_archive_name` + `sniff_tar` + unit test thuần Rust (không cần Docker).
3. `--` terminator cho mọi lệnh.
4. Temp-file + rename trong `streaming_cmd`; bỏ `remove_file` trên đích thật.
5. Copy-out sang `-` + `JobPaths { sink }`; bỏ `cleanup_on_abort` thủ công.
6. Disk guard cho copy-out.
7. Nối validate vào cả ba đường: Tauri command **và** HTTP route + payloads.
8. Viết lại integration test copy-out; đổi `returned.bin` → `returned.tar`.
9. `docs/api.md`; UI filters + tự thêm đuôi.
10. `cargo test`, `pnpm test`, `pnpm lint:rust`.

## Success Criteria

- [x] `pnpm test` chạy được và xanh. — script đã nối; 186/186.
- [x] Export tên không đuôi bị từ chối hoặc được tự thêm `.tar` ở UI trước khi gửi.
      — `require_archive_name` + `withTarSuffix`, cả hai có test.
- [x] Import `.gz`/`.zip` bị từ chối **trước** khi gọi runtime, thông báo nêu lý do.
- [x] Copy-out từ thư mục → `.tar` mà `tar -tf` liệt kê được nội dung.
      — `a_directory_copies_out_as_a_single_archive`, chạy với Docker thật.
- [x] Copy-out với `overwrite: true` rồi huỷ giữa chừng → file đích **cũ vẫn nguyên**.
      — `cancelling_does_not_destroy_an_existing_destination` +
      `a_failed_command_does_not_destroy_an_existing_destination`.
- [x] Container id bắt đầu bằng `-` không đổi được nghĩa lệnh.
      — `require_container_ref` + `--` terminator, `--` xác nhận thực nghiệm với
      Docker 29.5.2 cho cả `save` và `cp`.
- [~] Copy-out khi đĩa gần đầy bị từ chối kèm số liệu. — đã cài đặt
      (`MIN_COPY_OUT_FREE_BYTES`), **chưa có test**: dựng một filesystem gần đầy
      trong test không đáng giá. Kiểm bằng đọc code.
- [x] `file_transfer_against_docker.rs` xanh với assertion mới. — 6/6.
- [x] `docs/api.md` khớp hành vi thật.

## Kết quả

Cổng kiểm tra: vitest 186/186 · `cargo test --lib` 297/297 · integration 6/6 với
Docker thật · `svelte-check` 0/0 · eslint sạch · clippy `-D warnings` sạch.

Ngoài phạm vi dự kiến, phát hiện và xử lý trong lúc làm:

- Suite test **không chạy được** trên Node 20 (`jsdom@30` → `undici@8` cần
  `markAsUncloneable`, chỉ có từ Node 22). CI đã dùng Node 22; thêm `.nvmrc` để
  môi trường local khớp.
- `src/lib/normalizers.test.ts` có một assertion sai từ trước (mong `Names` là
  mảng; kiểu và mọi consumer đều coi là chuỗi). Sửa assertion, không sửa code.
- Code review bắt được: hai job cùng đích ghi chung một file `.part`. Đã khoá tên
  scratch theo `job_id` và thêm test.

Còn nợ, đã ghi vào Phase 2 chứ không im lặng: `unregister` chạy trước `rename`, nên
`active_stream_count() == 0` không còn hàm ý "đã publish"; và `overwrite: false` vẫn
có cửa sổ TOCTOU giữa lúc validate và lúc rename.

## Risk Assessment

- **`docker cp ... -` khác hành vi giữa runtime.** Colima dùng Docker CLI thật;
  nerdctl chưa kiểm. Giảm nhẹ: integration test chạy đúng lệnh production.
- **Hồi quy cho người copy file đơn lẻ** — có thật, đã chấp nhận. Phải ghi vào
  `docs/api.md` và nói rõ ở UI, không để người dùng tự phát hiện.
- **Sniff TAR quá chặt** từ chối archive hợp lệ → nhánh fallback ở mục B, ưu tiên
  cho qua khi không chắc.
- **Temp-file `.part` sót lại** khi tiến trình bị kill cứng. Chấp nhận: file có đuôi
  rõ ràng, và cách khác (ghi thẳng) đã được chứng minh là tệ hơn.
