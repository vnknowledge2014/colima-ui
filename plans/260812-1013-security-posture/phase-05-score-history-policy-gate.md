# Phase 5: Lịch sử điểm + policy gate + security alert

**Trạng thái:** XONG 2026-08-13 — xem "Đã dựng" ở cuối.

**Tier:** Pro · **Chờ:** phase 2, Pro P2 (`alerts.rs`, `metrics_store.rs`)

## Overview

Điểm một lần là thông tin. Điểm theo thời gian là **quản trị**. Phase này biến
scan từ hành động rời rạc thành đường cơ sở có thể giữ.

Ba thứ, cùng một phase vì cùng đọc/ghi một kho:
1. Lịch sử điểm theo image, theo thời gian.
2. Policy gate — "cảnh báo/chặn khi chạy image dưới ngưỡng X".
3. Security alert — CVE mới xuất hiện trên image bạn đang chạy.

## Đính chính tiền đề trước khi thiết kế

| Dễ giả định sai | Thực tế |
|---|---|
| Nhét scan history vào `metrics.db` của Pro P2 | Sai vòng đời. `metrics.db` bị prune liên tục (raw giữ 1h). Scan history là bản ghi thưa, giá trị **tăng** theo tuổi |
| Nhét vào `settings.db` | Cũng sai. `settings.db` là cấu hình người dùng tự tay tạo, không bao giờ prune. Scan result thì có prune |
| Dựng đường ống alert thứ hai | **Cấm.** Pro P2 đã có `alerts.rs` + `alert_events`. Security alert là **một loại** alert |
| Điểm cũ so được với điểm mới | Chỉ khi cùng `(scanner, pack_version)`. Khác pack → **đứt đoạn biểu đồ**, không nội suy |

**Kết luận: `security.db` riêng, file thứ ba.** Theo đúng lý lẽ hai-DB mà Pro P2
đã lập: dữ liệu prune được và cấu hình người dùng không ở chung file. Ở đây có
ba vòng đời, nên ba file — `policy_rules` đi vào `settings.db` cùng
`alert_rules`, scan history đi vào `security.db`.

## Requirements

**Functional**
- Lưu mỗi lần scan: digest, điểm, breakdown, `ScoreInputs`, số finding theo severity. **Không** lưu toàn bộ findings (dung lượng).
- Biểu đồ điểm theo thời gian cho một image, **đứt đoạn** khi `pack_version` đổi.
- Policy: ngưỡng điểm tối thiểu + severity tối đa cho phép, phạm vi toàn cục hoặc theo image pattern.
- Vi phạm policy → cảnh báo qua đường alert của Pro P2. **Cảnh báo, không chặn** ở bản đầu.
- Rescan định kỳ image đang chạy → CVE mới xuất hiện thì bắn alert.

**Non-functional**
- Rescan định kỳ **mặc định TẮT**. Bật là quyết định của người dùng — nó tốn CPU và băng thông.
- Executor rescan kiểm `paid` **tại điểm chạy**, không phải lúc đăng ký (quy ước prereq P7).
- Công tắc tắt rescan **luôn render**, kể cả khi entitlement hết hạn. Bài học P7: công tắc dừng không được nằm sau gate của chính nó.
- Hết hạn Pro → rescan **dừng chạy**, không chỉ ẩn UI.

## Architecture

```
commands/security_history.rs
  ├─ security.db  — scan_runs (digest, image_ref, score, breakdown_json,
  │                            pack_version, db_snapshot_date, scanner,
  │                            counts_by_severity, scanned_at)
  │                 KHÔNG có bảng findings đầy đủ
  ├─ retention: giữ N bản ghi/digest (mặc định 50) + trần dung lượng
  └─ reader: Connection riêng (không dùng chung Mutex với writer) — bài học Pro P2

commands/security_policy.rs
  ├─ settings.db — policy_rules (đi cùng alert_rules, KHÔNG prune)
  ├─ PolicyRule { pattern, min_score, max_severity, action: Warn }
  │     action chỉ có Warn ở bản này. Block là quyết định riêng, xem Risks
  └─ evaluate_on_scan(result) -> Vec<Violation>

commands/security_watch.rs
  ├─ scheduler: mặc định TẮT; bật thì rescan image đang chạy mỗi N giờ
  ├─ mỗi vòng: check `paid` TRƯỚC khi chạy → false thì dừng hẳn scheduler
  └─ finding mới (không có ở lần scan trước) → alerts::emit(...)
       DÙNG LẠI alerts.rs của Pro P2, không tạo kênh mới

UI: src/pages/Security.svelte → tab "History" + "Policy"  (ProGate paid)
```

**Vì sao không lưu findings đầy đủ:** một image nặng có hàng nghìn finding. 50
bản ghi × 20 image × 2000 finding = dữ liệu vô ích, vì cái người dùng cần là
"điểm đang tăng hay giảm" và "có gì mới". Cái "có gì mới" tính được từ scan
hiện tại so với **một** snapshot findings gần nhất — giữ đúng một bản, không phải lịch sử.

**Vì sao đứt đoạn biểu đồ khi pack đổi:** nối liền hai điểm chấm bằng hai thước
khác nhau là đồ thị nói dối. Đứt đoạn + nhãn "rule pack v2 từ đây" là trung thực
và người dùng hiểu ngay.

## Related Code Files

- Create: `src-tauri/src/commands/security_history.rs`, `security_policy.rs`, `security_watch.rs`
- Create: `src/components/security/{ScoreTrendChart,PolicyEditor,SecurityAlertLog}.svelte`
- Modify: `src-tauri/src/commands/alerts.rs` — thêm nguồn alert `Security` (dùng lại emit + `alert_events`)
- Modify: `src-tauri/src/routes/security.rs`
- Modify: `src/pages/Security.svelte`, `src/lib/api/security.ts`
- Modify: `src/lib/pro.svelte.ts` — đăng ký `onEntitlementChange` để dừng scheduler (hook do prereq P7 tạo)

## Implementation Steps

1. Kiểm `alerts.rs` của Pro P2 đã ship và expose được đường emit. Chưa → **dừng**.
2. `security.db` + schema + retention. Test retention không đụng `settings.db`.
3. Ghi bản ghi sau mỗi scan của phase 1.
4. `ScoreTrendChart` — đứt đoạn theo `pack_version`, nhãn rõ ràng.
5. `PolicyRule` + `evaluate_on_scan` → `Warn`. `PolicyEditor` UI.
6. `security_watch` scheduler: mặc định tắt, check `paid` trong vòng lặp, `onEntitlementChange` dừng.
7. Diff findings với snapshot trước → alert cho finding mới, qua `alerts.rs`.
8. Công tắc tắt watch đặt **ngoài** `ProGate` — test tường minh việc này.

## Tests

- Retention xoá scan cũ, `policy_rules` trong `settings.db` còn nguyên.
- `pack_version` đổi giữa hai lần scan → chart đứt đoạn, không nối.
- Entitlement chuyển `paid=false` khi scheduler đang chạy → vòng tiếp theo dừng, có log.
- Entitlement hết hạn → công tắc tắt watch **vẫn hiển thị và bấm được**.
- Finding mới xuất hiện → đúng một alert, không phải một alert mỗi lần rescan.
- Policy vi phạm → cảnh báo, container **vẫn chạy** (action=Warn).

## Risks

| Rủi ro | Xử lý |
|---|---|
| `action: Block` chặn nhầm container production của người dùng | **Không ship Block ở phase này.** Đưa vào chỉ khi Warn đã chạy đủ lâu và người dùng yêu cầu. Chặn sai một lần là mất niềm tin vĩnh viễn |
| Rescan định kỳ đốt CPU/băng thông im lặng | Mặc định tắt; khi bật hiện rõ chu kỳ và lần chạy kế tiếp |
| Alert spam khi CVE DB cập nhật lớn (hàng trăm CVE mới cùng lúc) | Gộp theo image: một alert "12 CVE mới trên nginx:1.25", không phải 12 alert |
| Ngưỡng mặc định đặt bừa | **Unresolved #4 trong plan.md** — lấy phân bố điểm thật từ phase 2 rồi mới đặt số |

## Success Criteria

- Scan cùng image 3 lần qua vài ngày → thấy đường điểm, hiểu được biến động.
- CVE mới trên image đang chạy → đúng một alert gộp, qua đường alert đã có.
- Huỷ Pro → rescan nền dừng thật, kiểm chứng bằng log, không chỉ ẩn UI.


## Đã dựng — 2026-08-13

Bước 1 (cổng chặn) đạt: `alerts.rs` có `record` + `publish_sse_event` và
`entitled_now_cached()` — đủ để cắm vào, không cần kênh thứ hai.

### Tệp

| Tệp | Vai trò |
|---|---|
| `commands/security_history.rs` | `security.db`, `scan_runs`, `finding_snapshot`, retention 50/digest |
| `commands/security_policy.rs` | `policy_rules` trong `settings.db`, `evaluate()` thuần, chỉ `Warn` |
| `commands/security_watch.rs` | Scheduler mặc định TẮT, kiểm entitlement trong vòng chờ |
| `commands/alerts.rs` | +2 biến thể metric, cột `image_ref`, `emit_security()` |
| `components/security/score-trend.ts` + `.test.ts` | Tách series theo pack/scanner, 7 test |
| `components/security/{ScoreTrendChart,PolicyEditor,ScheduledRescanSwitch}.svelte` | UI |

### Ba quyết định đáng ghi

1. **Ghi lịch sử đặt trong `audit_image_blocking`**, không đặt ở từng transport.
   Có ba đường vào (2 lệnh + watcher); một đường quên ghi là biểu đồ thủng lỗ.
   Chỉ ghi khi có entitlement — theo tiền lệ `metrics_store` (Free không có file).
2. **Đứt đoạn tính theo *liên tiếp*, không theo *bằng nhau*.** Người dùng lên v2
   rồi quay về v1 có **ba** đoạn, không phải hai — gộp hai quãng v1 lại là vẽ một
   đường thẳng xuyên qua quãng v2 như thể nó chưa từng xảy ra. Có test.
3. **`Severity::Unknown` không bao giờ vượt ngưỡng.** Scanner không phân loại
   được không phải bằng chứng rằng thứ đó nguy hiểm.

### Một điểm hở tự phát hiện và đã bịt

Bản đầu kiểm entitlement **sau** khi ngủ hết chu kỳ. Với chu kỳ 12 giờ, một
subscription hết hạn giữa chừng vẫn mua được trọn một vòng quét. Đã chuyển phép
kiểm vào trong vòng đánh thức 30 giây — lapse dừng trong nửa phút, **không cần
frontend hợp tác**: một cơ chế dừng phụ thuộc vào việc trang có đang mở hay
không thì không phải cơ chế dừng.

Hệ quả: `stop_for_entitlement_loss()` mà plan hình dung trở thành thừa và đã xoá
thay vì để lại làm dead code. `onEntitlementChange` vẫn dùng ở frontend, nhưng
chỉ để **hiển thị đúng sự thật** (`enabled` vs `running`), không phải để dừng.

### Công tắc nằm ngoài `ProGate` — có kiểm chứng

`ScheduledRescanSwitch` render cho mọi người. Backend cưỡng chế cùng một bất đối
xứng: bật cần entitlement, **tắt thì không bao giờ**. Có test cho cả bốn tổ hợp
(bật/tắt × có/không entitlement). Khi Pro hết hạn mà preference vẫn bật, UI nói
rõ "đang bật nhưng không chạy" thay vì một công tắc nói dối.

### Lệch plan, có chủ đích

- **Không ship ngưỡng mặc định nào** (Unresolved #4). Một con số đặt bừa mà bắn
  cảnh báo sẽ dạy người dùng bỏ qua alert — tệ hơn là không có policy. Người dùng
  tự đặt bar; UI từ chối tạo rule không có ngưỡng nào.
- **`SecurityAlertLog.svelte` không dựng.** Alert security đi vào đúng
  `alert_events` và đúng luồng SSE `alert.fired` mà log alert hiện có đọc — dựng
  một log thứ hai là đúng thứ plan cấm ở dòng "Dựng đường ống alert thứ hai:
  **Cấm**". Cột `image_ref` đã có để log hiện có phân biệt được.
- **Không có trần dung lượng riêng**, chỉ 50 bản ghi/digest. Một hàng vài trăm
  byte × 50 × số image là vài trăm KB; thêm một cơ chế trần thứ hai là phức tạp
  không mua được gì.

### Cổng chất lượng

`pnpm lint` · `pnpm typecheck` · `pnpm check` (0/0) · `cargo clippy -D warnings`
sạch · `cargo test --lib` **437 passed** · `score-trend.test.ts` 7 passed.

**Lưu ý môi trường (có sẵn, không do phase này):** `pnpm test` (vitest + jsdom)
chết ngay khi khởi động — `webidl.util.markAsUncloneable is not a function`,
undici 8 trên Node 20.19.1. Test cũ `security-posture.test.ts` cũng hỏng y hệt.
Chạy được bằng `pnpm vitest run --environment node` cho test thuần logic.
