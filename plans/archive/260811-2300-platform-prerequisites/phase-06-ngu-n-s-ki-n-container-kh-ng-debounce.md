---
phase: 6
title: Nguồn sự kiện container không debounce
status: completed
priority: P2
dependencies: []
effort: ''
---

# Phase 6: Nguồn sự kiện container không debounce

## Overview

`sse_docker_watcher` (`sse.rs:57-115`) tiêu thụ stream sự kiện Docker rồi **vứt bỏ nội dung sự kiện**: nó chỉ dùng chúng làm tín hiệu "có gì đó đổi", áp debounce trailing-edge 500 ms, rồi fetch lại toàn bộ danh sách container + image.

Đúng cho mục đích ban đầu (làm mới UI). Nhưng nó phá hỏng mọi thứ cần **đếm sự kiện**:

- Crash loop = container restart 5 lần trong 2 phút. Với debounce 500 ms, cả chuỗi restart nhanh gộp thành **một** lần fetch. Luật crash-loop ở Pro P3/P4 không bao giờ bắn.
- OOM-killed là một sự kiện `die` với exit code cụ thể — thông tin nằm trong event, và nó đang bị vứt.

## Requirements

**Functional**
- Một stream sự kiện container **không debounce, không mất mát**, giữ nguyên nội dung sự kiện: `action` (start/die/kill/oom/health_status), `container_id`, `exit_code`, `timestamp`.
- Người tiêu thụ đăng ký được nhiều bên cùng lúc.
- `sse_docker_watcher` tiếp tục hoạt động y như cũ (debounce + fetch) — nó là một consumer của stream mới, không phải bị thay thế.

**Non-functional**
- Không mở kết nối Docker event thứ hai. Một stream, nhiều consumer.
- Consumer chậm không được làm chặn consumer khác.

## Architecture

```
commands/docker_events.rs                        ← MỚI
  ├─ struct ContainerEvent { ts, action, container_id, container_name,
  │                          exit_code: Option<i64>, attributes }
  ├─ EVENTS_TX: broadcast::Sender<ContainerEvent>   (dung lượng lớn hơn SSE, ~512)
  ├─ start_event_watcher()  — một kết nối bollard duy nhất, đọc stream, publish TỪNG event
  └─ subscribe() -> Receiver<ContainerEvent>

Consumer:
  ├─ sse_docker_watcher   ← đổi thành subscribe() rồi tự debounce phía nó
  ├─ Pro P3 alert evaluator (crash-loop)
  └─ Pro P4 heal triggers (OomKilled, Unhealthy)
```

**Nguyên tắc: debounce là việc của consumer, không phải của nguồn.** Nguồn phát nguyên vẹn; ai cần làm mượt thì tự làm. Đây chính là lỗi thiết kế hiện tại — debounce nằm ở nguồn nên không consumer nào lấy lại được dữ liệu đã mất.

**Về `docker_state.rs:118-160`:** watcher phía Tauri có cùng vấn đề debounce. Phase này nên gộp cả hai về một nguồn duy nhất; nếu chi phí quá lớn thì ít nhất **không** thêm bản sao thứ ba, và ghi rõ bản nào là nguồn sự thật.

**Reconnect:** stream sự kiện Docker đứt khi VM restart. Watcher hiện tại `break` khỏi vòng lặp và **chết vĩnh viễn** (`sse.rs:107-111`) — sau khi colima restart, SSE im lặng cho tới khi khởi động lại app. Phase này phải thêm reconnect có backoff; nếu không, self-healing "restart VM" sẽ tự cắt đứt nguồn sự kiện của chính nó.

## Related Code Files

- Create: `src-tauri/src/commands/docker_events.rs`
- Modify: `src-tauri/src/sse.rs` (`sse_docker_watcher` thành consumer)
- Modify: `src-tauri/src/docker_state.rs:118-160` (gộp về một nguồn, hoặc ghi rõ quan hệ)
- Modify: `src-tauri/src/lib.rs` (spawn watcher)

## Implementation Steps

1. Đọc kỹ `docker_state.rs:118-160` và `sse.rs:57-115` — xác định hai watcher có trùng nhau không, và mode nào dùng cái nào.
2. `ContainerEvent` + parse từ `bollard::system::EventMessage`. Giữ `exit_code` từ `actor.attributes`.
3. `start_event_watcher` với reconnect backoff (1s → 2s → 4s → tối đa 30s), log mỗi lần reconnect.
4. Chuyển `sse_docker_watcher` sang `subscribe()`, giữ nguyên debounce 500 ms **ở phía nó**.
5. Gộp hoặc phân định rõ với watcher trong `docker_state.rs`.
6. Test: script khởi động container crash-loop (`docker run --restart=always` với entrypoint exit ngay), khẳng định nhận đủ N sự kiện `die` riêng biệt, không bị gộp.
7. Test reconnect: `colima stop && colima start`, khẳng định watcher tự nối lại và sự kiện chảy tiếp.

## Success Criteria

- [ ] Container restart 5 lần trong 10 giây → nhận đúng **5** sự kiện `die` riêng biệt, không gộp.
- [ ] `exit_code` có mặt trong sự kiện `die`.
- [ ] Sự kiện OOM phân biệt được với exit thường.
- [ ] SSE `docker-state-updated` hoạt động **y như trước** (debounce vẫn 500 ms phía consumer).
- [ ] `colima stop && colima start` → watcher tự nối lại trong 30 giây, không cần khởi động lại app.
- [ ] Chỉ **một** kết nối Docker event trong toàn app (verify bằng log hoặc `docker system events` phía ngoài).

## Risk Assessment

| Rủi ro | Giảm thiểu |
|---|---|
| **Watcher chết vĩnh viễn sau khi VM restart** — lỗi đang tồn tại, và self-healing sẽ kích hoạt chính nó | Reconnect backoff là success criteria, không phải tuỳ chọn |
| Consumer chậm làm nghẽn nguồn | `broadcast` dung lượng 512; consumer chậm nhận `Lagged`, không chặn ai |
| Thêm bản sao watcher thứ ba | Bước 1 bắt buộc: kiểm kê trước khi thêm |
| Đổi hành vi SSE hiện có | Debounce giữ nguyên, chỉ dời vị trí; criteria "y như trước" |
| Lụt sự kiện khi có 100 container khởi động cùng lúc | Consumer tự lọc theo container quan tâm; nguồn không lọc |

## Đã implement — 2026-08-12

250 test unit + 1 test tích hợp chạy với Docker thật. Warning đúng 4 baseline.

### Kiểm kê ở bước 1 đổi hẳn thiết kế

Plan giả định phải viết `start_event_watcher` mới. Đọc code thì
`docker_state::start_docker_watcher` **không phải** watcher sự kiện đơn thuần —
nó sở hữu vòng đời kết nối Docker cho Tauri mode: cờ suppression, ping liveness,
xoá cache, `docker-connection-lost`, và **đã có reconnect tử tế**.

Cái thiếu reconnect là `sse_docker_watcher`: bản sao đơn giản hơn, `break` vĩnh
viễn khi stream lỗi.

Nên thay vì thêm watcher thứ ba:

- `start_docker_watcher` publish sự kiện thô vào `docker_events` **trước** khi
  debounce (cả sự kiện đầu lẫn những cái vòng drain nuốt).
- `sse_docker_watcher` bỏ hẳn stream riêng, thành consumer, giữ debounce 500 ms
  ở phía nó.

Kết quả: **2 event stream → 1**, và SSE thừa hưởng reconnect có sẵn thay vì phải
viết backoff mới. Bước 3 của plan (viết reconnect backoff riêng) không cần nữa.

### Bằng chứng

Test tích hợp `tests/docker_events_against_docker.rs` chạy với Docker 29.5.2:
container `--restart on-failure:5` phát **6** sự kiện `die` riêng biệt, mỗi cái
giữ `exit_code`. Chạy 3 lần, xanh cả 3.

Đây là tiền đề unit test không chứng minh được: rằng Docker thực sự phát từng
`die` một chứ không gộp. Nếu daemon gộp thì không luật crash-loop nào chạy được,
và toàn bộ module này vô nghĩa.

`grep '.events('` trong `src/`: đúng một chỗ (`docker_state.rs`).

### Hai hồi quy im lặng tôi tự tạo ra, do review bắt

| Hồi quy | Hậu quả | Sửa |
|---|---|---|
| `fetch_current_docker_state` chỉ biết tìm socket colima | Bản cũ có fallback `connect_with_defaults()`. Trên máy chạy Docker Desktop, Tauri mode hoạt động còn **browser mode im lặng không hiện gì** — vĩnh viễn, không log lỗi | Dùng chung `docker_state::connect_bollard` (`pub(crate)`). Hai helper kết nối bất đồng ý là bug chờ được viết |
| Đường reconnect không đẩy state cho SSE | Browser client chỉ được làm mới khi có sự kiện container. Sau outage mà không container nào chạy thì **không có sự kiện nào** → ngồi trên dữ liệu trước outage vô thời hạn | `publish_sse_event` ở cả đường reconnect lẫn connection-lost, song song với `app.emit` |

### Ba sửa khác từ review

- **`exec_*` bị loại khỏi stream.** Chúng là container-type nhưng không phải vòng
  đời: đo thực tế, healthcheck exec chiếm **18/33** sự kiện container trong 30
  giây. Nặng hơn: `action` mang nguyên command line
  (`exec_create: /bin/sh -c wget -qO- http://…`), mà `ContainerEvent` derive
  `Serialize` — một healthcheck có xác thực sẽ đưa credential vào bất cứ thứ gì
  chuyển tiếp sự kiện. `health_status:` vẫn giữ, vì đó là vòng đời.
- **Vòng drain quay nóng 100% CPU khi channel đóng.** `is_ok()` coi
  `Ok(Err(Closed))` là "có sự kiện" và `recv()` trên channel đã đóng trả về tức
  thì. Không tới được hôm nay vì producer là `OnceLock` sống hết đời tiến trình —
  đúng lý do nó bị bỏ sót. Đổi sang match tường minh.
- **Hằng số test sai.** `on-failure:5` ra **6** `die` (lần hỏng đầu + 5 lần
  retry), không phải 5. Test cũ vẫn xanh vì nó dừng đếm ở 5. Đổi thành
  `RETRIES + 1` với ghi chú đo được, và cleanup chuyển sang `Drop` guard —
  cleanup đặt sau assert không bao giờ chạy khi assert fail, đúng lúc còn container
  đang restart cần dọn.

### Chưa verify được

- **`colima stop && colima start`** (criteria reconnect) — không chạy vì máy đang
  có container thật của người dùng. Reconnect là đường code có sẵn của
  `start_docker_watcher`, không phải phần mới; nhưng hai bản vá H1/H2 ở trên nằm
  trên đường đó và **chỉ được kiểm bằng biên dịch**, chưa bằng một lần outage thật.
- **Fallback Docker Desktop** — cần máy không có colima để kiểm thật.
