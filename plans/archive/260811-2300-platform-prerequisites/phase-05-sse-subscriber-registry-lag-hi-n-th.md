---
phase: 5
title: SSE subscriber registry + lag hiển thị
status: completed
priority: P1
dependencies: []
effort: ''
---

# Phase 5: SSE subscriber registry + lag hiển thị

## Overview

Hai khiếm khuyết của tầng SSE, cùng chặn thiết kế collector ở Free P3 / Pro P2.

**Khiếm khuyết 1 — không có sổ sách subscriber.** `sse.rs:17-27` là một `broadcast::Sender` process-global duy nhất; `routes/misc.rs:7` subscribe không ghi gì. `receiver_count()` đếm gộp mọi client (docker state, instance, k8s, KB). Không có cách nào biết "có ai đang xem Activity không" — mà đó chính là cơ chế bật/tắt collector tôi đặt làm tiêu chí quan trọng nhất của Free P3.

**Khiếm khuyết 2 — mất mẫu im lặng.** Channel dung lượng 64 (`sse.rs:22`) dùng chung với `sse_docker_watcher` (đẩy cả danh sách container + image mỗi lần đổi trạng thái). Khi client chậm, `BroadcastStream` trả `Lagged(n)`, và `routes/misc.rs:9-12` map thẳng thành `None` — **lỗ hổng dữ liệu vô hình**. Biểu đồ lịch sử vẽ đường thẳng qua chỗ mất mà không ai biết.

## Requirements

**Functional**
- Đếm được subscriber **theo loại sự kiện**, không phải tổng.
- Đăng ký/huỷ tự động theo vòng đời kết nối SSE — client đóng tab hoặc reload là giảm đếm ngay, không cần endpoint unsubscribe.
- `Lagged(n)` phải phát ra một sự kiện nhìn thấy được, không bị nuốt.

**Non-functional**
- Không phá vỡ client SSE hiện có (docker-state-updated, instances-update).
- Đăng ký phải rò rỉ được zero: crash client, mạng đứt, reload — đều phải giảm đếm.

## Architecture

```
sse.rs
  ├─ SSE_TX: broadcast::Sender<SseMessage>       ← giữ nguyên
  ├─ SUBSCRIBERS: Mutex<HashMap<String, usize>>  ← MỚI: đếm theo event topic
  ├─ struct SubscriptionGuard { topics: Vec<String> }
  │     impl Drop → giảm đếm  ← đây là cơ chế chống rò rỉ, không phải endpoint
  ├─ subscribe_topics(&[&str]) -> (Receiver, SubscriptionGuard)
  ├─ subscriber_count(topic: &str) -> usize
  └─ on_subscriber_change: broadcast nội bộ để collector phản ứng

routes/misc.rs
  api_events(topics: Option<Query<...>>)
    ├─ subscribe_topics(...) → giữ guard trong stream
    ├─ Lagged(n) → phát Event "stream-lagged" {dropped: n}  ← KHÔNG nuốt
    └─ guard drop khi stream kết thúc (client đóng) → đếm giảm
```

**Vì sao `Drop` guard chứ không phải endpoint subscribe/unsubscribe:** endpoint rò rỉ mỗi lần client crash hoặc reload trước khi gọi unsubscribe. Guard gắn vào vòng đời stream của Axum — client đóng kết nối thì stream drop, guard drop, đếm giảm. Không có đường nào bỏ sót.

**Về dung lượng channel 64:** nâng lên không giải quyết được gốc, chỉ dời ngưỡng. Cách đúng là (a) làm lag nhìn thấy được, và (b) khi Pro P2 ghi SQLite thì ghi ở **sink phía backend**, không phải từ dữ liệu đi qua SSE — SSE là kênh hiển thị, không phải kênh dữ liệu bền vững. Ghi rõ điều này để Pro P2 không mắc lỗi lấy SSE làm nguồn ghi.

**Tương thích ngược:** `api_events` không có tham số `topics` thì giữ hành vi cũ (nhận tất cả). Client hiện có không phải sửa.

## Related Code Files

- Modify: `src-tauri/src/sse.rs` (registry, guard, `subscribe_topics`)
- Modify: `src-tauri/src/routes/misc.rs` (dùng guard, xử lý `Lagged`)
- Modify: `src/lib/api/` (client SSE — tuỳ chọn truyền topics, xử lý sự kiện `stream-lagged`)
- Kiểm tra: `src-tauri/src/auth.rs:19` — `QUERY_TOKEN_PATHS` chứa `/api/events`; thêm query param không được phá parse token (nó parse đúng cặp `token=`, nên an toàn — xác nhận bằng test)

## Implementation Steps

1. `SUBSCRIBERS` map + `SubscriptionGuard` với `Drop`.
2. `subscribe_topics`: tăng đếm, trả receiver + guard.
3. `subscriber_count(topic)`.
4. Sửa `api_events`: nhận `?topics=a,b` tuỳ chọn; giữ guard sống trong closure của stream.
5. `Lagged(n)` → phát `stream-lagged` với số mẫu mất, thay vì `None`.
6. Kênh thông báo đổi số subscriber, để Free P3 collector bám vào.
7. Test: mở 3 kết nối SSE, đóng 1 → đếm giảm đúng. Kill client thô bạo → đếm vẫn giảm.
8. Test lag: đẩy nhanh hơn client đọc, khẳng định `stream-lagged` xuất hiện.
9. Test hồi quy: client cũ không truyền `topics` vẫn nhận đủ sự kiện; auth `?token=` vẫn hoạt động khi có thêm query param.

## Success Criteria

- [ ] Đóng tab / reload / kill client → `subscriber_count` giảm đúng, không cần gọi API.
- [ ] 3 kết nối, đóng 1, đếm còn 2. Đóng hết, đếm về 0.
- [ ] Lag xảy ra → sự kiện `stream-lagged` với số mẫu mất; UI hiển thị được lỗ hổng.
- [ ] Client SSE hiện có (docker-state, instances) hoạt động không sửa gì.
- [ ] `?token=` vẫn xác thực đúng khi có thêm `?topics=`.
- [ ] Doc trong `sse.rs` ghi rõ: SSE là kênh hiển thị, không phải nguồn ghi bền vững.

## Risk Assessment

| Rủi ro | Giảm thiểu |
|---|---|
| Guard không drop khi client đứt đột ngột | Test kill thô bạo; dựa vào vòng đời stream Axum chứ không phải sự hợp tác của client |
| Đếm sai làm collector chạy mãi (hao pin) | Test đóng-hết-về-0 là criteria bắt buộc |
| Phá client SSE hiện có | `topics` là tuỳ chọn, mặc định nhận tất cả |
| Pro P2 lấy SSE làm nguồn ghi SQLite | Ghi cấm rõ trong doc + nhắc lại trong phase Pro tương ứng |
| Thêm query param phá auth `?token=` | `token_from_query` parse đúng cặp; có test hồi quy |

## Đã implement — 2026-08-12

240 test toàn crate, warning đúng 4 baseline. 17 test mới (5 ở `sse.rs`, 11 ở
`routes/misc.rs`, 1 ở `api_server.rs`).

### Tính chất cả thiết kế dựa vào, và cách nó được chứng minh

Guard chỉ có giá trị nếu nó **thực sự sống trong body HTTP và chết cùng client**.
Test đơn vị gọi thẳng `subscribe_topics` vẫn xanh kể cả khi handler đánh rơi
guard ngay lúc dựng stream — nghĩa là nó không chứng minh gì cả.

Nên có một test chạy qua router thật (`build_router()` + `oneshot`): đếm
`0 → 1` khi có response, `→ 0` khi drop response. Reviewer cũng chạy độc lập và
đo thêm: giữ nguyên bằng 1 sau khi đã đọc một chunk khỏi body (guard không rơi ở
item đầu), và 64 kết nối đồng thời trên hai topic đều về 0.

### Ba sửa từ review

| Vấn đề | Vì sao nó quan trọng | Sửa |
|---|---|---|
| `subscribe_topics` nuốt lock bị poison **nhưng vẫn trả guard mang đủ topic** | Tăng hụt, giảm đủ → đếm về 0 khi vẫn còn người xem. Đúng cái hỏng mà registry sinh ra để chặn, và `saturating_sub` khiến nó im lặng | Guard chỉ mang những topic **đã đếm được**; lock hỏng → guard rỗng |
| `Query<EventsQuery>` khiến `?topics=a&topics=b` trả **400** | Endpoint này trước đây nhận mọi hình dạng query. `EventSource` coi 400 là hỏng cứng, không retry. Thu hẹp đầu vào của một stream tự-reconnect không đáng đổi lấy code gọn hơn | Chuyển sang `RawQuery`, tự parse như `auth::token_from_query` vẫn làm. Không hình dạng nào 400 nữa |
| Key topic do client cung cấp, không giới hạn | Map là process-global, key giữ suốt đời kết nối, và mỗi sự kiện phải đối chiếu với toàn bộ | Chặn 16 topic, mỗi topic ≤ 64 ký tự, khử trùng lặp |

Khử trùng lặp có lý do riêng: `?topics=a&topics=a` là **một** người xem. Đếm
thành 2 thì producer chạy tiếp khi không còn ai nhìn.

### Lệch khỏi plan

- **Không làm bước 6** ("kênh thông báo đổi số subscriber"). `subscriber_count()`
  là hàm đọc, đủ cho collector poll ở mỗi tick — nó vốn đã có vòng tick. Thêm
  một kênh phát tán để tiết kiệm một phép đọc `HashMap` là máy móc thừa. Mở lại
  nếu Free P3 chứng minh cần.
- **Không sửa `src/lib/api/`** như plan liệt kê. Backend phát `stream-lagged`;
  client hiện có bỏ qua sự kiện lạ một cách vô hại (`EventSource` chỉ gọi
  listener có tên khớp, không rơi xuống `onmessage`). Việc vẽ lỗ hổng lên biểu đồ
  thuộc Free P3 — phase đó sở hữu UI Activity.
- **Client có `?topics=` giờ chỉ nhận đúng topic đó**, thay vì nhận tất cả rồi tự
  lọc. Không client nào truyền `topics` hôm nay nên không phá gì, và nó khiến
  `subscriber_count` có nghĩa thật.

### Chưa làm

- **Chưa có test end-to-end cho `stream-lagged` trên dây.** `classify()` được test
  trực tiếp với `Lagged(n)` tổng hợp, nhưng chưa có test nào làm tràn channel 64
  bằng một consumer chậm thật rồi đọc sự kiện ra khỏi body HTTP.
- **Chưa có consumer nào** cho `stream-lagged` trong `src/` — thuộc Free P3.
