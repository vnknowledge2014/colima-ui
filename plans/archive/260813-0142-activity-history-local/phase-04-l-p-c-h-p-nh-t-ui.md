---
phase: 4
title: "Lớp đọc hợp nhất + UI"
status: completed
priority: P2
dependencies: [2]
effort: "1-1.5 ngày"
---

# Phase 4: Lớp đọc hợp nhất + UI

## Overview

Một dòng thời gian đọc từ năm nguồn, và trang để xem nó. Chờ phase 2 chứ không
chờ phase 3 — hiển thị được ngay khi mới cắm nhóm `destructive`.

## Requirements

**Functional**
- Dòng thời gian trộn `activity_log` + `heal_log` + `alert_events` + `scan_runs`
  + lịch sử compose fix, sắp theo thời gian.
- Lọc theo nhóm, theo đối tượng, theo khoảng thời gian.
- Free: xem được phần trong hạn giữ. Pro: toàn bộ + lọc nâng cao + xuất.

**Non-functional**
- Một nguồn hỏng không làm sập cả dòng thời gian.
- Không phân trang bằng cách nạp hết rồi cắt ở client.

## Architecture

```
commands/activity_feed.rs
  └─ feed(filter) -> Vec<FeedItem>
       đọc 5 nguồn, chuẩn hoá về một hình, trộn theo ts, cắt theo limit
       nguồn nào lỗi → bỏ qua nguồn đó, ghi vào `partial`, KHÔNG lỗi cả lệnh

src/pages/Activity.svelte  ← ĐÃ TỒN TẠI (metrics live + history)
  └─ thêm tab "History"      — KHÔNG dựng trang mới
       ├─ ActivityFeed.svelte  — danh sách + lọc
       └─ ProGate paid         — bọc lọc nâng cao + xuất, KHÔNG bọc danh sách
```

**Đã kiểm 2026-08-13:** `src/pages/Activity.svelte` và mục sidebar `activity` đã
tồn tại (metrics). Đây là **tab thứ hai trong trang đó**, không phải trang mới —
hai mục sidebar cùng tên "Activity" là lỗi điều hướng, không phải tính năng.

### Khử trùng khi hiển thị

Self-heal restart sinh **hai** bản ghi: `heal_log` (luật đã chạy) và
`activity_log` (container đã restart, `actor = app`). Đúng như thiết kế phase 3 —
nhưng dòng thời gian không được hiện hai dòng cho một sự việc.

Quy tắc: cùng `(ts ± 2s, target, verb)` và một trong hai có `actor = app` → gộp
thành một mục, giữ ngữ cảnh phong phú hơn (`heal_log` biết luật nào).

Cửa sổ ±2s là phỏng đoán, không phải sự thật. Đo trên dữ liệu thật rồi chỉnh;
ghi lại con số đã đo vào phase log.

### `partial` phải hiện ra

Nếu `security.db` khoá và `scan_runs` đọc không được, dòng thời gian vẫn dựng từ
bốn nguồn kia — và **nói rõ** là đang thiếu. Một dòng thời gian im lặng bỏ sót
một nguồn là dòng thời gian nói dối; người dùng kết luận "không có gì xảy ra".

### Ranh giới Free/Pro

| Free | Pro |
|---|---|
| Dòng thời gian trong hạn giữ (7 ngày) | Toàn bộ hạn giữ (365 ngày) |
| Lọc theo nhóm | Lọc theo đối tượng, khoảng thời gian, outcome |
| — | Xuất JSON/CSV |

**Danh sách không bọc `ProGate`.** Nó hiển thị dữ liệu đã có sẵn trên máy người
dùng; khoá lại là khoá người ta khỏi nhật ký của chính họ. Gate ở tầm nhìn xa và
tiện ích, không ở quyền xem.

## Related Code Files

- Create: `src-tauri/src/commands/activity_feed.rs`
- Create: `src/components/activity/ActivityFeed.svelte`
- Create: `src/components/activity/activity-feed.ts` (+ `.test.ts` — trộn và khử
  trùng là logic thuần, phải test được không cần DB)
- Modify: `src/lib/api/` (client), `src/pages/` (gắn trang/tab)
- Modify: `src/locales/{en,vi,ja,zh}.json`
- Modify: `src/components/ProGate.svelte` + `telemetry/events.rs` (`activity.export`)

## Implementation Steps

1. `FeedItem` — hình chuẩn hoá đủ cho cả năm nguồn, không phải hợp của mọi trường.
2. Bộ đọc từng nguồn, mỗi nguồn tự cô lập lỗi.
3. Trộn + khử trùng trong **TypeScript thuần** (`activity-feed.ts`) hay Rust?
   → **Rust**, cùng chỗ với truy vấn, để phân trang đúng. Logic khử trùng có
   test đơn vị riêng.
4. UI danh sách + lọc cơ bản (Free).
5. `ProGate paid` cho lọc nâng cao + xuất.
6. i18n 4 ngôn ngữ.

## Tests

- Trộn 5 nguồn → thứ tự thời gian đúng, `limit` tôn trọng.
- Một nguồn ném lỗi → còn lại vẫn ra, `partial` liệt kê đúng nguồn hỏng.
- Self-heal restart → **một** mục, không phải hai.
- Restart do người dùng, và self-heal restart cùng container cách 10 phút →
  **hai** mục (không gộp nhầm qua cửa sổ thời gian).
- Free: không thấy mục ngoài hạn giữ.
- Pro hết hạn → danh sách vẫn render, lọc nâng cao và xuất biến mất.

## Success Criteria

- [ ] Dòng thời gian trộn đủ 5 nguồn, sắp đúng.
- [ ] Một nguồn hỏng → hiện `partial`, không sập.
- [ ] Không có mục trùng cho self-heal.
- [ ] Danh sách render cho Free; chỉ lọc nâng cao + xuất bị gate.
- [ ] `pnpm lint/typecheck/check` + `cargo clippy -D warnings` sạch.

## Risk Assessment

| Rủi ro | Giảm thiểu |
|---|---|
| Cửa sổ khử trùng ±2s gộp nhầm hai việc thật | Test ca 10 phút; đo trên dữ liệu thật rồi chỉnh, ghi số đã đo |
| Nạp hết 5 nguồn rồi cắt ở client → chậm khi dữ liệu lớn | `limit` + khoảng `ts` đẩy xuống từng truy vấn |
| Bọc `ProGate` quanh cả danh sách | Success criteria tường minh; là quyết định sản phẩm, không phải chi tiết |
| ~~Trang mới trùng với Activity page~~ | **Đã giải quyết:** trang đã tồn tại, đây là tab thứ hai trong nó |


## Đã dựng — 2026-08-13

### Tệp

| Tệp | Vai trò |
|---|---|
| `commands/activity_feed.rs` | Chuẩn hoá 5 nguồn → `FeedItem`, trộn, `dedupe()`, CSV, lệnh xuất |
| `routes/activity.rs` | `+api_activity_feed`, `+api_activity_export` |
| `commands/security_history.rs` | `+recent(limit)` — đọc mọi image; tách `row_to_run` dùng chung |
| `commands/compose_autofix_apply.rs` | Tách `history_records()` đồng bộ khỏi lệnh async |
| `lib/api/activity.ts` | Client (chưa từng có, dù backend phase 2 đã xong) |
| `components/activity/ActionFeed.svelte` | UI + lọc theo nhóm + xuất |
| `pages/Activity.svelte` | Tab thứ tư `actions` |

### Hai hàm đọc phải thêm

`security_history` chỉ có `history(digest)` — trả lời "image **này** đi lên hay
xuống", thứ một biểu đồ cần. Dòng thời gian hỏi câu khác ("cái gì đã xảy ra,
theo thứ tự") và không có digest nào trong tay. Tương tự, lịch sử compose fix chỉ
tồn tại dưới dạng lệnh Tauri async; tách phần đồng bộ ra để hai nơi đọc **cùng
một** bộ bản ghi, không thể lệch nhau.

### Khử trùng: cửa sổ 2 giây, và vì sao hẹp

Self-heal restart sinh hai bản ghi có chủ đích (`heal_log` biết luật nào chạy,
`activity_log` biết container đã restart, `actor = app`). Cả hai đều đúng và đều
cần, nhưng là **một** sự việc.

Điều kiện gộp: cùng target, cùng verb, cách nhau ≤ 2s, **khác nguồn**, và ít nhất
một bên là `actor = app`. Ba điều kiện sau là thứ ngăn gộp nhầm — hai cú bấm của
người dùng cách nhau 1 giây vẫn là hai dòng.

2 giây chỉ cần hấp thụ độ lệch đồng hồ giữa hai lần ghi trong **cùng một lời
gọi**, không phải để đoán. Vẫn là con số phỏng đoán; đo lại trên dữ liệu thật
trước khi ai đó nới rộng.

7 test cho riêng `dedupe`, gồm các ca **không được** gộp: cách nhau 10 phút,
cùng nguồn, khác container, khác verb, hai hành động của người dùng.

### `partial` — nguồn hỏng phải hiện ra

Cả 5 nguồn đọc trong `match` riêng; nguồn nào lỗi thì tên nó vào `Feed.partial`
và bốn nguồn kia vẫn dựng được dòng thời gian. UI in dòng cảnh báo màu vàng.
Một dòng thời gian im lặng bỏ sót một nguồn đọc như "không có gì xảy ra" — đúng
điều duy nhất nó không được phép nói khi có chuyện đã xảy ra.

### Ranh giới Free/Pro

**Danh sách không bọc `ProGate`.** Nó hiển thị bản ghi cục bộ về chính máy người
dùng; tính phí để đọc lại là tính phí người ta xem lịch sử của chính họ. Pro mua
**tầm với**: hạn giữ dài hơn (đã cưỡng chế ở phase 2) và **xuất** (JSON/CSV).

Lệnh xuất kiểm entitlement ở backend, confine đường ghi bằng
`assert_path_within` theo đúng khuôn của diagnostics — picker ghi nhận ý định,
không phải ranh giới phân quyền.

### CSV có test cho ca thật sự làm vỡ export

Detail của một lỗi runtime chứa dấu phẩy, dấu nháy kép và xuống dòng. Test dựng
đúng chuỗi đó và khẳng định nháy kép được nhân đôi, bản ghi vẫn là **một** record
dù trải trên nhiều dòng vật lý.

### Cổng chất lượng

`cargo clippy -D warnings` sạch · `cargo test --lib` **465 passed** ·
`pnpm lint/typecheck/check` 0 lỗi.

## Code review — 2026-08-13, và ba lỗi thật nó bắt được

Reviewer trả `DONE_WITH_CONCERNS`. Ba lỗi đầu là lỗi **của tôi**, đều đã sửa.

### 1. CRITICAL — `dedupe` không bao giờ chạy trong thực tế

`from_heal` dựng verb bằng `format!("{:?}", action).to_lowercase()` →
`"restartcontainer"`. Nhưng `self_heal` ghi vào `activity_log` bằng
`action.as_str()` → `"restart_container"`. Điều kiện `kept.verb == item.verb`
**luôn sai** với đúng cặp mà nó tồn tại để gộp. Mọi lần self-heal restart sẽ
hiện hai dòng.

**Test của tôi không bắt được vì nó tự dựng cả hai phía với `verb: "restart"`** —
phủ sóng giả cho đúng cái mà chính module ghi là "phần duy nhất có thể sai".

Sửa: dùng `action.as_str()` (đổi thành `pub`, kèm chú thích nói rõ vì sao), và
**viết lại test đi qua `from_heal`/`from_activity` thật** thay vì dựng tay. Thêm
một test khẳng định trực tiếp hai kho đánh vần verb giống nhau.

### 2. HIGH — heal bị chặn lại được báo là đã làm

`HealOutcome` có `QuotaBlocked`, `NotEntitled`, `SwitchedOff` — nghĩa là app
**không hề hành động**. So khớp chuỗi của tôi đẩy cả ba vào `Ok`, tức dòng thời
gian khẳng định một cú restart chưa từng xảy ra.

Sửa bằng `match` vét cạn trên enum: thêm variant mới sẽ **không compile** thay vì
lặng lẽ rơi vào `Ok`.

### 3. HIGH — bộ lọc chỉ chạm 1 trong 5 nguồn

Chỉ `activity::query` nhận filter. Chọn "Lifecycle" vẫn trả về toàn bộ alert,
scan và compose fix. Sửa: lọc lại `kind`/`target` trên tập đã trộn.

### Sửa kèm

- **Điều kiện gộp siết thành *cả hai* phía là `App`.** Self-heal ghi cả hai dòng
  với `actor = App` nên không mất gì; còn yêu cầu chỉ một phía thì một cú restart
  do người dùng bấm trong vòng 2 giây sau một lần tự động sẽ bị nuốt — **xoá một
  sự việc có thật**, tệ hơn hiển thị trùng.
- Thông điệp gate của lệnh xuất từng nói "Security policy needs a subscription"
  (mượn guard của module khác). Đã có thông điệp riêng.

### Còn mở, chưa sửa

- `limit` là **theo từng nguồn rồi mới trộn** (mỗi nguồn tự chặn ở 500). Một
  nguồn ồn ào có thể làm cửa sổ thời gian co lại so với nguồn im. Đã ghi nhận,
  chưa đổi: sửa đúng là phân trang theo timestamp, đáng một việc riêng.
- `partial` mang nguyên văn lỗi DB ra client API.
- Tab `actions` có `role="tab"` nhưng thiếu `aria-controls` — giống hệt ba tab
  đã có, là khoảng trống sẵn có của trang chứ không do phase này.

### Cổng chất lượng sau khi sửa

`cargo clippy -D warnings` sạch · `cargo test --lib` **467 passed** (11 test cho
riêng `activity_feed`) · `pnpm lint/typecheck/check` 0 lỗi.

## Review vòng 2 — soát chính các bản sửa

Vòng 1 đọc trạng thái **trước** khi sửa, nên ba bản sửa CRITICAL/HIGH chưa ai
soát. Vòng 2 chỉ để trả lời: chúng có thật không.

**Kết quả: cả ba FIXED, và test thật sự đỏ nếu revert.** Reviewer kiểm chứng verb
giờ khớp cho **cả năm** variant `HealAction` do cùng dẫn xuất từ một `as_str`,
không chỉ hai variant được test.

### Vòng 2 tìm thêm — một HIGH do chính bản sửa của tôi tạo ra

**Lọc trước khi cắt, cắt trước khi lọc.** Bốn nguồn không diễn đạt được
`kind`/`target` bằng SQL nên tôi lọc sau khi đọc — nhưng vẫn đọc theo `limit` của
người dùng. Hệ quả: `target=abc123, limit=200` đọc 200 dòng heal **mới nhất của
mọi container** rồi bỏ dòng không khớp, trả về 0 trong khi dòng khớp nằm ngay
sau vị trí 200.

Sửa: khi có bộ lọc thì đọc trần của từng kho (500) và để bộ lọc quyết định cái gì
sống sót. Đọc 500 dòng sqlite là rẻ, và nhiều hơn mọi khung nhìn đã lọc cần tới.

### Một điểm tự khép trong lúc reviewer đang đọc

Reviewer nêu MEDIUM: vế SQL so khớp phân biệt hoa thường (`kind = ?`), vế trộn
thì không (`to_lowercase()`) — cùng một dòng bị hai vế phán khác nhau.

Trong lúc họ đọc, tôi đã đổi vế trộn sang `i.kind.as_str() == kind` vì một lý do
khác: `format!("{:?}")` là **đúng lớp lỗi vừa cắn ở defect 1** — nó khớp cho bốn
variant một từ hiện tại, nhưng một variant hai từ trong tương lai sẽ Debug ra
`networkchange` trong khi serde gửi `network_change`. Đổi sang `as_str` (nguồn sự
thật dùng chung với serde) khép luôn cả điểm phân biệt hoa thường. Có test khẳng
định `as_str` và chuỗi serde trùng nhau cho cả bốn variant.

### `real_heal_pair` chỉ ghim một trong hai bên (LOW)

Test đi qua `from_heal` thật, nhưng vế `activity_log` vẫn tự dựng bằng
`action.as_str()` — nếu ai đó đổi chỗ ghi trong `self_heal.rs`, test vẫn xanh và
lỗi trùng dòng quay lại. Chỗ ghi đó nằm trong một lời gọi DB mà unit test không
với tới.

Giải bằng cách dùng lại đúng cơ chế guard quét mã nguồn đã có ở
`activity_coverage.rs`: khẳng định thân `self_heal::perform` chứa
`action.as_str()` và **không** chứa `format!("{:?}", action)`.

### Chú thích `Suggested` tự mâu thuẫn

Nó viết "nothing was done, and nothing refused it" rồi map sang `Denied`. Đã viết
lại cho thẳng: `Denied` là từ duy nhất ở đây cho "máy không bị thay đổi", còn
việc đó là lời khuyên hay là từ chối thì nằm trong `detail` của chính luật.

### Cổng chất lượng sau vòng 2

`cargo clippy -D warnings` **0** · `cargo test --lib` **482 passed** ·
`pnpm lint/typecheck/check` **0 lỗi**.
