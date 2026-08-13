# DockGraph cho phần Graph view: dùng được gì, không dùng được gì

Ngày: 2026-08-12 · Nhánh: `dev` · Phương pháp: sequential-thinking (explicit)
Nguồn: https://github.com/dockgraph/dockgraph (indexed source: `dockgraph-repo`)
Plan liên quan: `plans/260811-2241-free-tier-foundation/phase-04-…` (status **Done**)

---

## Thought 1/6 — DockGraph không phải "thư viện"

| Thành phần | Công nghệ |
|---|---|
| Backend | Go, Docker Engine API, gorilla/websocket |
| Frontend | React 19, TypeScript, **React Flow**, **ELK.js** |
| Phân phối | Docker image `dockgraph/dockgraph`, cổng 7800 |

Không có package npm, không có crate. Đây là **một ứng dụng self-hosted trọn gói** chạy cạnh Docker daemon. Không có API để "import" vào Tauri app.

## Thought 2/6 [CHẶN] — Giấy phép BSL 1.1 cấm đúng ca dùng của mình

Nguyên văn README: *"The only restriction is offering it as a hosted service or **embedding it as a feature in a commercial product**."* Chuyển sang Apache-2.0 sau 4 năm kể từ ngày phát hành mỗi phiên bản.

Colima-UI có Pro tier + billing Polar → **là commercial product**. Do đó:

- Nhúng code DockGraph (Go hoặc React) vào app: **vi phạm**.
- Bundle image DockGraph rồi cho user mở trong app: **vi phạm** (embed as a feature).
- Chép nguyên file frontend của họ sang Svelte: vẫn là derivative work → **vi phạm**.
- Đọc để học kiến trúc, học ý tưởng tính năng: **được** (ý tưởng không được bảo hộ bởi license).

Kết luận: câu hỏi "dùng thư viện này" phải đổi thành "**dùng những thư viện mà DockGraph dùng**".

## Thought 3/6 — Thứ thật sự tạo ra chất lượng đồ thị của DockGraph là ELK, không phải React Flow

React Flow lo pan/zoom/handle/minimap. Vị trí node — thứ quyết định đồ thị đọc được hay không — do **ELK layered algorithm** tính. DockGraph nhóm container theo primary network; đó là `elk.layered` + compound node, không phải force simulation.

Đối chiếu hiện trạng repo (`src/lib/topology-layout.ts:93-263`): force-directed tự viết, chạy từng `stepLayout` trong `requestAnimationFrame` (`GraphCanvas.svelte:223`). Đặc tính: **không tất định** (mỗi lần mở một bố cục khác), node nhảy khi poll trả dữ liệu mới, không có khái niệm nhóm.

## Thought 4/6 [HYPOTHESIS → chốt] — Thay layout, giữ renderer

Hai lựa chọn thay thế:

| | A. `@xyflow/svelte` + `elkjs` | B. **chỉ `elkjs`**, giữ SVG hiện có |
|---|---|---|
| License | MIT + EPL-2.0 | EPL-2.0 (giữ nguyên, không sửa source) |
| Phải bỏ đi | `GraphCanvas.svelte` 374 dòng + test 5.3KB | chỉ `topology-layout.ts` 283 dòng |
| Được thêm | minimap, edge routing sẵn, hệ node/handle | bố cục tất định, nhóm theo network |
| Bundle | ~1.4MB (elkjs) + ~400KB (xyflow) | ~1.4MB |
| Rủi ro hồi quy | Cao — viết lại toàn bộ tương tác đang chạy | Thấp — thay đúng một module |

**Chọn B.** Renderer SVG hiện tại đã chạy, đã có test, đã có NodeDetail + highlight hàng xóm. Đổi sang xyflow là viết lại thứ đang hoạt động để lấy minimap — không đáng (YAGNI). Giá trị nằm ở bố cục.

ELK chạy được ngoài React: `elkjs/lib/elk.bundled.js`, API `new ELK().layout(graph)` trả Promise, hoàn toàn framework-agnostic. Chạy trong Web Worker (`elk-worker`) để không chặn UI thread.

## Thought 5/6 — Ý tưởng tính năng lấy được miễn phí từ DockGraph

Xếp theo tỉ lệ giá trị/công:

1. **Nhóm container theo primary network** — trực tiếp là compound node của ELK, đi kèm luôn với việc đổi layout.
2. **Compose-aware ghost services** — parse compose file, hiện service *chưa chạy* với style mờ. Repo đã có `routes/compose.rs`; đây là node type mới, không cần backend mới.
3. **Cạnh `depends_on`** — đọc từ compose, khác loại với cạnh network/volume.
4. **Table view** cùng dữ liệu, có group + sort — rẻ, không cần đồ thị.
5. **Cập nhật theo Docker event stream** thay vì poll — *không* nên làm bây giờ: đụng `poller.rs` + `sse.rs`, và `sse.rs` còn đang thiếu subscriber registry (đã ghi trong report 0958). Để sau.

## Thought 6/6 [FINAL] — Đề xuất

- **Không** dùng DockGraph dưới bất kỳ dạng nào (BSL 1.1 chặn).
- Thêm `elkjs` (dev→prod dependency), thay `topology-layout.ts` bằng adapter gọi ELK trong worker, giữ nguyên `GraphCanvas.svelte`/`NodeDetail.svelte`/`Topology.svelte`.
- Bổ sung nhóm theo network + node service chưa chạy + cạnh `depends_on`.
- Ghi phase mới: `phase-05-graph-layout-elk.md` (phase 4 đã Done, không sửa lịch sử).

### Nghĩa vụ license khi bundle elkjs

EPL-2.0 là copyleft mức file: giữ nguyên file `elk.bundled.js` (không patch), kèm text license + link source trong màn hình About/third-party notices. Nếu sau này phải sửa elkjs → phải công bố phần sửa. Không ảnh hưởng code của mình.

---

## Câu hỏi chưa chốt

1. Có chấp nhận +1.4MB bundle cho elkjs không, hay muốn giữ force layout tự viết và chỉ thêm nhóm theo network thủ công?
2. Màn hình third-party notices đã tồn tại chưa? Nếu chưa, phải dựng cùng phase này.
3. Compose ghost services: lấy compose file từ đâu — `docker compose ls` của engine, hay cho user trỏ file? DockGraph mount thư mục; app desktop thì khác.
