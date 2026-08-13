---
phase: 7
title: Entitlement gate theo paid
status: completed
priority: P1
dependencies: []
effort: ''
---

# Phase 7: Entitlement gate theo paid

## Overview

`ProGate` hiện gate theo **capability do sidecar khai báo** (`pro.svelte.ts:111-113`). Sidecar chưa tồn tại: `pro/mod.rs:20-21` ghi rõ chưa bundle, `pro/bridge.rs:32-41` là placeholder tự chú thích "once the binary exists".

Hệ quả nếu dùng nguyên trạng cho tính năng Pro mới: **mọi khách trả tiền** đều rơi vào `isPaidButUnavailable` → màn hình "khôi phục Pro component" cho tính năng họ vừa mua.

## Quyết định của chủ dự án (2026-08-11): honor system

App MIT chạy trên máy người dùng thì không chống crack được thật. Chấp nhận điều đó:

- Gate trên `proState.paid` — đúng như `subscription/mod.rs:35-36` đã ghi: "the only field a gate may read".
- **Ngừng tuyên bố "gate ở backend"**. Sửa mọi chỗ trong plan/doc nói vậy.
- **Không** đầu tư chống giả mạo. Finding về `subscription_store` nhận `entitled: true` từ client (`subscription/mod.rs:126-143`) được ghi nhận và **cố ý không sửa** — nó không phải lỗ hổng khi ta không hứa cưỡng chế.

Quyết định này cũng gỡ luôn vi phạm "feature ladder": entitlement quay về đúng một boolean, như `docs/pricing-rationale.md:24-27` đã cam kết.

## Requirements

**Functional**
- `ProGate` gate theo `paid`, không theo capability id.
- Giữ nguyên phân biệt ba trạng thái cho **tính năng cần sidecar** nếu sau này có: `unlocked` / `needs_update` / `locked`. Đường capability **không xoá**, chỉ ngừng dùng cho 3 tính năng Pro mới.
- Telemetry funnel (`GatedCapability`) vẫn ghi được — nhưng nó là enum wire đóng (`telemetry/events.rs:30-34`), nên mỗi tính năng mới cần một biến thể mới, và **phase này sở hữu việc đó** (trước đây không phase nào nhận).

**Non-functional — quan trọng nhất**
- **Công tắc dừng của một tính năng không bao giờ được nằm sau gate của chính tính năng đó.** `ProGate` locked render upsell **thay cho** children (`ProGate.svelte:45-51,63-80`); khi entitlement hết hạn, trang self-healing và nút tắt toàn cục biến mất trong khi executor vẫn chạy.
- Hết hạn entitlement phải **dừng hành vi nền**, không chỉ ẩn UI.

## Architecture

```
ProGate.svelte
  ├─ view = paid ? "unlocked" : "locked"          ← cho tính năng Pro mới
  └─ giữ nhánh capability cho tính năng cần sidecar (chưa cái nào)

MỚI — quy ước bắt buộc cho mọi tính năng có hành vi nền:
  ┌─────────────────────────────────────────────────────┐
  │ 1. Gate UI     → ProGate (ẩn chức năng)             │
  │ 2. Gate hành vi → kiểm tra `paid` TRONG executor,   │
  │                   không phải lúc đăng ký            │
  │ 3. Công tắc dừng → LUÔN render, kể cả khi locked    │
  └─────────────────────────────────────────────────────┘

pro.svelte.ts
  └─ onEntitlementChange(cb)   ← MỚI: executor đăng ký để dừng khi hết hạn
```

**Vì sao kiểm tra trong executor chứ không lúc đăng ký:** `pro_status()` re-detect mỗi lần gọi (`pro/mod.rs:56-58`) và cache hết hạn theo đồng hồ (`subscription/cache.rs:49-62`). Đăng ký lúc boot nghĩa là nâng cấp phải khởi động lại app, còn hạ cấp thì vẫn chạy tiếp. Kiểm tra tại điểm hành động giải quyết cả hai.

## Related Code Files

- Modify: `src/components/ProGate.svelte` (nhánh `paid`, giữ nhánh capability)
- Modify: `src/lib/pro.svelte.ts` (thêm `onEntitlementChange`)
- Modify: `src-tauri/src/telemetry/events.rs:30-34` (thêm biến thể `GatedCapability` cho tính năng mới)
- Modify: `docs/pricing-rationale.md` (nếu có chỗ ngụ ý cưỡng chế backend — kiểm tra và sửa)
- Kiểm tra: `src-tauri/src/subscription/mod.rs:35-36` — xác nhận doc đã nói đúng, không cần sửa

## Implementation Steps

1. Đọc `ProGate.svelte` đầy đủ; xác định 3 nhánh view hiện tại và điểm rẽ.
2. Thêm chế độ gate theo `paid` (prop `mode="paid"` hoặc bỏ `capability` là mặc định `paid`), giữ nhánh capability nguyên vẹn.
3. `onEntitlementChange(cb)` trong `pro.svelte.ts`, bắn khi `paid` đổi.
4. Thêm biến thể `GatedCapability` cho 3 tính năng Pro (auto-fix, metrics, self-heal) — cả phía Rust lẫn `GATED_ENUM` phía Svelte.
5. Viết quy ước "công tắc dừng luôn render" thành comment trong `ProGate.svelte`, kèm lý do.
6. Test: giả lập cache hết hạn khi self-heal đang bật → executor dừng, công tắc vẫn thấy được.
7. Test: nâng cấp khi app đang chạy → tính năng mở khoá không cần restart.
8. Grep toàn bộ plan/doc tìm câu "gate ở backend" và sửa.

## Success Criteria

- [ ] Khách trả tiền, không có sidecar → dùng được cả 3 tính năng Pro. (Đây là bug bị chặn.)
- [ ] Entitlement hết hạn khi self-heal đang chạy → **hành vi nền dừng**, không chỉ ẩn UI.
- [ ] Công tắc tắt self-healing hiển thị được kể cả khi entitlement đã hết hạn.
- [ ] Nâng cấp/hạ cấp có hiệu lực **không cần khởi động lại app**.
- [ ] Không còn tài liệu nào tuyên bố gate được cưỡng chế ở backend.
- [ ] Nhánh capability vẫn còn và vẫn hoạt động (không xoá đường sidecar tương lai).
- [ ] Mỗi capability id mới có mặt đủ ở cả `GATED_ENUM` (Svelte) lẫn enum Rust.

## Risk Assessment

| Rủi ro | Giảm thiểu |
|---|---|
| **Công tắc dừng biến mất cùng gate** | Quy ước tường minh + test kịch bản hết hạn; là success criteria |
| Hành vi nền sống lâu hơn entitlement | Kiểm tra `paid` trong executor tại điểm hành động, không lúc đăng ký |
| Xoá nhầm đường capability, đóng cửa sidecar tương lai | Chỉ thêm nhánh, không xoá |
| Telemetry im lặng vì thiếu biến thể enum | Phase này sở hữu việc thêm; là success criteria |
| Ai đó sau này tưởng gate là cưỡng chế được | Ghi quyết định honor system vào doc, không chỉ trong plan |

## Đã implement — 2026-08-12

264 test Rust pass, `pnpm check` đúng baseline 143/36/36, 0 lỗi trong file của phase.

### Cơ chế

`capability` thành **tuỳ chọn**. Không truyền → gate hỏi `isPaid()`. Truyền →
giữ nguyên logic ba nhánh cho sidecar tương lai. Thêm prop `id` cho telemetry
funnel, vì id định danh tính năng không còn trùng với capability id.

Ba biến thể `GatedCapability` mới (`MetricsHistory`, `MetricsAlerts`,
`SelfHealing`) + test ghim đúng chuỗi wire, vì enum Rust và `GATED_ENUM` phía
Svelte là hai nửa của một ánh xạ ở hai ngôn ngữ — lệch nhau thì event bị bỏ im
lặng, funnel chỉ đơn giản là không báo gì.

`onEntitlementChange(cb)` cho executor nền: UI reactive tự re-render, nhưng thứ
*đang chạy* thì không giữ component nào và sẽ tiếp tục làm việc sau khi
entitlement hết hạn.

### Một mâu thuẫn sản phẩm lộ ra, đã hỏi chủ dự án

`Compose.svelte` gate cả tab diagnose sau `capability="compose.diagnose"` —
capability không ai khai → **diagnose khoá với tất cả mọi người**, kể cả khách
trả tiền, trong khi `/api/compose/diagnose` đã ship và chạy được.

Đây là quyết định sản phẩm chứ không phải lỗi cơ chế, nên hỏi thay vì tự sửa.
Chốt: **diagnose bỏ gate hẳn** (nó tồn tại hôm nay, và `pricing-rationale` đặt
Free là "toàn bộ app như hôm nay"; nó cũng không tốn gì mỗi lần dùng — Docker tự
validate + tra KB cục bộ, bước AI là key của người dùng). **Auto-fix** chuyển
sang gate theo `paid`, sẵn cho Pro P1.

Kéo theo: xoá 2 khoá locale mồ côi ở cả 4 file (còn 318 khoá, đồng bộ).

### Bốn sửa từ review

| Vấn đề | Vì sao quan trọng |
|---|---|
| Tab Diagnose (giờ miễn phí) vẫn đeo `ProBadge` | Đúng cái mislabelling phase này sửa, chỉ ở tầng trên: người dùng free thấy huy hiệu PRO trên thứ họ dùng được |
| `loadEntitlement` catch → `setPaid(false)` | Fail-closed, mâu thuẫn với chính chính sách `refreshEntitlement` ghi ngay bên dưới. Một lỗi IPC thoáng qua **hạ cấp khách trả tiền** — và giờ `paid` là gate duy nhất nên nó còn bảo cả việc nền dừng lại. Lỗi có sẵn, nhưng thay đổi này làm vùng ảnh hưởng thành thấy được với khách |
| Funnel chụp ảnh ở `onMount` | Entitlement resolve bất đồng bộ: khách trả tiền mà gate render trước sẽ bị ghi nhận là "chạm gate". Đổi sang `$effect` có cờ bắn-một-lần |
| `setPaid` lặp trên `Set` sống | Listener đăng ký giữa chừng bị gọi trong cùng lượt; listener gọi lại `setPaid` khiến phần còn lại nhận giá trị đã cũ. Snapshot trước khi thông báo |

Reviewer cũng fact-check được một câu dẫn nguồn của tôi: `pricing-rationale.md`
không nêu đích danh "diagnosis is Free" — nó nói Free là "the whole app as it
exists today". Cùng kết luận, nhưng comment đã sửa cho đúng nguồn.

### Ý kiến review tôi giữ nguyên, có lý do

Reviewer xếp ba biến thể enum mới và `onEntitlementChange` là **YAGNI** (chưa có
call site nào). Giữ, vì đó chính là deliverable của phase: Pro P2/P3 bị chặn ở
đây đúng để không phải mỗi phase tự thêm một nửa ánh xạ. Bỏ sót một nửa thì
hỏng **im lặng** — và đó là kiểu lỗi phase này được lập ra để chặn.

### Chưa verify được

- **Không chạy được test component nào.** jsdom 30 không nạp trên Node 20
  (`webidl.util.markAsUncloneable is not a function`), làm hỏng cả test có sẵn.
  Nên các tiêu chí cần mount thật — khách trả tiền thấy auto-fix mở khoá, nâng
  cấp có hiệu lực không cần restart, funnel bắn đúng lần — mới chỉ suy luận từ
  ngữ nghĩa `$derived` trên `$state` proxy, chưa chạy. Đây là vấn đề hạ tầng test
  riêng, không thuộc phase này.
- **Tiêu chí self-healing** (executor dừng, công tắc vẫn thấy) không kiểm được:
  self-healing là Pro P3, chưa tồn tại. Phase này giao *quy ước* và *cơ chế*.
