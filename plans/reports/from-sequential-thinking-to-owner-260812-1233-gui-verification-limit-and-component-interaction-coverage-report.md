# Giới hạn verify GUI, và phần thay thế làm được

Ngày: 2026-08-12 12:33 · Nhánh `dev` · Plan: `plans/archive/260811-2241-free-tier-foundation/` (phase 1, 2, 3)

Yêu cầu: "tối ưu cho các phase bạn đề cập". Lượt trước đã đóng hai tiêu chí đo được (CPU collector 11.16% → 0.14%, và URL prefill GitHub). Thứ duy nhất còn để ngỏ là **click GUI thật** — nên lượt này là về nó.

## Kết luận trước: GUI gap không đóng được ở đây, và tôi không nới auth để đóng nó

Đã điều tra ba đường:

| Đường | Vì sao không đi được |
|---|---|
| Drive cửa sổ Tauri native | WKWebView trên macOS không mở CDP; không có endpoint để attach |
| Browser mode + headless Chromium | Cần API token. Token là CSPRNG sinh trong process, **chỉ lấy được qua Tauri IPC** — không env var, không file, không endpoint (`auth.rs:40-77`) |
| Backend headless rồi drive frontend | Không có bin target nào chạy `start_api_server()`; `cli.rs` và `cui.rs` đều không |

Playwright browsers có trong cache máy nhưng không phải dependency của repo.

Đường thứ hai chỉ mở được nếu tôi thêm env override vào `get_api_token()`. **Tôi không làm.** Đó chính là thứ tiên quyết P1 ("hardening token API cục bộ") vừa siết lại, và nới nó để test của tôi dễ hơn là đánh đổi sai chiều — một lỗ hổng thật để đổi một dấu tick.

Nên: **muốn đóng thì cần `pnpm tauri dev` chạy lên và có người bấm.** Không có cách nào khác mà không hạ trust boundary.

## Phần làm được: logic của việc click thì mount và bấm được

Phần lớn giá trị của một cú click không nằm ở pixel mà ở *quyết định* component đưa ra. Cái đó test được — cùng kỹ thuật đã bắt lỗi `effect_update_depth_exceeded` ở `GraphCanvas` lượt trước, thứ mà mọi test tầng dữ liệu đều không thấy.

25 test mới, không thêm dependency nào (dùng `fireEvent` có sẵn thay vì `@testing-library/user-event`):

### `BundlePreview.test.ts` — 8 test, phase 2

Ưu tiên cao nhất vì đây là hành vi liên quan **quyền riêng tư**: bỏ tick một section thì nội dung đó **thật sự biến khỏi** thứ được copy và được lưu. Một checkbox trông như loại trừ mà không loại trừ thì tệ hơn là không có checkbox, và **không test backend nào thấy được** — lựa chọn nằm ở frontend.

Cũng phủ: log section mặc định không tick; nội dung chỉ hiện khi bấm Show và hiện **đầy đủ** (preview mà lược bớt thì không phải thứ để ra quyết định); Save chỉ gửi section đã tick; issue URL không nhét bundle vào; collector lỗi thì hiện lỗi chứ không phải dialog rỗng.

### `TransferDialog.test.ts` — 8 test, phase 1

Cổng chặn trước khi có gì được spawn: Start bị chặn tới khi các ô đủ nghĩa (export không chọn image, copy thiếu một đầu). `destDir` và `fileName` truyền **rời nhau** đúng như confinement cần — nếu ai đó gộp lại thành một đường dẫn thì test này đỏ. Lỗi start hiện **tại chỗ** chứ không chỉ là toast bay qua. Tiến độ của job khác **không** làm nhảy thanh của dialog này. Job bị huỷ được coi là **xong**, không phải thất bại.

### `Sparkline.test.ts` — 9 test, phase 3

Đóng tiêu chí "hiển thị lỗ hổng khi mất mẫu" ở **tầng vẽ**. Trước đó chỉ có ở tầng dữ liệu (`MetricsHistory` lưu `null`) — nhưng việc *bản vẽ* có tôn trọng điều đó hay không là câu hỏi khác, và là câu hỏi người dùng thực sự thấy: một polyline nối qua lỗ hổng khẳng định một mức tải chưa từng được quan sát, và không có cách nào phân biệt nó với dữ liệu thật.

Phủ: đường bị cắt tại **mọi** gap; mẫu đơn lẻ giữa hai gap vẽ thành dấu chấm (bỏ im lặng sẽ đọc thành "không có dữ liệu" trong khi có); chuỗi toàn gap không sập; không NaN/Infinity với chuỗi toàn 0; không ghim trần 100% (CPU vượt 100% trên nhiều core, ghim trần sẽ cắt phẳng đúng phần đáng xem).

## Gates

| | |
|---|---|
| `cargo test --lib` | 285/285 |
| `vitest run` | **181/181** (16 file, +25 test lượt này) |
| `pnpm run build` | pass |
| `svelte-check` | baseline 143 lỗi/36 warning/36 file — 0 từ file của tôi |
| `package.json` | **không thêm dependency nào** cho test |

## Còn lại chưa verify — thật, không phải hình thức

- Bố cục thị giác: không có test nào biết một cái nút bị đè hay chữ bị tràn.
- Hành vi tầng OS: file picker native (`dialog.open`), mở browser (`openExternal`) — cả hai đều bị mock trong test.
- Cảm giác khi dùng: bảng Activity ở 2s có "nhảy" không, dialog có thấy nhanh không.

Ba thứ này cần mắt người. Không có cách vòng.

## Câu hỏi chưa giải quyết

1. Có muốn tôi thử `pnpm tauri dev` để bạn tự bấm, hay để lúc khác? (Build dev profile mất vài phút và mở một cửa sổ trên máy bạn.)
2. `260811-1930-subscription-teams-supabase-schema` gần xong nhưng phase 7 tự ghi **Partial** (chỉ pass offline gate, test mua thật bị chặn) — có coi là đủ để archive không?
3. Toàn bộ khối 4 phase + tối ưu này vẫn **chưa commit**. Muốn chạy `/code-review` trước khi commit không?
