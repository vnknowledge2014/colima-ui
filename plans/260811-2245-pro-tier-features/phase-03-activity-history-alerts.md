---
phase: 3
title: "Self-healing rules"
status: code xong 2026-08-13; còn 5 kịch bản chạy tay trên máy thật
priority: P2
dependencies: [2, "prereq:P6", "prereq:P7"]
effort: ""
---

> **Cổng bước 1 đã qua (2026-08-13).** Tiên quyết P6 có thật và có reconnect:
> `docker_events.rs:81` phát sự kiện thô trước debounce, `docker_state.rs:71-134`
> giữ vòng reconnect kèm ping liveness và phát `docker-reconnected`. Luật 5 do đó
> không tự cắt nguồn sự kiện của mình.
>
> **Hai sai lệch so với tài liệu này, cố ý:**
> 1. `knowledge.db` thay cho `settings.db` — repo không có `settings.db`; phase 2
>    đã quyết như vậy cho `alert_rules` (`alerts.rs:15-22`). Yêu cầu thật là
>    "retention không với tới được", và `knowledge.db` thoả.
> 2. Không có bảng quota riêng. Quota suy ra từ `heal_log` bằng cách đếm các
>    outcome đã chạm tới máy trong cửa sổ một giờ — bền vững qua restart do cấu
>    tạo, và không thể lệch với log theo cách một bảng đếm song song có thể.

# Phase 3: Self-healing rules

> Trước red-team đây là phase 4. Đánh số lại sau khi gộp store vào phase 2 và cắt cluster transfer.

## Overview

Từ "báo cho anh biết" (phase 2) sang "làm giúp anh". Luật dạng **điều kiện → hành động → giới hạn**, hiển thị rõ ràng.

**Quyết định đã chốt:** mặc định là **đề xuất** (dry-run). Tự động chỉ bật cho từng luật, do người dùng chủ động.

## Đính chính tiền đề (red-team)

| Bản đầu nói | Thực tế |
|---|---|
| Luật crash-loop bắt được container restart nhiều lần | **Không.** Không có nguồn sự kiện: `sse.rs:57-115` và `docker_state.rs:118-160` đều áp debounce trailing-edge 500 ms rồi vứt nội dung sự kiện. Restart loop nhanh gộp thành **một** lần fetch → luật crash-loop không bao giờ bắn, nên luật restart cứ chạy mãi. → **Tiên quyết P6.** |
| Quota/giờ chặn được hành động lặp | **Không qua vòng đời tiến trình.** Quota và đồng hồ vi phạm nằm trong bộ nhớ; restart app reset sạch. Chỉ `ExitRequested` được hook (`lib.rs:329-331`). Quota — biện pháp giảm thiểu số một — không chặn gì. |
| Công tắc tắt toàn cục bảo vệ người dùng | **Nó biến mất đúng lúc cần nhất.** `ProGate` locked render upsell *thay cho* children (`ProGate.svelte:45-51,63-80`). Entitlement hết hạn (`subscription/cache.rs:54`) → trang self-healing và công tắc biến mất, executor vẫn restart container và VM. |
| Restart VM là hành động an toàn | Nó **cắt đứt nguồn sự kiện của chính mình**: watcher hiện tại `break` vĩnh viễn khi stream đứt (`sse.rs:107-111`). Tiên quyết P6 phải có reconnect trước khi luật này được bật. |
| 5 luật, mỗi luật chạy được ở chế độ Auto | Luật 3 (đề xuất prune) và 4 (đề xuất mem limit) **vĩnh viễn là advisory** — chúng đề xuất theo bản chất. Tiêu chí "mỗi kịch bản phục hồi đúng ở Auto" không thoả mãn được với chúng. |

## Requirements

**Bộ luật — 5 cái, phân loại đúng bản chất**

| # | Trigger | Hành động | Auto được? |
|---|---|---|---|
| 1 | Container `unhealthy` quá N phút | Restart | Có |
| 2 | Crash-loop (restart >N lần/M phút) | **Dừng hẳn** + báo (restart tiếp là vô ích) | Có |
| 3 | Disk VM >X% | Đề xuất prune, kèm số thật từ `docker system df` | **Không — luôn advisory** |
| 4 | Container OOM-killed | Đề xuất tăng mem limit, số từ peak 7 ngày (phase 2) | **Không — luôn advisory** |
| 5 | VM colima không phản hồi | Restart colima | Có, nhưng chờ P6 reconnect |

**Bắt buộc**
- Không luật nào ở chế độ Auto khi cài mới.
- Quota/giờ **bền vững qua restart app** — lưu vào `settings.db` (phase 2), không phải bộ nhớ.
- Công tắc tắt toàn cục **luôn render**, kể cả khi entitlement hết hạn.
- Executor kiểm tra `paid` **tại điểm hành động**, không phải lúc đăng ký.
- Nhật ký mọi hành động, kể cả bị chặn bởi quota.

## Architecture

```
commands/self_heal.rs
  ├─ HealRule { id, trigger, action, mode: Suggest|Auto,
  │             max_per_hour, scope, enabled }
  │     mode chỉ nhận Auto với luật 1, 2, 5 — luật 3, 4 khoá ở Suggest
  ├─ Trigger ngưỡng      → tiêu thụ alert event của phase 2 (KHÔNG tự đánh giá metric)
  ├─ Trigger sự kiện     → docker_events::subscribe() (tiên quyết P6)
  ├─ HealExecutor
  │     ├─ if !proState.paid { dừng, ghi log }      ← tại điểm hành động
  │     ├─ quota check từ settings.db               ← bền vững
  │     └─ match mode { Suggest => tạo đề xuất, Auto => thực thi }
  │           MỘT đường code, khác đúng một nhánh
  └─ heal_log trong settings.db

UI:
  src/pages/settings/SelfHealing.svelte
    ├─ CÔNG TẮC TẮT TOÀN CỤC — render NGOÀI ProGate, luôn thấy
    └─ phần còn lại trong ProGate chế độ paid
  + tray menu: công tắc tắt thứ hai
```

**Vì sao công tắc nằm ngoài gate:** người mất quyền dùng tính năng vẫn phải có quyền **dừng** nó. Đây là quy ước chung do tiên quyết P7 đặt ra, phase này là ca dùng đầu tiên.

## Related Code Files

- Create: `src-tauri/src/commands/self_heal.rs`, `src-tauri/src/routes/self_heal.rs`
- Create: `src/pages/settings/SelfHealing.svelte`
- Create: `src/components/activity/HealSuggestionBanner.svelte`, `HealLog.svelte`
- Create: `src/lib/api/self-heal.ts`
- Modify: `src-tauri/src/commands/alerts.rs` (expose alert event stream)
- Modify: `src-tauri/src/commands/metrics_store.rs` (bảng `heal_rules`, `heal_log` trong `settings.db`)
- Modify: `src/pages/Settings.svelte`, `src-tauri/src/tray.rs`
- Modify: `src/locales/{en,vi,ja,zh}.json`

## Implementation Steps

1. Xác nhận tiên quyết P6 xong **và có reconnect** — luật 5 restart VM sẽ tự cắt nguồn sự kiện nếu không.
2. Schema `heal_rules` + `heal_log` trong `settings.db`, kèm bảng quota có timestamp.
3. Seed 5 luật, **tất cả `mode: Suggest`**, `enabled: true`. Luật 3, 4 đánh dấu `auto_capable: false`.
4. Trigger sự kiện: subscribe `docker_events` cho `die` + exit code (OOM) và health status.
5. `HealExecutor`: kiểm tra `paid` → quota (đọc/ghi `settings.db`) → thực thi hoặc tạo đề xuất.
6. `SuggestPrune` chạy `docker system df` lấy số thật. `SuggestMemLimit` lấy peak 7 ngày từ phase 2 × 1.3.
7. Công tắc toàn cục: render ngoài `ProGate`; thêm vào tray.
8. Đăng ký `onEntitlementChange` (tiên quyết P7) → hết hạn thì dừng executor ngay.
9. UI: chuyển luật sang Auto phải qua dialog nói rõ hậu quả ("luật này sẽ tự restart container mà không hỏi bạn").
10. Test 5 kịch bản; với luật 3, 4 chỉ test đường Suggest.

## Success Criteria

- [x] Cài mới trên DB trắng: **không luật nào ở Auto** — seed ghi cứng `'suggest'`,
      `INSERT OR IGNORE` nên seed lại không bao giờ đổi lựa chọn của người dùng.
- [x] Luật 3, 4: **không có đường Auto trong code**. Bốn lớp chặn: `execute()`
      trả `Suggested` trước khi tới `perform()`; `perform()` từ chối tường minh;
      `update_rule` chặn ghi `Auto`; `row_to_rule` hạ cấp khi đọc phải row hỏng.
      Test `advisory_actions_are_never_auto_capable`.
- [x] Mọi hành động có dòng log, kể cả bị chặn — `execute()` ghi log trên mọi
      nhánh outcome, `HealLog.svelte` hiển thị cả sáu loại outcome.
- [x] Quota không tự gia hạn: chỉ `Executed`/`Failed` tiêu quota. Test
      `only_actions_that_reached_the_machine_spend_quota`.
- [x] Crash-loop đếm đúng theo cửa sổ trượt, và reset sau khi bắn. Test
      `a_crash_loop_needs_its_deaths_inside_the_window`,
      `five_deaths_inside_the_window_trip_the_rule`, `tripping_resets_the_window`.
- [x] Công tắc toàn cục render **ngoài** `ProGate` (`SelfHealing.svelte`), và có
      bản thứ hai trong tray (`tray.rs`, `MenuAction::StopSelfHealing`).

**Còn lại — chỉ kiểm được trên máy thật, chưa chạy:**

- [ ] Luật 1, 2, 5: phục hồi đúng ở Auto, hoặc báo đúng lý do bỏ cuộc.
- [ ] Crash-loop bắt được container restart 5 lần trong 2 phút (P6 chạy thật).
- [ ] **Quota sống sót qua restart app**: quota 3, dùng 2, restart, lần thứ 4 vẫn bị chặn.
- [ ] **Entitlement hết hạn khi self-heal đang chạy → executor dừng**, công tắc vẫn thấy được.
- [ ] Công tắc toàn cục dừng ngay hành động đang chờ.
- [ ] `colima stop && colima start` (luật 5) → nguồn sự kiện tự nối lại, luật tiếp tục hoạt động.

## Risk Assessment

| Rủi ro | Giảm thiểu |
|---|---|
| **Hành động tự động sai trên môi trường thật** — rủi ro cao nhất | Mặc định Suggest; Auto bật từng luật qua dialog; quota bền vững; công tắc ở 2 nơi, ngoài gate |
| **Công tắc biến mất cùng entitlement** | Render ngoài `ProGate`; là success criteria |
| **Quota reset khi restart** | Lưu `settings.db`, không bộ nhớ; là success criteria |
| Restart VM cắt nguồn sự kiện của chính nó | Chờ tiên quyết P6 có reconnect; test kịch bản đầy đủ |
| Restart lặp vô hạn | Luật 2 dừng hẳn thay vì restart tiếp; quota chặn cứng |
| Phình phạm vi thành orchestrator | Giới hạn cứng 5 luật; không thêm action mới trong phase này |
