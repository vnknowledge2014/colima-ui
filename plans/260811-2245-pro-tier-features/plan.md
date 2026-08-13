---
title: "Pro tier — 3 tính năng trả phí (auto-fix compose, metrics history + alerts, self-healing)"
description: "Ba tính năng gate sau entitlement paid: auto-fix compose có patch áp dụng được, lưu trữ metrics + cảnh báo, và self-healing theo luật."
status: in-progress — code cả 3 phase xong 2026-08-13; còn hai việc CẦN MÁY THẬT: soak 24h (ph.2) và 5 kịch bản chạy tay (ph.3)
priority: P1
branch: "dev"
tags: [pro-tier, monetization, compose, metrics, self-healing]
# free-tier-foundation đã xong 2026-08-12 và archive
# (plans/archive/260811-2241-free-tier-foundation/) — collector cho phase 2 đã sẵn.
blockedBy: [260811-1930-subscription-teams-supabase-schema, 260810-2258-colimaui-commercial-foundation]
blocks: []
created: "2026-08-11T15:43:03.704Z"
createdBy: "ck:plan"
source: skill
---

# Pro tier — 3 tính năng trả phí

> **Sửa sau red-team 2026-08-11.** Bản đầu có 5 phase và một mô hình gating sẽ khoá chính khách trả tiền. Xem `plans/reports/from-red-team-to-owner-260811-2300-free-pro-tier-plans-adjudication-report.md`.

## Thay đổi so với bản đầu

| Thay đổi | Lý do |
|---|---|
| **Cắt phase "Cluster transfer + graph liên cụm"** | Không nằm trong danh mục 9 mục của `260810-2258-colimaui-commercial-foundation/plan.md:92-100`; bỏ qua entry gate Wave 2 và trần 3 mục (`phase-07:24,38,90`). Không ai phụ thuộc nó. Thêm nữa: **không tồn tại lớp targeting đa instance** — `docker --context` xuất hiện 0 lần trong repo, `detect_docker_host()` lấy profile chạy đầu tiên (`path_util.rs:172-202`). Xây nó là một plan riêng, không phải một phase. |
| **Gộp "Metrics store" vào "History + Alerts"** | Store không ship UI và có đúng một consumer. Tách ra là ranh giới giả. |
| **Bỏ 5 capability id, gate theo `paid`** | Capability đòi sidecar chưa tồn tại (`pro/bridge.rs:32-41` là placeholder) → mọi khách trả tiền bị khoá. Cũng là feature ladder mà `pricing-rationale.md:24-27` từ chối. |
| **Bỏ tuyên bố "gate ở backend"** | Không cưỡng chế được: `subscription_store` nhận `entitled: true` từ client (`subscription/mod.rs:126-143`). |

## Overview

Ba tính năng đủ điều kiện gate: không cái nào là wrapper quanh lệnh CLI miễn phí.

| Phase | Nỗi đau | Tận dụng sẵn (đã grep-verify) |
|---|---|---|
| 1. Compose auto-fix | "File compose hỏng, không biết sửa sao" | `compose_diagnose.rs`, `knowledge_bank.rs`, `DiagnosePanel.svelte` |
| 2. Metrics history + Alerts | "Đêm qua nó chết, tôi không biết vì sao" | Collector từ Free P3; `rusqlite` đã có (`Cargo.toml:38`) |
| 3. Self-healing | "Tôi phải ngồi canh và restart tay" | Phase 2 + nguồn sự kiện từ tiên quyết P6 |

## Phases

| Phase | Name | Status |
|-------|------|--------|
| 1 | [Compose auto-fix patch engine](./phase-01-compose-auto-fix-patch-engine.md) | **Xong 2026-08-13** — 7/20 corpus, 0 file hỏng thêm |
| 2 | [Metrics history + Alerts](./phase-02-metrics-time-series-store.md) | Code xong 2026-08-13 — **chờ soak 24h thật** (cách chạy đã ghi trong phase) |
| 3 | [Self-healing rules](./phase-03-activity-history-alerts.md) | **Code xong 2026-08-13** — 7 unit test; còn 5 kịch bản chạy tay trên máy thật |

Phase 2 → 3 là tuyến tính bắt buộc. Phase 1 độc lập.

## Dependencies

| blockedBy | Vì sao |
|---|---|
| ~~`260811-2300-platform-prerequisites`~~ | **Xong 2026-08-12**, đã archive (`plans/archive/260811-2300-platform-prerequisites/`). P7 giao `ProGate` gate theo `paid` + `onEntitlementChange`; P6 giao `docker_events` cho crash-loop/OOM ở phase 3; P5 giao `subscriber_count` mà collector của Free P3 dùng. |
| ~~`260811-2241-free-tier-foundation`~~ | **Xong 2026-08-12**, đã archive. Collector (`commands/metrics_collector.rs`) và `MetricWriter` mà phase 2 ghi vào đã sẵn sàng. |
| `260811-1930-subscription-teams-supabase-schema` | `proState.paid` phải chạy được end-to-end. |
| `260810-2258-colimaui-commercial-foundation` | **Chỉ phase 1** — cổng go/no-go của compose auto-fix spike, xem phần dưới. Phase 2 và 3 không chờ plan này. |

Quan hệ blockedBy ở frontmatter là mức plan (công cụ chỉ đọc được mức đó). Bảng trên là ánh xạ chính xác tới từng phase — dùng nó để biết cái gì thật sự chặn cái gì.

## Cổng chưa chạy — phase 1 phải đi qua trước

`260810-2258-colimaui-commercial-foundation/phase-05-compose-auto-fix-spike.md` đã đặt auto-fix là một **spike có cổng go/no-go**:

- `:44,52,59` — "Không đặt auto-patch đầy đủ làm cổng launch"
- `:144` — corpus đánh giá hiện có **0 file**
- `:19` — giữ comment YAML đã được kết luận là **bất khả thi** với `serde_yml 0.0.12` (`Cargo.toml:29`, chưa đổi)

Phase 1 ở đây **không được** cam kết lại điều đã bị bác. Nó bắt đầu bằng việc chạy cổng đó.

## Quyết định đã chốt với chủ dự án (2026-08-11)

| Quyết định | Chốt |
|---|---|
| Activity Monitor | Tách đôi: live = Free, lịch sử + cảnh báo = Pro |
| Graph view | Free (trang topology mới, xem Free P4) |
| Self-healing mặc định | Đề xuất (dry-run); tự động chỉ khi người dùng bật từng luật |
| Cluster transfer | **Cắt** |
| Entitlement | **Honor system.** Gate trên `paid`; không đầu tư chống giả mạo |

## Quy ước gating (áp dụng mọi phase)

- Gate trên `proState.paid`. **Không** dùng capability id cho ba tính năng này.
- Gate UI qua `ProGate`; gate **hành vi nền** bằng cách kiểm tra `paid` **trong executor tại điểm hành động**, không phải lúc đăng ký — `pro_status()` re-detect mỗi lần gọi (`pro/mod.rs:56-58`) và cache hết hạn theo đồng hồ (`subscription/cache.rs:49-62`).
- **Công tắc dừng của một tính năng không bao giờ nằm sau gate của chính nó.** `ProGate` locked render upsell *thay cho* children (`ProGate.svelte:45-51,63-80`).
- Mỗi tính năng cần một biến thể `GatedCapability` mới ở **cả** `GATED_ENUM` (Svelte) lẫn enum Rust (`telemetry/events.rs:30-34`) — tiên quyết P7 sở hữu việc này.
- Không tuyên bố gate được cưỡng chế ở backend.

## Acceptance chung

- [ ] Khách trả tiền **không có sidecar** dùng được cả ba tính năng.
- [ ] Entitlement hết hạn → hành vi nền dừng, không chỉ ẩn UI.
- [ ] Tài khoản Free thấy preview trung thực (dữ liệu ví dụ ghi rõ là ví dụ), không thấy chức năng.
- [ ] Pro offline vẫn dùng được tới khi cache hết hạn.
- [ ] `cargo clippy` + `pnpm check` sạch.

## Red Team Review

### Session — 2026-08-11
**Reviewer:** 4 lens (Security Adversary, Failure Mode Analyst, Assumption Destroyer, Scope & Complexity Critic), Full tier
**Findings áp dụng cho plan này:** 11 (11 accepted, 0 rejected) · 5 Critical, 4 High, 2 Medium
**Phân xử đầy đủ:** `plans/reports/from-red-team-to-owner-260811-2300-free-pro-tier-plans-adjudication-report.md`

| # | Finding | Severity | Disposition | Applied To |
|---|---------|----------|-------------|------------|
| 1 | 5 capability id resolve về hư vô → khoá 100% khách trả tiền; là feature ladder | Critical | Accept | Bỏ capability, gate `paid`; tiên quyết P7 |
| 2 | Gate backend là blob client gửi, không cưỡng chế được | Critical | Accept | Chốt honor system; bỏ tuyên bố backend gate |
| 3 | P1 nhảy qua cổng go/no-go chưa chạy của compose spike | Critical | Accept | **P1 thêm Bước 0 bắt buộc**; blockedBy commercial-foundation |
| 4 | LLM round-trip xoá secret, `compose_validate` không thấy | Critical | Accept | P1: LLM chỉ trả patch giới hạn + `key_diff_check` chặn |
| 5 | Schema metrics thiếu cột instance → dữ liệu trộn không gỡ được | Critical | Accept | P2: cột `instance` từ commit đầu |
| 6 | Crash-loop không có nguồn sự kiện (debounce nuốt burst) | High | Accept | Tách sang tiên quyết P6; P3 chờ |
| 7 | Quota self-heal chỉ trong bộ nhớ, reset khi restart | High | Accept | P3: quota vào `settings.db` |
| 8 | Công tắc tắt self-heal nằm sau gate của chính nó | High | Accept | P3: công tắc render ngoài `ProGate` + tray |
| 9 | Entitlement động nhưng đăng ký boot-time | High | Accept | P2, P3: kiểm tra `paid` tại điểm hành động |
| 10 | Ba bảng downsample thừa; `metrics.db` trộn dữ liệu prune được với cấu hình | Medium | Accept | P2: hai bảng; tách `settings.db` |
| 11 | P5 ngoài danh mục đã thoả thuận; `FixStrategy` trait thừa; P2 không có UI | Medium | Accept | **Cắt P5**; P1 dùng match; gộp P2 vào P3 |

**Quyết định của chủ dự án đi kèm:** honor system cho entitlement (finding 2 ghi nhận và cố ý không sửa `subscription/mod.rs:126-143`); cắt cluster transfer; self-healing mặc định Suggest.

### Whole-Plan Consistency Sweep
- Files reread: `plan.md`, `phase-01`, `phase-02`, `phase-03` (phase-04, phase-05 cũ đã xoá)
- Decision deltas checked: 11
- Reconciled stale references: 0 (các lần nhắc `MetricSink`/`serde_yaml`/`compose.autofix` còn lại đều nằm trong bảng đính chính, có chủ ý)
- Unresolved contradictions: 0

**Lưu ý đánh số file:** tên file `phase-02-metrics-time-series-store.md` và `phase-03-activity-history-alerts.md` giữ nguyên từ lần tạo đầu, nội dung đã đổi (gộp store+alerts vào 02, self-healing vào 03). Tiêu đề trong frontmatter là nguồn sự thật.

## Bước 0 của phase 1 đã chạy — 2026-08-12

Cổng go/no-go của compose auto-fix **đã chạy**, lần đầu tiên. Kết quả:
**go có điều kiện, phạm vi hẹp** — apply cho `undefined_reference` (60%) và
`schema` (50%); còn lại chỉ gợi ý. Tổng 35% trên corpus 20 file synthetic.

Đầy đủ: `plans/reports/from-autofix-gate-to-owner-260812-0025-compose-deterministic-fixability-verdict-report.md`

Kèm theo, ba phát hiện độc lập với auto-fix và đáng sửa dù plan này đi tiếp hay không:

- **`categorize()` có nhánh chết** (`compose_diagnose.rs:66-71`): `schema` bắt
  `must be a` nên `structure` không bao giờ chạy. Đang ship.
- **Biến chưa set validate sạch thành chuỗi rỗng** — mật khẩu rỗng không bị bắt.
- **Build context thiếu cũng pass** `docker compose config`.

Phần còn lại của plan vẫn bị chặn: **cả ba phase chờ prereq P7**, phase 2 chờ
Free P3, phase 3 chờ P6. Tất cả đều `pending`.
