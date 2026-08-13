---
phase: 6
title: "Ngôn ngữ hình ảnh cho Topology: icon, chip node, rail chú giải, cụm điều khiển"
status: done
priority: P3
dependencies: [4, 5]
effort: ""
---

# Phase 6: Ngôn ngữ hình ảnh cho Topology

Phase 4 dựng trang, phase 5 sửa bố cục. Còn lại là thứ người dùng nhìn thấy: **node vẽ bằng hình khối trừu tượng, không icon, không chú giải**.

## Chẩn đoán — ba lỗi thiết kế cụ thể

### 1. Đồ thị tự chế ngôn ngữ hình ảnh thứ hai cho cùng năm khái niệm

`Icons.svelte` đã có glyph cho `Container`, `Network`, `Volume`, `Image`, `Compose` — dùng ở sidebar, ở NodeDetail, ở mọi trang khác. Nhưng `GraphCanvas.svelte:300-311` vẽ:

| Loại | Hình trong đồ thị | Icon ở phần còn lại của app |
|---|---|---|
| container | chữ nhật bo góc | thùng hàng có 3 vạch |
| network | hình tròn | 3 chấm nối hình chữ Y |
| volume | hình thoi | khối lục giác |
| project | vuông bo góc lớn | 4 ô vuông + đường chéo |
| image | hình tròn nhỏ | 3 đĩa xếp chồng |

Người dùng phải học **hai** bộ ký hiệu cho cùng năm thứ. Tệ hơn: network và image đều là hình tròn, chỉ khác bán kính 22 vs 15 — ở mức zoom 0.5 (mức mặc định cho 50 node) hai thứ đó không phân biệt được. Đây là lỗi nặng nhất và cũng là lỗi rẻ nhất để sửa: icon đã có sẵn, chỉ là chưa ai đặt vào.

### 2. Trạng thái chỉ mã hoá bằng màu

`statusColor` gán xanh/xám/đỏ vào `stroke`. Người mù màu đỏ-lục — 8% nam giới — không phân biệt được container **running** với **unhealthy**. Đó chính là cặp quan trọng nhất trên màn hình này. Cần kênh thứ hai (hình dạng), không phải chỉnh sắc độ.

### 3. Không có chú giải, và affordance thì vô hình

- Không chỗ nào nói hình thoi = volume, xanh = running.
- Zoom bằng lăn chuột, kéo node, kéo nền: cả ba đều không có dấu hiệu nào trên màn hình.
- `Reset layout` là nút chữ nằm ở header — xa canvas, trong khi nó là hành động của canvas.
- Băng filter là 5 checkbox. Checkbox tốn một dải chiều cao ngang với rail chú giải mà **không** mang thông tin chú giải nào.

## Quyết định thiết kế

### A. Node = chip có icon, không phải hình khối + nhãn treo dưới

**Chọn chip** thay vì "giữ hình khối, thêm icon nhỏ bên cạnh". Lý do: hình khối + icon là *ba* thứ phải đọc (hình, icon, nhãn) để biết một node là gì. Chip gộp còn một: icon là ký hiệu loại, nhãn nằm ngay cạnh nó, viền là trạng thái. Nhãn treo dưới hình cũng là nguồn chồng chữ khi đông node — đưa vào trong chip thì hộp ELK cấp phát đúng bằng thứ được vẽ.

```
┌─────────────────────────────┐
│▌ [icon]  my-container    ● │   ← 30px cao
└─────────────────────────────┘
 ▲          ▲               ▲
 │          │               └ dấu trạng thái (kênh hình dạng)
 │          └ nhãn, cắt ở 20 ký tự
 └ sọc trái 3px, màu theo trạng thái
```

| Thuộc tính | Giá trị |
|---|---|
| Cao | 30, bo góc 6 |
| Icon | 14px tại x=8, màu theo **loại** |
| Nhãn | font 11, bắt đầu x=28, cắt 20 ký tự + `…` |
| Rộng | `28 + độ_rộng_nhãn + 22`, kẹp trong `[104, 224]` |
| Nền | `--bg-secondary`, viền 1px `--border-primary` |
| Sọc trái | 3px, màu theo **trạng thái** |

Màu icon theo loại (kênh nhận dạng), tách khỏi màu trạng thái (kênh sức khoẻ) — hai kênh không tranh nhau:

| Loại | Token |
|---|---|
| container | `--accent-blue` |
| network | `--color-info` |
| volume | `--color-warning` |
| project | `--accent-purple` |
| image | `--text-muted` |

### B. Trạng thái mã hoá bằng **hình dạng + màu**, không chỉ màu

| Trạng thái | Dấu bên phải chip | Sọc trái |
|---|---|---|
| running | chấm đặc r3 | `--color-success` |
| stopped | vòng rỗng r3 | `--text-muted` |
| unhealthy | tam giác 7px | `--color-danger` |
| none (network/volume/image) | không có dấu | viền trung tính |

Đọc được khi in đen trắng, đọc được với mọi kiểu mù màu.

### C. Box network có thanh tiêu đề

Hiện tại chỉ là khung nét đứt + chữ. Thêm thanh 26px: icon Network 12px + tên + số container bên phải. Box trở thành một thực thể có danh tính thay vì một vùng nền mơ hồ.

```
┌ [⑃] bridge                      4 ┐
│  ┌──────────┐  ┌──────────┐       │
│  │▌[▤] api ●│  │▌[▤] web ●│       │
│  └──────────┘  └──────────┘       │
└───────────────────────────────────┘
```

### D. Băng filter → **rail vừa lọc vừa chú giải**

Một dải, hai việc. Không tốn thêm chiều cao so với 5 checkbox hiện tại.

```
[▤ Containers 12] [⑃ Networks 3] [⬡ Volumes 5] [⊞ Projects 2] [◎ Images 8]     ● running  ○ stopped  ▲ unhealthy
└──────────────── chip bật/tắt: mờ đi khi tắt ────────────────┘                 └── chú giải trạng thái ──┘
```

Mỗi chip mang **đúng** icon và **đúng** màu loại mà đồ thị đang dùng. Chú giải không phải là thứ phải bảo trì riêng — nó chính là cái điều khiển. Checkbox biến mất; trạng thái bật/tắt thể hiện bằng độ mờ + viền, như các chip filter khác trong app.

### E. Cụm điều khiển nổi trên canvas, góc dưới-trái

`[＋] [－] [⤢ fit] [↻ reset]` — dọc, nền mờ, cách mép 12px.

Đưa zoom thành nút hiện hữu là cách duy nhất để người dùng biết canvas zoom được. `Reset layout` rời khỏi header về đây vì nó thuộc về canvas. Header còn lại: ô tìm kiếm + nút refresh.

### F. Hướng bố cục: giữ `RIGHT`

Trái→phải đọc là "container gắn vào những gì": nhóm network thành cột trái, volume/image/project chảy sang phải. Không đổi.

### G. Tham chiếu chéo — nối đồ thị với phần còn lại của app

Đồ thị hiện là ngõ cụt: thấy `my-api` gắn vào `pgdata` thì vẫn phải tự sang trang Volumes tìm lại. Ba hướng nối, dùng **đúng khuôn mẫu bàn giao một-lần đã có** trong `store.svelte.ts:58,67` (`helpArticle`, `settingsSection`): bên gọi đặt giá trị rồi mới đổi trang, trang đích đọc xong thì xoá.

**G1. Trong NodeDetail: danh sách hàng xóm bấm được** — giá trị lớn nhất, rẻ nhất, không rời trang.

```
my-api                                    [container]
─────────────────────────────────────────────────────
Networks    ⑃ bridge          ⑃ backend
Volumes     ⬡ pgdata
Image       ◎ node:20-alpine
Project     ⊞ shop
```

Bấm một mục → `selectedId` nhảy sang node đó, canvas làm nổi bật nó. Dữ liệu đã có sẵn trong `graph.edges`, không cần API mới. Đây là thứ biến đồ thị từ tranh tĩnh thành công cụ duyệt.

**G2. Nút "Mở trong <trang>"** ở đầu NodeDetail → sang trang chuyên trách của loại đó, chọn sẵn tài nguyên:

| Loại node | Trang đích (`uiState.currentPage`) |
|---|---|
| container | `containers` |
| network | `networks` |
| volume | `volumes` |
| image | `images` |
| project | `compose` |

**G3. Chiều ngược lại: "Xem trong Topology"** ở Containers / Networks / Volumes / Images / Compose → mở topology với node đó chọn sẵn và canvas căn vào nó.

**Cơ chế** — thêm một trường vào `uiState`, không phải năm:

```ts
/**
 * Tài nguyên cần chọn sẵn ở lần ghé trang tới. Cùng hợp đồng một-lần với
 * `helpArticle`: người gọi đặt rồi mới đổi trang, trang đích đọc xong xoá.
 * `id` là id node đã gắn tiền tố loại ("volume:pgdata") khi đích là Topology,
 * hoặc id gốc của tài nguyên khi đích là trang danh sách.
 */
focusResource: null as { page: string; id: string } | null,
```

Trang danh sách hiện **không nhận prop nào** (`Containers.svelte` … `Compose.svelte`: 0 `$props`), nên mỗi trang phải tự đọc `uiState.focusResource` trong `onMount`. Đó là 5 chỗ sửa nhỏ và giống hệt nhau — chấp nhận được, và rẻ hơn việc đổi chữ ký của cả năm trang.

**Thứ tự làm**: G1 trước (không đụng router, giá trị cao nhất). G2/G3 sau, và nếu phải cắt scope thì cắt G3 — chiều từ trang danh sách vào đồ thị ít cấp bách hơn chiều ngược lại.

## Files

| File | Việc |
|---|---|
| `src/components/Icons.svelte` | + `ZoomIn`, `ZoomOut`, `Fit` (3 glyph, cùng phong cách stroke 2px/24px viewBox) |
| `src/components/topology/GraphCanvas.svelte` | node → chip có icon; dấu trạng thái; thanh tiêu đề box; cụm điều khiển nổi |
| `src/lib/topology-elk.ts` | `NODE_SIZE` theo chip (cao 30, rộng tính từ nhãn); `LABEL_CHAR_WIDTH` 5.6→6.0 (font 10→11); padding trên của group 34→38 cho thanh tiêu đề |
| `src/pages/Topology.svelte` | băng checkbox → rail chip + chú giải; bỏ nút `Reset layout` ở header; đọc + xoá `uiState.focusResource` khi mount (G3) |
| `src/components/topology/NodeDetail.svelte` | danh sách hàng xóm bấm được (G1); nút "Mở trong <trang>" (G2) |
| `src/store.svelte.ts` | + `focusResource` (G2/G3) |
| `src/pages/{Containers,Networks,Volumes,Images,Compose}.svelte` | đọc + xoá `focusResource` khi mount; nút "Xem trong Topology" (G2/G3) |
| `src/lib/topology-elk.test.ts` | cập nhật kỳ vọng kích thước |
| `src/components/topology/GraphCanvas.test.ts` | khẳng định mỗi node vẽ đúng icon của loại; dấu trạng thái đúng hình |
| `src/locales/{en,vi,ja,zh}.json` | `topology.zoom_in`, `zoom_out`, `fit`, `legend_running`, `legend_stopped`, `legend_unhealthy`, `open_in`, `view_in_topology`, `related` |

## Kiểm chứng

- `pnpm vitest run src/lib/topology-elk.test.ts src/components/topology/`
- Đọc được ở zoom 0.5 với 50 container: nhãn không chồng, icon vẫn phân biệt được.
- Ảnh chụp màn hình chuyển sang thang xám: vẫn phân biệt được running / stopped / unhealthy.
- Bốn locale đủ chuỗi mới.
- G1: bấm một hàng xóm trong NodeDetail → node đó được chọn, panel đổi nội dung, không đổi trang.
- G2/G3: `focusResource` bị **xoá** sau khi tiêu thụ — quay lại trang đó lần nữa phải không còn chọn sẵn gì (đúng hợp đồng một-lần của `helpArticle`).
- `pnpm svelte-check` không phát sinh lỗi mới ở file đụng tới.

## Rủi ro

| Rủi ro | Mức | Xử lý |
|---|---|---|
| Chip rộng hơn hình tròn → đồ thị nở ngang | Medium | Kẹp rộng ở 224 + cắt nhãn; ELK vốn xếp theo lớp nên nở ngang là chiều nó xử lý tốt nhất |
| `{@html Icons.X}` bên trong `<svg>` | Medium | Icon là chuỗi `<svg>` lồng — hợp lệ trong SVG2, nhưng phải đặt trong `<foreignObject>` hoặc chuyển sang `<symbol>`+`<use>`. **Chốt: `<symbol>` + `<use>`**, định nghĩa một lần trong `<defs>`, tránh lặp 50 lần cùng một path |
| 5 màu loại + 3 màu trạng thái = 8 màu trên một màn hình | Low | Màu loại chỉ tô icon 14px; sọc/dấu trạng thái mới là thứ bão hoà. Không tô nền chip theo loại |

## Kết quả thực thi (2026-08-12)

**Đã làm — toàn bộ A–G**
- `Icons.svelte`: + `ZoomIn`, `ZoomOut`, `Fit`.
- `GraphCanvas.svelte`: `<defs>` chứa 5 `<symbol>`, node stamp bằng `<use>` — **một** bản path cho 50 node, không phải 50. Chip 30px: sọc trạng thái trái, icon màu theo loại, nhãn trong chip, dấu trạng thái (chấm đặc / vòng rỗng / tam giác) bên phải. Box network có thanh tiêu đề + số container. Cụm `＋ － ⤢ ↻` nổi góc dưới-trái.
- `topology-elk.ts`: kích thước chip tính từ nhãn đã cắt (`displayLabel`), `sizes` map trả về để renderer vẽ đúng thứ ELK cấp; `count` cho group; padding trên 38.
- `Topology.svelte`: rail chip lọc-kiêm-chú giải + 3 dấu trạng thái; `Reset layout` rời header.
- **G1** `NodeDetail`: danh sách hàng xóm nhóm theo loại, sắp xếp ổn định, bấm → đổi selection trong đồ thị.
- **G2** nút "Open in <trang>"; Containers + Compose mở sẵn panel chi tiết khi đến.
- **G3** nút "Topology" ở hàng của cả 5 trang danh sách.
- `src/lib/topology-link.ts` (mới): `viewInTopology` + `consumeFocus`. 5 trang cần cùng một cặp thao tác — gom một chỗ thay vì gõ lại 5 lần với 5 cơ hội quên bước xoá.
- 4 locale, 7 khoá mỗi ngôn ngữ.

**Kiểm chứng**: 150/150 test frontend pass (thêm 3 test: mỗi loại có icon riêng, trạng thái mã hoá bằng hình dạng, có 4 nút điều khiển canvas) · `pnpm build` OK · svelte-check **143 lỗi / 36 file — y hệt baseline trước phase**, tức 0 lỗi mới.

**Khác với plan**
- `id` trong `focusResource` **kèm `page`**, không chỉ id trần. Không có nó thì một yêu cầu focus bị người dùng bỏ dở sẽ kích hoạt trên trang bất kỳ họ ghé tiếp theo.
- Node image được tham chiếu bằng `repository:tag`, không phải image id — vì đó là thứ container báo cáo và là thứ đồ thị dùng làm khoá.
- Không tái dùng Set `selected` (checkbox chọn hàng loạt) để làm nổi bật hàng khi đến trang Networks/Volumes/Images: người dùng sẽ hạ cánh với một hàng đã tick rồi bấm "xoá hàng loạt" tưởng là rỗng. Ba trang đó chỉ có chiều đi (G3), chưa có chiều đến — chúng không có khái niệm panel chi tiết để mở.
- Bỏ ý "canvas căn vào node được focus": chọn + làm nổi bật là đủ, và tránh phải giữ handle vào component.

**Nợ còn lại**: node service compose chưa chạy + cạnh `depends_on` → đã xử lý ở phase 7.
