---
phase: 4
title: "Trang topology mới"
status: done
priority: P3
dependencies: []
effort: ""
---

# Phase 4: Trang topology mới

> **Viết lại hoàn toàn sau red-team 2026-08-11.** Bản đầu mô tả việc "hợp nhất `ClusterTopology.svelte` và `XRay.svelte` rồi xoá chúng". Tiền đề đó sai và việc làm theo sẽ gây hồi quy.

## Overview

Một trang **mới** vẽ đồ thị tầng Docker: container ↔ network ↔ volume ↔ compose project ↔ image, trong phạm vi engine hiện tại.

## Đính chính: hai file kia không phải thứ tôi tưởng

| File | Thực tế | Bằng chứng |
|---|---|---|
| `src/pages/XRay.svelte` | Đồ thị **tài nguyên Kubernetes**, nhận prop `namespace` | `XRay.svelte:3-15` |
| `src/pages/ClusterTopology.svelte` | Liệt kê **colima instance** cho ngữ cảnh k8s | `ClusterTopology.svelte:3-11` |
| Cả hai | **Component con** của `Kubernetes.svelte`, không phải route | `Kubernetes.svelte:13,14,72,576-577,579` |
| `App.svelte` | 13 page import, **không có** cái nào trong hai file trên | `App.svelte:8-20` |
| `Sidebar.svelte` | 13 menu id, **không có** `topology`/`xray` | `Sidebar.svelte:31-58` |
| Locale | grep `xray\|topology`: **0 kết quả** | `src/locales/en.json` |

**Kết luận:** không có gì để hợp nhất, không có route để gỡ, không có chuỗi locale mồ côi. Làm theo bản plan cũ sẽ **xoá mất hai view Kubernetes duy nhất** — trong khi chính phase đó tuyên bố k8s ngoài phạm vi.

**Quy tắc cho phase này: không đụng vào hai file đó.** Chúng nằm trong lãnh địa Kubernetes; nếu sau này muốn thống nhất trải nghiệm đồ thị k8s và Docker thì đó là plan riêng, có khảo sát riêng.

## Requirements

**Functional**
- Node: container, network, volume, compose project, image.
- Edge: container→network (attached), container→volume (mount), container→compose project, container→image.
- Click node → panel chi tiết + hành động nhanh (start/stop/logs).
- Lọc theo loại node, tìm theo tên, làm nổi bật hàng xóm khi hover.
- Màu theo trạng thái: running / stopped / unhealthy.

**Ngoài phạm vi**
- Tài nguyên Kubernetes (đã có `XRay.svelte` lo).
- Nhiều instance. Bản đầu có tham số `?instance=` cho Pro P5 — **Pro P5 đã bị cắt**, và không tồn tại lớp targeting đa instance nào (`docker --context`: 0 lần trong repo). Tham số đầu cơ đó bị bỏ.

**Non-functional**
- Mượt ở 50 container, 10 network. Không cần lo tới 1000 node.
- Không thêm dependency đồ thị nếu SVG thuần đủ.

## Architecture

```
GET /api/topology  → routes/topology.rs → commands/topology.rs
   gộp: list_containers + list_networks + list_volumes + compose ps
   → TopologyGraph { nodes: Vec<Node>, edges: Vec<Edge> }

UI: src/pages/Topology.svelte      ← TRANG MỚI
   ├─ route mới trong App.svelte + mục mới trong Sidebar.svelte
   ├─ layout: force-directed tự viết (~150 dòng)
   └─ render SVG
```

Gom dữ liệu ở **backend trong một endpoint**, không để UI gọi 4 API rồi ghép — giảm round-trip và giữ logic quan hệ ở một chỗ.

**Về thư viện layout:** kiểm tra `package.json` trước. Nếu `d3-force` chưa có thì tự viết — 50 node không cần thư viện, và thêm dependency cho việc này là đánh đổi tệ.

## Related Code Files

- Create: `src-tauri/src/commands/topology.rs`
- Create: `src-tauri/src/routes/topology.rs`
- Create: `src/pages/Topology.svelte`
- Create: `src/components/topology/GraphCanvas.svelte`, `src/components/topology/NodeDetail.svelte`
- Create: `src/lib/api/topology.ts`
- Modify: `src/App.svelte` (thêm route — **thêm**, không sửa cái có sẵn)
- Modify: `src/components/Sidebar.svelte` (thêm mục menu)
- Modify: `src-tauri/src/commands/mod.rs`, `src-tauri/src/routes/mod.rs`, `src-tauri/src/api_server.rs`
- Modify: `src/locales/{en,vi,ja,zh}.json`
- **KHÔNG đụng:** `src/pages/XRay.svelte`, `src/pages/ClusterTopology.svelte`, `src/pages/Kubernetes.svelte`

## Implementation Steps

1. `TopologyGraph` types phía Rust: `Node { id, kind, label, status, meta }`, `Edge { from, to, kind }`.
2. `commands/topology.rs`: gọi các command đã có, dựng graph. Tái dùng `docker_state.rs`, không gọi docker trực tiếp.
3. Endpoint `GET /api/topology` (không tham số instance).
4. Layout engine ở frontend, tách module riêng để test độc lập.
5. `GraphCanvas.svelte`: SVG, pan/zoom, hover highlight.
6. `NodeDetail.svelte`: panel bên phải, tái dùng action đã có ở `Containers.svelte`.
7. Thêm route + mục sidebar + chuỗi 4 locale.
8. Kiểm tra hồi quy: trang Kubernetes vẫn hoạt động y nguyên.

## Success Criteria

- [x] Trang topology mới hiển thị đúng container/network/volume/compose của engine hiện tại.
- [x] 50 container + 10 network: layout ổn định dưới 1 giây, pan/zoom mượt.
- [x] Click node → start/stop chạy đúng, graph cập nhật.
- [x] `XRay.svelte`, `ClusterTopology.svelte`, `Kubernetes.svelte` **không bị sửa**; trang Kubernetes hoạt động y như trước.
- [x] Không có tham số `instance` nào trong endpoint.
- [x] Không thêm dependency đồ thị mới (trừ khi khảo sát bước 4 chứng minh cần).

## Risk Assessment

| Rủi ro | Giảm thiểu |
|---|---|
| **Lặp lại lỗi cũ: đụng vào component Kubernetes** | Danh sách "KHÔNG đụng" tường minh + criteria hồi quy |
| Trùng lặp khái niệm với XRay khiến người dùng bối rối | Đặt tên và mô tả rõ: đây là đồ thị **Docker**, XRay là đồ thị **Kubernetes** |
| Force layout nhảy loạn | Cố định seed, ghim node khi kéo, nút reset layout |
| Phình phạm vi thành "graph cho mọi thứ" | Giới hạn 5 loại node đã liệt kê |
| Thêm dependency nặng cho việc nhỏ | Kiểm tra `package.json`; tự viết nếu chưa có |

## Kết quả thực thi — 2026-08-12

Đã build và verify. Bằng chứng:

- `cargo test --lib`: 182/182 pass (4 test mới ở `commands/topology.rs`).
- `vitest run`: 133/133 pass (16 test mới: `lib/topology-layout.test.ts` 10, `components/topology/GraphCanvas.test.ts` 6).
- `pnpm run build`: pass. `svelte-check`: về đúng baseline 143 lỗi/36 file, **0 lỗi trong file của phase này**.
- Layout ở 84 node/127 edge: 400 iteration trong ~16ms; drift 2.5px sau 50 iteration nữa (đã settle); deterministic; edge dài trung bình 121.5px so với k=110; cặp node gần nhất 47.8px (không chồng).
- Luật dựng edge validate trên docker thật (7 container, 5 network, 14 volume): 7 network edge khớp hết, 21 volume edge, 11 bind mount bị loại đúng thay vì thành node volume giả, 6 compose project edge.
- `XRay.svelte`, `ClusterTopology.svelte`, `Kubernetes.svelte` không bị sửa (`git status` sạch). Không thêm dependency đồ thị. Endpoint không có tham số `instance`.

### Hai bug được code review phát hiện và đã sửa

1. **`$effect` đọc state nó vừa ghi** → `effect_update_depth_exceeded`, trang không mở được. `fitToView()` đọc `layout` và toạ độ node ngay trong effect đã gán `layout`. Sửa: `layout` không còn là `$state` (redraw do biến `tick` điều khiển), thân effect chạy trong `untrack`, và effect chỉ phụ thuộc vào khoá hình dạng đồ thị. Nguyên nhân sót: verification ban đầu chỉ test layout dưới dạng module thuần, chưa bao giờ mount component — nay đã có test mount thật.
2. **Edge trùng làm sập keyed `{#each}`** (`docker run -v cache:/a -v cache:/b` → `Mounts: "cache,cache"`). Sửa ở cả hai lớp: backend dedupe theo cặp, renderer cũng dedupe vì một edge trùng ở bất kỳ đâu sẽ hạ cả trang.

Ngoài ra: partial failure của `network ls`/`volume ls` giờ trả về qua `warnings` thay vì im lặng thành "container không attach gì".

### Lệch khỏi plan gốc, có lý do

- **Compose project node lấy từ label container**, không gọi thêm `docker compose ls`. `compose ls` chỉ báo project đang có container nên kết quả tương đương, mà tiết kiệm một tiến trình và bảo đảm mọi project node đều có ít nhất một edge.
- **Thêm vào `vitest.config.ts`**: `resolve: { conditions: ['browser'] }`. Không có nó, `import ... from 'svelte'` giải về bản server và `mount()` throw, tức không thể test component nào. Toàn bộ 133 test vẫn pass sau thay đổi này.
- **Thêm icon `Topology` vào `Icons.svelte`** thay vì dùng lại `Icons.Network` — hai mục sidebar cạnh nhau cùng icon thì không phân biệt được.
