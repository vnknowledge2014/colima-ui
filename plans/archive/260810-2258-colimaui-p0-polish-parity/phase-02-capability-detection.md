---
phase: 2
title: Capability Detection
status: completed
priority: P1
effort: 3-4 ngày
dependencies:
  - 1
---

# Phase 2: Capability Detection

## Overview

Trả lời câu hỏi "chưa cài gì thì sao?" bằng **một nguồn sự thật** về trạng thái công cụ trên máy, rồi cắm nó vào các bề mặt onboarding **đã tồn tại** thay vì dựng bề mặt thứ ba.

## Sửa lại từ v1

| v1 nói | Thực tế | v2 làm |
|---|---|---|
| "Mở rộng `routes/capabilities.rs` (DRY)" | File đó là schema API cho AI Agent — 35 dòng `json!` tĩnh, phân loại `SAFE`/`DANGEROUS`. Không liên quan detect công cụ | **Tạo route mới** `routes/system_capabilities.rs`. Đừng trộn 2 hợp đồng không liên quan |
| "Tạo `EmptyState.svelte` + Dashboard checklist" | `SetupWizard.svelte` (568 dòng) + `GettingStartedTour.svelte` (171 dòng) + empty state ở 11 file = đã 2 bề mặt | **Bỏ** component và checklist mới. Cắm dữ liệu vào SetupWizard + empty state sẵn có |
| "Dùng `SYSTEM_INFO_CACHE` sẵn có" | Cache đó ở `helpers.rs:130`, TTL cứng 300s, dùng chung `/api/system` + `/api/version`, `TimedCache` **không có hàm invalidate** (`helpers.rs:102-128`), và std Mutex bị giữ qua 3 lần spawn process bên trong handler async (`routes/system.rs:12-18`) | **Cache riêng**, TTL ngắn, có invalidate, spawn qua `spawn_blocking` |

## Requirements

**Functional**
- Trạng thái từng thành phần: `colima`, `docker`/`nerdctl`, `limactl`, `kubectl`, `docker compose` — mỗi cái là `missing | installed_not_running | running | unknown`, kèm `version`, `install_hint`, `doc_id`.
- Invalidate cache khi instance đổi trạng thái (start/stop) — không chờ TTL.
- `SetupWizard` và các empty state hiện có đọc từ store này, bỏ mọi logic detect trùng lặp.

**Non-functional**
- Không giữ std Mutex qua ranh giới await hay qua process spawn.
- Detect chạy song song, không chặn render lần đầu.
- Hoạt động cả khi app mở từ Finder (PATH khác) — `path_util::fix_path_env()` đã xử lý, nhưng phải test trên bản bundle thật.

## Architecture

```
src-tauri/src/commands/system_capabilities.rs (MỚI — tách khỏi system.rs)
  ├─ detect_all() → Vec<Capability>   (spawn_blocking, chạy song song)
  ├─ CapCache { TTL ngắn + invalidate() }   ← RIÊNG, không dùng SYSTEM_INFO_CACHE
  └─ invalidate gọi từ poller khi instance đổi trạng thái

src-tauri/src/routes/system_capabilities.rs (MỚI — KHÔNG phải capabilities.rs)
src/store/capabilities.svelte.ts (MỚI)
       ├─→ SetupWizard.svelte      (bỏ detect riêng)
       └─→ empty state ở 11 file   (dùng dữ liệu, không tạo component mới)
```

## Related Code Files

- Create: `src-tauri/src/commands/system_capabilities.rs`
- Create: `src-tauri/src/routes/system_capabilities.rs`
- Modify: `src-tauri/src/lib.rs` — đăng ký command + route
- Modify: `src-tauri/src/poller.rs` — gọi invalidate khi instance đổi trạng thái
- Modify: `src-tauri/src/platform.rs` — `install_hint` phân nhánh macOS/Linux
- Create: `src/store/capabilities.svelte.ts`
- Modify: `src/components/SetupWizard.svelte` — dùng store, xoá detect riêng
- Modify: các trang có empty state — truyền capability phụ thuộc đúng
- Modify: `src/lib/api/system.ts`, `src/locales/*` (4 locale)
- **Không** tạo: `EmptyState.svelte`, `OnboardingChecklist.svelte`

## Implementation Steps

1. Đọc `SetupWizard.svelte` (568 dòng) xem nó đang detect những gì và bằng cách nào — phần lớn logic có thể chuyển thẳng xuống Rust.
2. Viết `detect_all()` chạy song song qua `spawn_blocking`; mỗi capability trả state + version + hint + doc_id.
3. Viết `CapCache` riêng với TTL ngắn (~15s) và `invalidate()`. Không đụng `SYSTEM_INFO_CACHE`.
4. Nối invalidate vào `poller.rs` tại điểm phát hiện instance đổi trạng thái.
5. Tạo route HTTP **mới**; đăng ký cả Tauri command.
6. `capabilities.svelte.ts` — load lúc khởi động, refresh sau mỗi thao tác start/stop.
7. Refactor `SetupWizard.svelte` đọc từ store; xoá code detect trùng.
8. Rà 11 file có empty state; chỗ nào nguyên nhân là thiếu công cụ thì hiện đúng trạng thái + CTA, dùng markup sẵn có của trang.
9. i18n 4 locale.

## Tests / Validation

- Rust unit: chuẩn hoá state từ output giả lập cho từng công cụ.
- Rust unit: cache invalidate hoạt động; TTL đúng.
- Rust: khẳng định không giữ Mutex qua await (review thủ công + `cargo clippy`).
- Thủ công: đổi tên binary `colima` tạm thời → mọi trang hiện `missing`, không phải bảng trống.
- Thủ công: cài công cụ giữa phiên → sau start/stop instance thì trạng thái cập nhật, không chờ 300s.
- Thủ công: chạy **bản bundle** (không phải `pnpm tauri dev`) mở từ Finder → detect vẫn đúng.

## Success Criteria

- [ ] Route và command **mới**, không đụng `routes/capabilities.rs`.
- [ ] Cache riêng, có invalidate, không giữ Mutex qua spawn.
- [ ] `SetupWizard` không còn detect riêng.
- [ ] Trang có empty state phân biệt được `missing` / `not_running` / thật sự trống.
- [ ] Mỗi trạng thái thiếu công cụ có đúng 1 CTA hoạt động.
- [ ] Đúng trên bản bundle mở từ Finder.
- [ ] Không tạo component onboarding mới.

## Risk Assessment

| Rủi ro | Giảm thiểu |
|---|---|
| Refactor SetupWizard 568 dòng phát sinh hồi quy | Chuyển từng bước, giữ hành vi; test thủ công trên máy sạch |
| Detect sai do PATH khi mở từ Finder | Bắt buộc test trên bản bundle, không chỉ dev |
| Spawn nhiều process làm chậm khởi động | Song song + cache + không chặn render |
| Hint cài đặt lệch giữa macOS/Linux | Phân nhánh qua `platform.rs` |
