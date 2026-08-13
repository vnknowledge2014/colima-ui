---
phase: 4
title: Compose Labels & Grouping
status: completed
priority: P2
effort: 3-4 ngày
dependencies:
  - 1
---

# Phase 4: Compose Labels & Grouping

## Overview

Nhóm container theo compose project như OrbStack.

**Phát hiện quyết định:** `grep -ci labels` trên `src-tauri/src/docker_state.rs` và `src-tauri/src/commands/containers.rs` = **0**. Tính năng này hiện **không có nguồn dữ liệu**. Plan v1 coi đây là "kiểm tra ở bước 1"; thực tế nó là phần lớn công việc.

## Sửa lại từ v1

| v1 | Thực tế | v2 |
|---|---|---|
| "Trích `DataTable` dùng chung cho 5 trang" | `Containers.svelte` **không có thẻ `<table>` nào** (chỉ `class="table-actions"` tại `:378`); chỉ `Kubernetes.svelte` và `Dashboard.svelte` có `<table>`. Zero consumer cho abstraction đề xuất | **Bỏ** framework DataTable/ColumnHeader/TableGroup. Làm grouping trực tiếp trong `Containers.svelte` |
| "Backend có thể đã trả labels" | 0 match ở cả 2 file | **Labels plumbing là bước 0**, chiếm phần lớn phase |
| Liệt kê 1 mapper container | `docker_state.rs:210-253` (bollard, 9 key, không labels) **và** `sse.rs:152` (mapper inline riêng) | Sửa **cả 2**, hoặc tốt hơn: cho `sse.rs` dùng lại `map_containers` |
| — | Hai đường dữ liệu có shape labels khác nhau: bollard trả `HashMap`, CLI trả chuỗi phân tách dấu phẩy | Chuẩn hoá ở Rust, không ở TS |

## Requirements

**Functional**
- Backend trả `labels` cho container ở **cả 2 đường dữ liệu**, cùng một shape đã chuẩn hoá.
- Container nhóm theo `com.docker.compose.project`; container lẻ vào nhóm "Standalone".
- Nhóm thu/mở được, hiện tổng số + số đang chạy, có hành động cấp nhóm.
- Toggle Nhóm / Danh sách phẳng, được ghi nhớ.

**Non-functional**
- Không thêm component library, không dựng framework bảng.
- Logic nhóm là hàm thuần, test được độc lập với UI.
- Hoạt động giống hệt ở desktop mode và browser mode — đây là chỗ dễ hỏng nhất.

## Architecture

```
BƯỚC 0 — labels plumbing (phần lớn công việc)
src-tauri/src/docker_state.rs :210-253  map_containers()
   └─ thêm labels: HashMap<String,String>   (bollard)
src-tauri/src/sse.rs :152                mapper inline
   └─ TỐT NHẤT: xoá mapper này, gọi lại map_containers()
      nếu không: chuẩn hoá chuỗi CLI phân tách dấu phẩy → cùng HashMap
src-tauri/src/commands/containers.rs
   └─ trả labels

src/lib/api/types.ts  Container.labels?: Record<string,string>

BƯỚC 1 — grouping (nhẹ)
src/lib/composeGrouping.ts   groupContainersByProject() — hàm thuần + test
src/pages/Containers.svelte  render nhóm trực tiếp, KHÔNG qua DataTable
```

## Related Code Files

- Modify: `src-tauri/src/docker_state.rs` — `map_containers` trả labels
- Modify: `src-tauri/src/sse.rs` — dùng lại `map_containers` thay vì mapper inline tại `:152`
- Modify: `src-tauri/src/commands/containers.rs` — trả labels
- Modify: `src/lib/api/types.ts` — `labels` trong type Container
- Create: `src/lib/composeGrouping.ts` + `composeGrouping.test.ts`
- Modify: `src/pages/Containers.svelte` — render nhóm, toggle, hành động cấp nhóm
- Modify: `src/lib/settingsStore.svelte.ts` — nhớ toggle + nhóm đang thu
- Modify: `src/locales/*` (4 locale)
- **Không** tạo: `DataTable.svelte`, `ColumnHeader.svelte`, `TableGroup.svelte`

## Implementation Steps

1. **Bước 0a:** thêm `labels` vào `map_containers` trong `docker_state.rs` (bollard đã có sẵn field labels).
2. **Bước 0b:** xử lý `sse.rs:152` — ưu tiên xoá mapper inline và gọi `map_containers`. Nếu không khả thi thì chuẩn hoá shape labels để khớp tuyệt đối.
3. **Bước 0c:** kiểm chứng ngay: gọi cả Tauri IPC và HTTP `/api/containers`, so sánh JSON — labels phải giống hệt. Không qua bước này thì không đi tiếp.
4. Thêm `labels` vào `types.ts`.
5. Viết `composeGrouping.ts` thuần + test: không nhãn, nhãn rỗng, nhiều project, tên project trùng tên service, ký tự lạ.
6. Render nhóm trong `Containers.svelte` bằng markup sẵn có của trang; thu/mở; toggle Nhóm/Phẳng.
7. Hành động cấp nhóm (start/stop/remove cả project) gọi qua `withErrorReport` của Phase 1.
8. Nhớ trạng thái toggle + nhóm đang thu qua `settingsStore` (lưu ý store nuốt lỗi ghi — kiểm tra ghi thành công).
9. i18n 4 locale.

## Tests / Validation

- Rust unit: `map_containers` trả labels; shape khớp giữa 2 đường.
- **Integration bắt buộc:** so sánh JSON container từ Tauri IPC và từ HTTP → labels giống hệt.
- TS unit: `composeGrouping` với ≥6 trường hợp biên.
- Thủ công: `docker compose up` 1 project → nhóm hiện đúng ở **cả desktop và browser mode**.
- Thủ công: stop cả nhóm hoạt động; lỗi hiện qua hợp đồng lỗi v2.
- Hiệu năng: ~200 container, scroll mượt (chưa cần virtualization; chỉ thêm nếu đo thấy chậm).

## Success Criteria

- [ ] Labels trả về ở **cả 2 đường dữ liệu**, shape giống hệt, có test so sánh.
- [ ] `sse.rs` không còn mapper container trùng lặp (hoặc đã chuẩn hoá tuyệt đối).
- [ ] Container nhóm theo compose project, thu/mở, có hành động cấp nhóm.
- [ ] Toggle Nhóm/Phẳng được ghi nhớ.
- [ ] Grouping hoạt động ở browser mode.
- [ ] Không tạo framework bảng nào.

## Risk Assessment

| Rủi ro | Mức | Giảm thiểu |
|---|---|---|
| **Hai mapper phân kỳ → grouping im lặng không hoạt động ở browser mode** | Cao | Bước 0c là cổng bắt buộc; ưu tiên xoá mapper trùng thay vì đồng bộ 2 bản |
| Shape labels khác nhau giữa bollard và CLI | Cao | Chuẩn hoá ở Rust; test so sánh JSON 2 đường |
| Cám dỗ dựng lại DataTable giữa chừng | TB | Nếu thấy cần bảng chung, ghi lại làm plan riêng khi đã có ≥3 consumer thật |
| `settingsStore` nuốt lỗi ghi → mất trạng thái toggle | Thấp | Kiểm tra kết quả ghi; không im lặng bỏ qua |
