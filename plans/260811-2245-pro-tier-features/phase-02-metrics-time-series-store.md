---
phase: 2
title: "Metrics history + Alerts"
status: in-progress — code xong 2026-08-13 (gồm sửa nâng cấp giữa phiên); CHỜ soak 24h đồng hồ thật, cần máy chạy một ngày
priority: P1
dependencies: ["prereq:P7", "free:P3"]
effort: ""
---

# Phase 2: Metrics history + Alerts

> **Gộp từ hai phase cũ** ("Metrics time-series store" + "Activity history + Alerts") sau red-team 2026-08-11. Store không ship UI và có đúng một consumer — tách ra là ranh giới giả.

## Overview

Mặt Pro của Activity Monitor: lưu mẫu xuống đĩa, vẽ biểu đồ lịch sử, và bắn cảnh báo theo luật. `docker stats` cho anh cái *bây giờ*; cái này cho anh cái *đêm qua*.

`rusqlite` đã có (`Cargo.toml:38`). Không thêm dependency cho phần lưu trữ.

## Đính chính tiền đề (red-team)

| Bản đầu nói | Thực tế |
|---|---|
| Ba bảng downsample raw/1m/1h | Over-engineering. Raw giữ 1 giờ → tối đa ~36k dòng, tự phản bác con số 864k/ngày dùng để biện minh. **Hai bảng là đủ**: raw (1h) + 1m (giữ dài hạn). |
| "DB riêng vì vòng đời khác nhau" rồi nhét cả `alert_rules`/`heal_log` vào cùng file | Mâu thuẫn nội tại. Dữ liệu prune được và cấu hình người dùng **không** cùng vòng đời. |
| "WAL cho phép đọc/ghi song song" | Không đứng vững với pattern SQLite duy nhất trong repo: `static DB: OnceLock<Mutex<Connection>>`, không pragma, `.expect()` panic (`knowledge_bank.rs:7-14,25`). Một Mutex toàn cục thì WAL không giúp gì. Cần connection riêng cho reader nếu muốn song song thật. |
| Đọc dữ liệu từ SSE | **Cấm.** SSE mất mẫu im lặng khi lag (`sse.rs:22`, `routes/misc.rs:9-12`). Ghi từ collector. |
| Không có cột instance | `detect_docker_host()` lấy profile chạy đầu tiên (`path_util.rs:172-202`). Thiếu cột này thì mẫu của hai instance trộn lẫn **không gỡ lại được**. |

## Requirements

**Lưu trữ**
- Ghi batch `MetricSample` từ collector (Free P3) qua `MetricWriter`.
- Hai bảng: `samples_raw` (giữ 1 giờ), `samples_1m` (gộp trung bình + max, giữ 90 ngày).
- Trần dung lượng cứng, mặc định 200 MB.
- Container đã xoá vẫn giữ lịch sử kèm tên (`containers_seen`).
- **Cột `instance` trên mọi bảng mẫu**, từ dòng dữ liệu đầu tiên.

**Hiển thị**
- Biểu đồ CPU/RAM/net/disk, khoảng 1h / 24h / 7d / 30d.
- Chồng nhiều container để so sánh.
- Hiển thị lỗ hổng dữ liệu (từ `stream-lagged` và từ khoảng thời gian máy ngủ), không nội suy.

**Cảnh báo**
- Luật `metric > ngưỡng` kéo dài `>N phút`.
- Cooldown chống spam.
- Thông báo hệ thống + banner trong app + lịch sử đã bắn.
- Preview backtest: "luật này sẽ bắn N lần trong 7 ngày qua".

## Architecture

```
commands/metrics_store.rs
  ├─ metrics.db          — samples_raw, samples_1m, containers_seen  (prune được)
  ├─ SqliteMetricWriter { tx: mpsc::Sender<Vec<MetricSample>> }
  │     bounded channel; đầy → DROP và ĐẾM, không bao giờ chặn collector
  ├─ writer task: gom ~5s, ghi một transaction
  ├─ retention task: mỗi 5 phút — gộp raw→1m, xoá quá tuổi, ép trần dung lượng
  └─ reader: Connection RIÊNG (không dùng chung Mutex với writer) + WAL

commands/alerts.rs
  ├─ settings.db         — alert_rules, alert_events   (cấu hình người dùng, KHÔNG prune)
  ├─ AlertEvaluator: nhận batch từ collector, giữ state vi phạm trong bộ nhớ
  └─ bắn thông báo + ghi alert_events

UI: src/pages/Activity.svelte → tab "History" + "Alerts" (ProGate chế độ paid)
```

**Hai file DB, không phải một:** `metrics.db` bị xoá/prune thường xuyên; `settings.db` chứa luật người dùng tự tay tạo và phải sống sót qua mọi lần dọn dẹp. Trộn chung là cách đánh mất cấu hình của người dùng khi retention chạy.

**Đánh giá luật ở collector, không quét DB:** cùng dữ liệu đang chảy, O(luật × container) mỗi tick, độ trễ = một chu kỳ tick.

## Related Code Files

- Create: `src-tauri/src/commands/metrics_store.rs`, `metrics_retention.rs`, `alerts.rs`
- Create: `src-tauri/src/routes/metrics.rs`
- Create: `src/components/activity/HistoryChart.svelte`, `AlertRuleEditor.svelte`, `AlertLog.svelte`
- Create: `src/lib/api/metrics.ts`, `alerts.ts`
- Modify: `src-tauri/src/commands/metrics_collector.rs` (gán `MetricWriter` — file do Free P3 tạo)
- Modify: `src/pages/Activity.svelte`, `src-tauri/src/lib.rs`
- Modify: `src-tauri/capabilities/default.json` (quyền notification — **kiểm tra trước**, có thể chưa có)
- Modify: `src/locales/{en,vi,ja,zh}.json`

## Implementation Steps

1. Kiểm tra `capabilities/default.json` có quyền notification chưa. Nếu chưa, dùng plugin notification của Tauri (nhất quán với app), không `notify-rust`.
2. Schema hai DB. `PRAGMA journal_mode=WAL` **và** connection riêng cho reader — WAL một mình không đủ khi repo dùng một Mutex toàn cục.
3. `SqliteMetricWriter` với `mpsc::channel(capacity)` bounded; `try_send`; đếm drop và phơi ra UI.
4. Writer task: gom 5 giây, một transaction. Ghi từng mẫu sẽ giết SSD.
5. `containers_seen`: upsert tên/image, để container đã xoá tra ngược được.
6. Retention: gộp raw→1m cho dữ liệu >1 giờ, xoá >90 ngày, ép trần dung lượng từ cũ nhất.
7. Query chọn bảng theo độ dài khoảng: <1h→raw, còn lại→1m.
8. `AlertEvaluator` + cooldown + luật ngưỡng.
9. `HistoryChart.svelte`: SVG thuần cho biểu đồ đường; đừng kéo Chart.js về cho việc này.
10. `AlertRuleEditor` với preview backtest trên dữ liệu 7 ngày — đây là cách duy nhất người dùng đặt ngưỡng hợp lý ngay lần đầu.
11. Gán writer **chỉ khi `paid`**, và kiểm tra lại `paid` trong writer task (hạ cấp phải ngừng ghi mà không cần restart).
12. Test 24 giờ / 20 container.

## Success Criteria

- [ ] Mọi bảng mẫu có cột `instance` từ commit đầu tiên.
- [ ] 24 giờ / 20 container: DB dưới trần, drop < 1%.
- [ ] Truy vấn 7 ngày dưới 200 ms; biểu đồ vẽ dưới 1 giây.
- [ ] Collector tick **không bao giờ** bị chặn bởi writer (test: writer chậm giả lập, tick vẫn đều).
- [ ] Container đã xoá vẫn tra được lịch sử kèm tên.
- [ ] Lỗ hổng dữ liệu hiển thị là lỗ hổng, không phải đường thẳng nội suy.
- [ ] Luật ngưỡng bắn đúng một lần cho một lần vi phạm kéo dài; cooldown chặn spam.
- [ ] Preview backtest khớp số lần bắn thật khi để chạy.
- [ ] Tài khoản Free: writer không được gán, `metrics.db` không tồn tại.
- [ ] Hạ cấp giữa chừng → ngừng ghi, **không cần restart app**.
- [ ] `settings.db` sống sót qua một lần retention xoá sạch `metrics.db`.

## Kết quả — backend, 2026-08-12

`cargo test` 366/366 (15 test mới), clippy `-D warnings` sạch, frontend
typecheck/lint/check/build/285 test xanh.

**Tạo:** `commands/metrics_store.rs`, `commands/alerts.rs`, `routes/metrics.rs`
(kèm 4 route alert), client `src/lib/api/metrics.ts`.
**Sửa:** `metrics_collector.rs` (gọi `alerts::on_batch` trên cùng batch),
`lib.rs`, `api_server.rs`, `commands/mod.rs`, `routes/mod.rs`.

### Đã làm (bước 1–8, 11)

- Hai bảng `samples_raw`/`samples_1m` + `containers_seen`, **cột `instance` từ
  dòng đầu**. Retention gộp raw→1m, xoá >90 ngày, ép trần 200 MB (xoá cũ nhất
  rồi `VACUUM` — không có VACUUM thì file không bao giờ nhỏ lại).
- Writer là thread + bounded channel: hàng đợi đầy thì **drop và đếm**, không
  bao giờ chặn tick của collector. Có test khoá lại điều này.
- Gate theo `paid` **đọc lại mỗi batch**, không phải lúc cài writer — hết hạn
  giữa chừng là ngừng ghi ngay, không cần restart. Free thì không có
  `metrics.db` nào cả.
- Alert: ngưỡng + thời lượng + cooldown, đánh giá trên chính batch của collector.
  `evaluate_batch` là hàm thuần nên **backtest dùng đúng cùng một hàm** — preview
  không thể lệch khỏi hành vi thật.

### Ba quyết định khác đặc tả

**`alert_rules` nằm trong `knowledge.db`, không phải `settings.db`.** Repo không
có `settings.db`; app settings sống trong `knowledge.db`. Yêu cầu thật của plan
không phải cái tên file mà là **retention không với tới được** — và điều đó vẫn
đúng.

**Bắn thông báo là việc của frontend.** Backend ghi `alert_events` rồi phát sự
kiện `alert.fired`. `osNotify.ts` đã nắm ba quy tắc (chỉ khi cửa sổ ở nền, chỉ
kết cục cuối, không bao giờ kèm text lỗi runtime); dựng đường thứ hai ở backend
là để hai chỗ sớm muộn bất đồng về cả ba.

**Reader mở connection riêng.** Repo dùng một `Mutex<Connection>` toàn cục cho
SQLite; nếu dùng chung, mọi truy vấn lịch sử phải xếp hàng sau transaction của
writer — đúng thứ WAL sinh ra để khỏi phải chịu.

### Một lỗi của tôi, đã sửa

Tôi **ghi đè `src/lib/api/metrics.ts`** — file chưa từng commit nên git không
khôi phục được. Đã dựng lại từ chỗ dùng nó (`Activity.svelte`, `ResourceTable`,
`metricsHistory.ts`) và đối chiếu với backend: `topics=metrics.sample`,
`stream-lagged` mang `{"dropped":n}`. Typecheck, svelte-check và 285 test xanh
trở lại, gồm cả `metricsHistory.test.ts`. Vẫn nên xem lại file đó bằng mắt.

### UI — 2026-08-13

`HistoryChart.svelte` (SVG thuần, ~230 dòng — thư viện biểu đồ tốn vài trăm KB
cho bốn polyline, và thứ duy nhất quan trọng ở đây là **không nối đường qua lỗ
hổng**, đúng cái các thư viện mặc định làm sai), `AlertRuleEditor.svelte` (có
preview backtest), `AlertLog.svelte`, tab Live/History/Alerts trong `Activity`,
`lib/alertNotifications.ts`, i18n 4 ngôn ngữ. 17 test frontend mới.

### Review tìm ra một lỗi chí mạng — do chính tôi gây ra

`metrics.sample` và `alert.fired` **chỉ** được publish qua SSE; không có
`app.emit` nào cho chúng. Khi dựng lại `metrics.ts` tôi thêm nhánh Tauri nghe
sự kiện Tauri — nên trong bản desktop nó không nghe được gì, **và tệ hơn**:
collector quyết định có lấy mẫu hay không bằng cách đếm subscriber của
`/api/events`, nên số đếm đứng ở 0 và **không có mẫu nào được lấy**. Cả tính
năng chết trên đúng bản mà người dùng chạy, trong khi browser mode vẫn chạy.
Giờ cả hai dùng một transport duy nhất là SSE.

### Sáu lỗi High khác đã sửa

1. **Mất lô cuối khi thoát app** — writer `break` khi kênh đóng mà còn hàng đợi.
2. **Roll-up cắt giữa phút** làm mất nửa đầu phút đó và mọi đỉnh trong nó: lần
   sau gộp phần còn lại rồi `INSERT OR REPLACE` đè lên. Giờ cutoff bo về mốc
   phút, và có test chạy hai lượt để khoá lại.
3. **Mọi biểu đồ 24h/7d/30d kết thúc cách đây một tiếng** — bảng 1m theo định
   nghĩa không chứa gì mới hơn `now-1h`. Giờ đọc cả phần raw gần nhất, gộp theo
   phút cho cùng ý nghĩa.
4. **State cảnh báo sống sót qua lần sửa luật**: đổi ngưỡng vẫn giữ thời gian đã
   tích theo ngưỡng cũ (bắn ngay) và cooldown cũ (im lặng). Giờ xoá state khi
   luật đổi.
5. **Backtest không khớp hành vi thật** — nó chạy trên trung bình 1 phút nên
   luật bắt đỉnh ngắn sẽ ra 0. Sửa docstring và **nói thẳng trên UI** thay vì
   giữ câu "preview không thể lệch".
6. **I/O chặn trên tick 2 giây**: mỗi tick đọc file entitlement + lấy mutex
   `knowledge.db` + chạy lại DDL. Giờ cache luật (invalidate khi đổi) và cache
   entitlement 30 giây.

Kèm: tách `skippedBatches` (hết hạn Pro) khỏi `droppedBatches` (đĩa không kịp) —
UI hiển thị cái sau là "lỗ hổng thật", nên trộn hai thứ là nói sai; VACUUM một
lần thay vì tối đa 8 lần (mỗi lần khoá độc quyền lâu hơn `busy_timeout` 5s của
writer); filter container dùng tham số ràng buộc + trần 20.000 dòng; gate
entitlement cho toàn bộ route/command alert, không chỉ lúc bắn.

### Còn lại

- **Soak mô phỏng đã có (2026-08-13)** — `cargo test --lib -- --ignored soak`, 3 test,
  ~15 giây: một ngày ảo 20 container ở đúng nhịp 2 giây, đẩy qua chính code ghi và
  retention thật, trên DB file thật (WAL checkpoint trước khi đo). Kiểm: trần dung
  lượng giữ được, cửa sổ raw 1 giờ đúng và phần còn lại đã rollup, truy vấn 7 ngày
  vẫn trả dữ liệu kèm tên container.
  **Không thay thế soak đồng hồ thật.** Thời gian mô phỏng không thấy được: writer
  rò rỉ bộ nhớ qua một ngày, nhịp lấy mẫu trôi, daemon restart giữa chừng, hay
  hàng đợi đầy vì đĩa khựng. **Tỉ lệ drop nói riêng chưa đo được** — nó là tính
  chất của hàng đợi dưới áp lực thật, mà ở đây không có gì bị chậm lại.
- Soak 24 giờ / 20 container (trần dung lượng, tỉ lệ drop <1%, truy vấn 7 ngày
  dưới 200ms). Cần máy chạy thật một ngày, không phải việc chạy trong phiên.
- ~~Nâng cấp Pro **giữa phiên** vẫn cần restart mới có writer~~ — **sửa
  2026-08-13**, xem mục dưới.

## Nâng cấp giữa phiên — đã sửa 2026-08-13

`start_if_entitled()` chỉ chạy lúc khởi động. Ai mua Pro giữa phiên **không có
writer nào cho tới khi restart**: lịch sử thủng một khoảng, không có gì trên màn
hình giải thích, và người vừa trả tiền là người thấy nó. Chiều ngược lại đã đúng
từ trước vì writer đọc lại entitlement mỗi batch.

Móc vào `subscription_store` — chỗ backend biết entitlement vừa đổi, và bao luôn
chế độ browser vì route HTTP gọi lại chính hàm đó.

Hai thứ bắt buộc đi kèm:

1. **`start_if_entitled` phải idempotent.** `install` *thay* sink của collector
   chứ không cộng dồn, nên gọi lần hai sẽ sinh thread writer thứ hai ghi cùng
   một DB, còn thread cũ chỉ biết khi kênh của nó bị drop. Có test khoá lại
   guard này (`a_second_start_is_refused_while_a_writer_is_attached`).
2. **Xoá cache entitlement khi bản ghi đổi** (`invalidate_entitlement_cache`) —
   không thì người vừa nâng cấp chờ tới 30 giây mà không hiểu vì sao.

## Soak — trạng thái thật, 2026-08-13

**Mô phỏng: 3/3 xanh** sau toàn bộ thay đổi của ngày hôm nay
(`cargo test --lib -- --ignored soak`, ~17 giây).

**Soak 24 giờ đồng hồ thật: CHƯA CHẠY.** Không phải việc làm được trong một
phiên — nó cần app chạy thật một ngày. Ghi lại đây cách chạy để lần sau không
phải nghĩ lại.

### Cách chạy

App đã phơi sẵn đúng bốn số cần đo, không phải dựng thêm gì:

```
GET /api/metrics/health   → { acceptedBatches, droppedBatches,
                              skippedBatches, dbBytes, writing }
```

1. Tài khoản Pro, ~20 container chạy liên tục, để app mở 24 giờ.
2. Ghi lại `health` lúc bắt đầu và lúc kết thúc.
3. Ba tiêu chí của plan:
   - **Tỉ lệ drop < 1%** → `droppedBatches / (accepted + dropped)`.
     `skippedBatches` **không** tính vào — nó là "Pro đã hết hạn", không phải
     "đĩa không kịp"; trộn hai thứ là báo sai nguyên nhân.
   - **DB dưới trần** → `dbBytes` < 200 MB.
   - **Truy vấn 7 ngày < 200 ms** → mở tab History, khoảng 7d, đo.

### Vì sao mô phỏng không thay thế được

Bốn thứ chỉ đồng hồ thật mới thấy, và soak mô phỏng **không** kiểm được cái nào:

- **Tỉ lệ drop.** Đây là tính chất của hàng đợi *dưới áp lực*; trong mô phỏng
  không có gì chậm lại nên hàng đợi không bao giờ đầy và con số luôn là 0.
- **Rò rỉ bộ nhớ của writer** qua một ngày chạy.
- **Nhịp lấy mẫu trôi** — tick 2 giây có giữ được sau 43.200 lần không.
- **Daemon restart / máy ngủ giữa chừng**, và lỗ hổng dữ liệu sinh ra từ đó có
  hiển thị đúng là lỗ hổng không.

## Risk Assessment

| Rủi ro | Giảm thiểu |
|---|---|
| **Mất cấu hình người dùng khi prune** | Tách `settings.db` khỏi `metrics.db`; là success criteria |
| **Thiếu cột instance, dữ liệu trộn không gỡ được** | Có cột từ commit đầu; criteria đầu tiên |
| DB phình vô hạn | Downsample + trần cứng + retention task |
| Writer chậm làm đứng UI live | Bounded channel + drop-and-count |
| Spam thông báo → người dùng tắt tính năng | Cooldown + duration threshold + preview backtest |
| Ghi liên tục hao SSD | Batch 5s/transaction |
| Người dùng huỷ Pro rồi mất dữ liệu đã thu | Không xoá DB khi hết hạn; chỉ ngừng ghi và ngừng truy vấn qua UI |
