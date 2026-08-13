---
phase: 5
title: "Bố cục đồ thị bằng ELK + nhóm theo network"
status: done
priority: P3
dependencies: [4]
effort: ""
---

# Phase 5: Bố cục đồ thị bằng ELK

Nâng cấp trang topology (phase 4 đã Done) từ force-directed tự viết sang **ELK layered** — thuật toán mà DockGraph dùng. Phân tích đầy đủ: `plans/reports/from-sequential-thinking-to-owner-260812-1049-dockgraph-elk-graph-upgrade-report.md`.

## Ràng buộc license (đọc trước khi code)

DockGraph phát hành theo **BSL 1.1**, cấm *"embedding it as a feature in a commercial product"*. Colima-UI có Pro tier → **không được** copy code, không được bundle image, không được port file frontend của họ sang Svelte. Chỉ mượn **ý tưởng** và dùng **thư viện bên dưới** (`elkjs`, EPL-2.0).

## Vấn đề cần giải

`src/lib/topology-layout.ts` (force-directed, 283 dòng):
- Không tất định — mỗi lần mở ra một bố cục khác.
- Node dịch chuyển mỗi lần poller trả dữ liệu mới.
- Không có khái niệm nhóm; container cùng network nằm rải rác.

## Phạm vi

**Trong phạm vi**
- Thêm `elkjs` (^0.12.0), chạy trong Web Worker.
- Adapter `src/lib/topology-elk.ts`: `TopologyGraph` → ELK graph → toạ độ tuyệt đối cho từng node.
- Compound node: gom container theo **primary network**; cạnh sang network phụ vẽ xuyên nhóm.
- Ba loại cạnh phân biệt bằng style: network / volume mount / `depends_on`.
- Node service compose **chưa chạy** (style mờ), lấy từ `routes/compose.rs`.
- Màn hình third-party notices: text EPL-2.0 + link source elkjs.

**Ngoài phạm vi**
- `@xyflow/svelte`. Renderer SVG hiện tại đã chạy và đã có test — không viết lại để lấy minimap.
- Cập nhật theo Docker event stream (cần subscriber registry trong `sse.rs`, xem report 0958).
- Table view — tách phase riêng nếu muốn.

## Files

| File | Việc |
|---|---|
| `package.json` | + `elkjs` |
| `src/lib/topology-elk.ts` | **mới** — adapter + khởi tạo worker |
| `src/lib/topology-elk.test.ts` | **mới** — map node/edge, nhóm network, xử lý node mồ côi |
| `src/lib/topology-layout.ts` | **xoá** sau khi adapter chạy; giữ `layoutBounds` nếu `fitToView` còn dùng |
| `src/lib/topology-layout.test.ts` | xoá theo |
| `src/components/topology/GraphCanvas.svelte` | bỏ vòng `requestAnimationFrame`/`stepLayout`; nhận toạ độ từ Promise; vẽ khung nhóm |
| `src-tauri/src/commands/topology.rs` | + node service chưa chạy, + cạnh `depends_on`, gắn `primary_network` cho container |

## Các bước

1. Cài `elkjs`, dựng worker, xác nhận `elk.layout()` chạy trong Tauri webview (bundle offline, không CDN).
2. Viết adapter + test trước khi đụng component (dữ liệu vào/ra thuần, dễ test).
3. Đổi `GraphCanvas` sang toạ độ tất định: layout chạy khi dữ liệu đổi, không chạy mỗi frame. Giữ nguyên pan/zoom, highlight hàng xóm, `NodeDetail`.
4. Backend: bổ sung `primary_network`, service chưa chạy, cạnh `depends_on`.
5. Vẽ khung nhóm network trong SVG.
6. Third-party notices.

## Kiểm chứng

- `pnpm vitest run src/lib/topology-elk.test.ts src/components/topology/`
- Bố cục **tất định**: mở lại cùng dữ liệu → cùng toạ độ.
- Không nhảy node khi poll trả dữ liệu không đổi.
- 50 container / 10 network: layout < 300ms, UI thread không bị chặn.
- `pnpm svelte-check`, `cargo test`.

## Rủi ro

| Rủi ro | Mức | Xử lý |
|---|---|---|
| elkjs +~1.4MB bundle | Medium | App desktop, tải một lần — chấp nhận. Nếu từ chối thì huỷ phase, chỉ thêm nhóm network vào force layout. |
| Worker không chạy trong webview Tauri | Medium | Kiểm ở bước 1 **trước** khi làm tiếp; fallback: gọi `elk.bundled.js` đồng bộ trên main thread (50 node vẫn kịp). |
| Nghĩa vụ EPL-2.0 | Low | Không sửa source elkjs; kèm license + link. |
| Hồi quy tương tác đang chạy | Low | Chỉ đổi nguồn toạ độ, không đổi renderer. |

## Kết quả thực thi (2026-08-12)

**Đã làm**
- `elkjs@0.12.0`. Worker chạy được qua Vite (`elk-worker.min-*.js` tách chunk riêng, 1.42MB, lazy).
- `src/lib/topology-elk.ts` + 14 test hàm thuần. `topology-layout.ts` và test của nó đã xoá.
- `GraphCanvas.svelte`: bỏ vòng `requestAnimationFrame`; nhóm network vẽ thành box; kéo node chỉ dời đúng node đó (lưu ở `overrides`, sống qua poll).
- `topology.rs`: thêm `primaryNetwork` vào meta container.
- Settings: mục "Open Source Licenses" (EPL-2.0 cho elkjs).

**Kiểm chứng**: 139/139 test frontend pass · `cargo test --lib topology` 4/4 pass · `pnpm build` OK · svelte-check 0 lỗi ở file đã sửa (143 lỗi còn lại là có sẵn ở 36 file khác).

**Khác với plan**
- Giữ luôn `elk.bundled.js` làm fallback khi không có Worker (+1.4MB đĩa). Lý do: CSP của Tauri có thể chặn spawn Worker, và đây cũng là đường mà test chạy. Không tải lúc khởi động — cả hai đều là dynamic chunk.
- Node service compose chưa chạy + cạnh `depends_on`: **chưa làm**. Cần parse YAML từ `config_files` của `docker compose ls` — đọc file ngoài daemon, là một trust boundary riêng. Tách thành phase sau.
- Cạnh container→network chính bị bỏ khi vẽ (containment đã thể hiện quan hệ đó); network phụ vẫn có cạnh.

**Nợ môi trường (không do phase này)**: `pnpm vitest` chết với Node 20.19.1 (`jsdom@30` cần Node ≥22 — `webidl.util.markAsUncloneable`). Phải chạy bằng Node 24. Nên chốt `engines` trong `package.json` hoặc hạ `jsdom`.
