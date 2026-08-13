---
phase: 3
title: "Activity Monitor (live)"
status: done
priority: P1
dependencies: ["prereq:P5"]
effort: ""
---

# Phase 3: Activity Monitor (live)

## Overview

Màn hình giám sát thời gian thực cho container + VM: CPU, RAM, network, disk I/O, process. Thay `docker stats` bằng UI sắp xếp/lọc được.

**Phase này quan trọng ngoài giá trị hiển thị:** nó dựng **collector** — vòng lấy mẫu duy nhất của app. Plan Pro phase 2 gắn tầng lưu trữ vào chính collector này. Nếu collector thiết kế sai (poll rời rạc ở mỗi component), Pro phase 2 sẽ phải viết lại.

## Requirements

**Functional**
- Bảng live: tên container, CPU %, mem dùng/giới hạn, net I/O, block I/O, PID.
- Sắp xếp theo mọi cột, lọc theo tên/trạng thái.
- Tài nguyên VM tổng thể (từ `/api/system/engine-resources`).
- Xem process trong một container (`container_top`).
- Chu kỳ lấy mẫu chỉnh được (1s / 2s / 5s), mặc định 2s.

**Non-functional**
- Chạy nền không được ngốn CPU: collector chỉ chạy khi có consumer, dừng khi rời trang.
- Một collector duy nhất cho toàn app, không phải mỗi component tự poll.

## Architecture

```
commands/metrics_collector.rs          ← MỚI, hạt nhân của phase
  ├─ MetricsCollector { interval, writer: Option<MetricWriter> }
  ├─ start()/stop() theo sse::subscriber_count("metrics.sample")  ← tiên quyết P5
  ├─ mỗi tick: all_container_stats() + engine_resources()
  ├─ chuẩn hoá → MetricSample { ts, instance, container_id, cpu_pct, mem_bytes, ... }
  ├─ publish_sse_event("metrics.sample", batch)     — kênh HIỂN THỊ
  └─ if let Some(w) = &writer { w.write(&batch) }   — kênh BỀN VỮNG (Pro cắm vào)
UI: src/pages/Activity.svelte
  ├─ subscribe SSE "metrics.sample"
  ├─ xử lý sự kiện "stream-lagged" → hiện lỗ hổng dữ liệu, không vẽ đường thẳng qua
  └─ cửa sổ trượt trong bộ nhớ có giới hạn cứng (120 mẫu) để vẽ sparkline
```

## Đính chính tiền đề (red-team 2026-08-11)

| Bản đầu nói | Thực tế |
|---|---|
| "Đếm subscriber, dừng khi 0" | Không có cơ chế nào để đếm. `sse.rs:17-27` là một broadcast channel process-global; `routes/misc.rs:7` subscribe không ghi sổ; `receiver_count()` gộp cả client docker/k8s/KB. → **Tiên quyết P5** xây registry theo topic. |
| Trait `MetricSink` với nhiều impl | Trừu tượng non cho đúng hai người dùng. Thay bằng `Option<MetricWriter>` cụ thể: Free để `None`, Pro gán `Some`. Nhánh vẫn tồn tại, chỉ là một dòng ở nơi khởi tạo — đừng giả vờ trait làm nó biến mất. |
| Pro sẽ đọc dữ liệu qua SSE | **Cấm.** SSE mất mẫu im lặng khi lag (`sse.rs:22` dung lượng 64, `routes/misc.rs:9-12` nuốt `Lagged`). Pro ghi từ **collector**, không phải từ SSE. |

**Cột `instance` trong `MetricSample`:** có mặt ngay từ đầu dù hôm nay chỉ một giá trị. `detect_docker_host()` lấy profile chạy đầu tiên (`path_util.rs:172-202`) — nếu sau này hỗ trợ nhiều instance mà cột này chưa có, dữ liệu đã ghi sẽ trộn lẫn không gỡ lại được.

## Related Code Files

- Create: `src-tauri/src/commands/metrics_collector.rs`
- Create: `src/pages/Activity.svelte`
- Create: `src/lib/api/metrics.ts`
- Create: `src/components/activity/ResourceTable.svelte`, `src/components/activity/Sparkline.svelte`
- Modify: `src-tauri/src/commands/containers.rs` (tái dùng `all_container_stats`, `container_top`)
- Modify: `src-tauri/src/lib.rs` (khởi tạo collector state)
- Modify: `src/components/Sidebar.svelte` (mục Activity)
- Modify: `src/App.svelte` (route)
- Modify: `src/locales/{en,vi,ja,zh}.json`

## Implementation Steps

1. Định nghĩa `MetricSample` (phẳng, serializable, **có cột `instance`**) và `MetricWriter` trong `metrics_collector.rs`. Struct này sẽ thành schema bảng ở Pro — chốt nó trước khi code UI.
2. `MetricsCollector` với `Arc<RwLock<...>>` state, tokio task chạy vòng tick.
3. Bật/tắt theo `sse::subscriber_count("metrics.sample")` từ tiên quyết P5. **Không** tự chế endpoint subscribe/unsubscribe — nó rò rỉ mỗi lần client reload hoặc crash.
4. Gom một batch mỗi tick, phát **một** SSE event, không phải một event/container.
5. UI xử lý `stream-lagged`: vẽ khoảng trống, không nội suy.
6. `Activity.svelte`: bảng + sparkline, cửa sổ trượt trong bộ nhớ có giới hạn cứng.
7. Panel process: gọi `container_top` on-demand, không đưa vào vòng tick.
8. Chọn chu kỳ lấy mẫu, lưu vào settings.
9. Đo tải: chạy 30 phút với 20 container, ghi lại CPU trung bình của app.

## Success Criteria

- [x] Đóng trang Activity → collector dừng hẳn (verify bằng log hoặc `Instruments`), CPU app về nền.
- [x] 20 container, tick 2s: CPU app trung bình dưới 2% trên máy Apple Silicon. — **đo được 0.14%** (xem phần bổ sung cuối file)
- [x] Số liệu khớp `docker stats` trong sai số hợp lý.
- [x] Cửa sổ trượt trong UI không tăng bộ nhớ vô hạn sau 1 giờ mở trang.
- [x] `MetricSample` có cột `instance` ngay từ commit đầu tiên.
- [x] UI hiển thị lỗ hổng khi nhận `stream-lagged`, không nội suy qua chỗ mất.
- [x] Collector không import gì từ `pro`/`subscription`; nhánh Pro nằm ở nơi khởi tạo.

## Risk Assessment

| Rủi ro | Giảm thiểu |
|---|---|
| **Collector thiết kế sai → Pro phải viết lại** | Chốt `MetricSample` (phẳng, có cột `instance`) và chữ ký `MetricWriter` ngay ở phase này; review schema trước khi code UI |
| App ngốn CPU khi chạy nền | Đếm subscriber, dừng khi 0; đây là success criteria bắt buộc |
| `docker stats` chậm khi nhiều container | Một lệnh `stats --no-stream` cho tất cả, không lặp từng container |
| Rò rỉ bộ nhớ ở UI | Giới hạn cứng số mẫu giữ lại |

## Kết quả thực thi — 2026-08-12

Tiên quyết P5 (SSE subscriber registry) đã Completed nên phase này chạy được.

### Bằng chứng

- `cargo test --lib`: **264/264 pass** (12 test mới ở `metrics_collector`).
- `vitest`: 150/150 (9 test mới ở `metricsHistory`).
- **Collector dừng hẳn khi không ai xem** — đo bằng bộ đếm tick thật, không suy đoán (`src-tauri/tests/metrics_collector_lifecycle.rs`, gate `#[ignore]`): 2.5s không subscriber → tick **không tăng**; subscribe → tick tăng; drop guard → tick **ngừng tăng**. Đây là tiêu chí đầu bảng của phase.
- **Số liệu khớp `docker stats`**: test đối chiếu trên engine thật — mỗi dòng docker in ra thành đúng một sample, và `memBytes/memLimitBytes` suy ra phần trăm lệch **dưới 1%** so với `MemPerc` docker tự báo. Đây là cái bắt lỗi đơn vị.
- **Không phình bộ nhớ**: test mô phỏng 1800 tick (1 giờ ở chu kỳ 2s) với 20 container ổn định + 1 container mới mỗi tick → giữ đúng 21 series, tổng điểm ≤ 21 × 120.
- `svelte-check`: 0 lỗi và 0 warning từ file của phase này. `pnpm build` pass.

### Quyết định thiết kế đáng ghi

1. **Metrics dùng SSE ở *cả* desktop lẫn browser**, khác phần còn lại của app. Lý do bắt buộc: `subscriber_count` chỉ đếm client SSE có nêu topic. Ở Tauri mode frontend dùng `listen()` — không đăng ký topic nào — nên count sẽ luôn 0 và **collector không bao giờ chạy**. Ghép Tauri event với một cặp endpoint register/unregister thì rò rỉ một watcher mỗi lần reload, đúng thứ plan cấm. Vòng đời của HTTP stream chính là vòng đời của subscription.
2. **`MetricWriter = Arc<dyn Fn(&[MetricSample])>`**, không phải trait riêng — đúng tinh thần plan. Free để `None`, Pro gọi `set_metric_writer` một lần lúc khởi tạo.
3. **Có test khẳng định collector không tham chiếu `pro`/`subscription`**, quét chính source của module. Needle dựng lúc chạy, nếu viết literal thì test tự khớp source của nó.
4. **Engine resources cache 15s**, không gọi mỗi tick: `engine_resources()` cũng chạy `docker stats` riêng, đưa vào vòng tick là nhân đôi tải daemon cho số liệu đổi rất chậm.
5. **Trừ thời gian lấy mẫu khỏi chu kỳ ngủ**, nếu không chu kỳ thành "interval cộng thời gian docker thấy thích" và trục thời gian của UI hết đúng nghĩa.
6. **Lịch sử tách thành `lib/metricsHistory.ts`** thay vì nằm trong component: đó là chỗ rò bộ nhớ, và tách ra thì tiêu chí "1 giờ không phình" thành unit test.

### Xử lý mất mẫu

`stream-lagged` → `history.markGap()` đẩy `null` vào mọi series; Sparkline vẽ nhiều polyline rời, đứt tại lỗ hổng; dòng bảng mờ đi; thanh trạng thái đếm tổng số mẫu mất. Không nội suy qua chỗ mất ở bất kỳ đâu.

### Chưa verify

- Chưa click thật trên GUI: chuyển chu kỳ, sắp xếp cột, panel process mới verify ở mức type + build.

**Bổ sung 2026-08-12 — verify tương tác ở tầng component.** Không click được cửa sổ thật trong môi trường này (lý do ở báo cáo `plans/reports/from-sequential-thinking-to-owner-260812-1233-*`), nhưng phần *logic* của việc click thì mount và bấm được — cùng kỹ thuật đã bắt lỗi `effect_update_depth_exceeded` ở `GraphCanvas`. Còn lại chưa phủ: bố cục thị giác, hành vi ở tầng OS (file picker native, mở browser), và cảm giác khi dùng.

9 test mới ở `src/components/activity/Sparkline.test.ts` đóng tiêu chí "hiển thị lỗ hổng khi mất mẫu" ở **tầng vẽ**, chứ trước đó chỉ có ở tầng dữ liệu (`MetricsHistory` lưu `null`): đường bị cắt tại mọi gap chứ không nối qua, mẫu đơn lẻ giữa hai gap được vẽ thành dấu chấm (bỏ im lặng sẽ đọc thành "không có dữ liệu" trong khi có), chuỗi toàn gap không sập, không NaN/Infinity với chuỗi toàn 0, và không ghim trần 100% — CPU vượt 100% trên nhiều core nên ghim trần sẽ cắt phẳng đúng phần đáng xem.

## Bổ sung — 2026-08-12: đo và tối ưu chi phí lấy mẫu

Tiêu chí CPU trước đó để ngỏ. Đã dựng phép đo (`src-tauri/tests/metrics_collector_cost.rs`, gate `#[ignore]`, chạy `--release`): 20 container throwaway, 25 container đang chạy trên máy, chu kỳ 2s, 60 giây, đo `getrusage(RUSAGE_SELF + RUSAGE_CHILDREN)` — tính cả tiến trình con, vì `docker stats` chạy như con của app và bỏ nó ra sẽ cho một con số đẹp mà sai.

Kết quả lần đầu **bác bỏ tiêu chí, và bác bỏ cả tiền đề**:

| Lần đo | CPU | Chi phí/mẫu | Tick trong 60s |
|---|---|---|---|
| Ban đầu (`docker stats` CLI) | **11.16%** | 669 ms | **10**/30 |
| Sau khi chuyển sang engine API | 6.06% | 303 ms | 12/30 |
| Sau khi bỏ engine snapshot khỏi vòng tick | 7.55% | 151 ms | **30**/30 |
| Sau khi bỏ hẳn `engine_resources` khỏi collector | **0.14%** | **2.8 ms** | 30/30 |

Chỉ 10 tick nghĩa là chu kỳ 2s **bất khả thi** — một lần `docker stats --no-stream` mất ~6 giây với 25 container. Đó không phải chuyện tinh chỉnh hằng số.

Ba nguyên nhân, tìm bằng đo chứ không bằng đoán:

1. **CLI thay vì API.** `docker stats --no-stream` spawn một tiến trình mỗi tick *và* tự lấy hai mẫu nội bộ cho mỗi container để tính CPU%. Đo trực tiếp: `one_shot: true` qua bollard = **9 ms**/container, `one_shot: false` = **1.009 s**/container. 25 container đồng thời qua API = **~20 ms**.
   Chuyển sang bollard (đã là dependency), tự tính delta CPU từ tick trước — vốn đã lấy mẫu định kỳ nên tick trước *chính là* mẫu "pre". Dữ liệu còn đúng hơn: delta trải trên đúng chu kỳ thật, không phải cửa sổ 1 giây nội bộ của CLI. Giữ đường CLI làm fallback cho engine không nói Docker API (`nerdctl`/containerd).
2. **`engine_snapshot()` được `await` ngay trong vòng tick.** Nó gọi `engine_resources()`, bên trong lại chạy `docker stats` CLI. Cứ ~15s một tick phải trả 5-6 giây → chu kỳ trôi. Chuyển sang refresh nền, không chặn.
3. **`engine_resources` vốn không nên có ở đây.** Đo từng lệnh: `docker stats` = **2.04s**, `docker info` = 0.01s, `docker system df` = 0.01s. Phần đắt duy nhất là tổng CPU — thứ collector **đã có** (tổng `cpu_pct` từng container). Bỏ hẳn khỏi collector; Activity page fetch `/api/system/engine-resources` **một lần khi mở** cho core count/memory/version/disk — những con số đổi theo phút, không thuộc một mẫu 2 giây.

Đúng đắn sau khi đổi nguồn dữ liệu: test mới `api_figures_agree_with_the_docker_cli` so từng container giữa đường API và `docker stats` CLI — tên, số PID khớp tuyệt đối; memory và limit trong sai số; counter đơn điệu. Chạy trên 5 container thật.
