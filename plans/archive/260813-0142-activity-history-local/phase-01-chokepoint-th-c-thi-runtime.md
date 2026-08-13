---
phase: 1
title: Chokepoint thực thi runtime
status: completed
priority: P2
dependencies: []
effort: 0.5 ngày
---

# Phase 1: Chokepoint thực thi runtime

## Overview

Gộp bốn bản sao của `docker_output` thành một hàm dùng chung. **Không đổi hành
vi.** Mục đích duy nhất: tạo ra một chỗ để phase 3 cắm việc ghi nhận vào.

## Requirements

- Functional: mọi lệnh docker hiện có chạy y như cũ, cùng timeout, cùng thông
  điệp lỗi.
- Non-functional: sau phase này tồn tại **đúng một** đường chạy lệnh docker.

## Architecture

Hôm nay:

```
containers.rs:51  async fn docker_output(args) -> Output   ← 25 call site
volumes.rs:37     async fn docker_output(args) -> Output   ← 6
networks.rs:25    async fn docker_output(args) -> Output   ← 6
compose.rs:16     async fn docker_output(args) -> Output   ← 7
```

Bốn thân hàm gần như giống hệt: `timeout(10s)` + `spawn_blocking` +
`get_runtime_cmd().args(...).output()`.

Sau phase này:

```
commands/runtime.rs
  ├─ get_runtime_cmd()                     (giữ nguyên)
  └─ pub async fn run(args, timeout) -> Result<Output, String>
       ← chỗ duy nhất phase 3 cần chạm vào
```

**Không** dựng trait, không dựng struct executor. Một hàm tự do là đủ cho một
tập lệnh đóng — thêm lớp trừu tượng ở đây là chi phí không mua được gì.

### Timeout: cả bốn đều 10s — giữ nguyên, kể cả chỗ sai

<!-- Updated: Validation Session 1 - claim "timeout không đồng nhất" đã bị bác bởi verification pass -->

Đã đo 2026-08-13: **cả bốn bản sao đều `from_secs(10)`**, không hề khác nhau.
Bản nháp phase này phỏng đoán ngược lại — sai.

Hàm chung vẫn nhận `timeout` làm tham số (để chỗ gọi tự khai báo ý định), nhưng
mọi call site truyền đúng **10 giây** như hôm nay.

**Bug đã phát hiện, cố ý không sửa ở đây:** `compose_up` (`compose.rs:81`) chạy
qua timeout 10 giây. `docker compose up` có thể pull image và mất vài phút, nên
lệnh này đang bị cắt giữa chừng trên máy thật.

Chốt với chủ dự án 2026-08-13: **giữ nguyên, mở việc riêng.** Phase này là
refactor thuần — trộn một bug fix vào một diện tích đổi 4 tệp làm cả hai khó
review và khó rollback. Gộp xong thì việc sửa timeout chỉ còn là đổi một tham số
ở một chỗ, tức phase này khiến bug **dễ sửa hơn** dù không sửa nó.

## Related Code Files

- Modify: `src-tauri/src/commands/runtime.rs` (thêm `run`)
- Modify: `src-tauri/src/commands/containers.rs` (xoá bản sao, đổi call site)
- Modify: `src-tauri/src/commands/volumes.rs`
- Modify: `src-tauri/src/commands/networks.rs`
- Modify: `src-tauri/src/commands/compose.rs`

## Implementation Steps

1. Đọc cả bốn `docker_output`, lập bảng so sánh **thông điệp lỗi** và cách xử lý
   join error. (Timeout đã đo: cả bốn là 10s — không cần so nữa.)
2. Viết `runtime::run(args: Vec<String>, timeout: Duration)` mang đúng ngữ nghĩa
   hợp nhất. Khác biệt nào không hợp nhất được thì giữ ở call site.
3. Đổi từng module một, chạy `cargo test --lib` sau mỗi module — không đổi cả
   bốn rồi mới chạy.
4. Xoá bốn bản sao. `grep -c "fn docker_output"` phải ra 0.

## Tests

- `cargo test --lib` giữ nguyên số test pass (hiện **437**), không giảm.
- Test mới: `run` trả timeout đúng thông điệp khi lệnh treo.
- Kiểm bằng tay: liệt kê container, xoá volume, `compose up` — cả ba vẫn chạy.

## Success Criteria

- [ ] `grep -rc "fn docker_output" src-tauri/src` → 0.
- [ ] Mọi call site vẫn là 10s. `compose_up` **vẫn** 10s — bug được giữ nguyên có chủ đích.
- [ ] `cargo clippy -D warnings` sạch, số test không giảm.

## Risk Assessment

| Rủi ro | Giảm thiểu |
|---|---|
| ~~Gộp nhầm timeout~~ | Không còn: cả bốn đã đo là 10s. `compose up` bị cắt là bug **có sẵn**, giữ nguyên, mở việc riêng |
| Đổi thông điệp lỗi → test hoặc UI khớp chuỗi bị vỡ | Grep chuỗi lỗi cũ trong test và `src/` trước khi đổi |
| Phase "dọn dẹp" phình thành refactor lớn | Phạm vi cứng: chỉ `docker_output`. Không đụng adapters colima/lima ở phase này |
