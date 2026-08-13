---
title: "Free tier — 4 tính năng nền (transfer, diagnostic bundle, activity live, graph)"
description: "Bốn tính năng ở tier Free: thứ CLI đã làm miễn phí (docker cp/save), công cụ hỗ trợ tự phục vụ, nền thu thập metrics, và một trang topology mới."
status: completed
completed: '2026-08-12'
priority: P1
branch: "dev"
tags: [free-tier, transfer, diagnostics, monitoring, topology]
blockedBy: []
blocks: [260811-2245-pro-tier-features]
created: "2026-08-11T15:42:58.786Z"
createdBy: "ck:plan"
source: skill
---

# Free tier — 4 tính năng nền

> **Sửa sau red-team 2026-08-11.** Phiên bản đầu của plan này dựa trên nhiều tuyên bố "đã có sẵn" mà grep chứng minh là sai. Xem `plans/reports/from-red-team-to-owner-260811-2300-free-pro-tier-plans-adjudication-report.md`. Phần bị ảnh hưởng nặng nhất là phase 4, đã viết lại hoàn toàn.

## Overview

Bốn tính năng **không được gate**, vì ba lý do khác nhau:

| Phase | Vì sao Free |
|---|---|
| 1. File transfer + export TAR | `docker cp` / `docker save` là lệnh miễn phí. `docs/pricing-rationale.md` đã tuyên bố không bán wrapper quanh lệnh free. Đây là phễu thu hút, không phải doanh thu. |
| 2. Diagnostic bundle + report bug | Công cụ hỗ trợ. Nó phục vụ chính bạn: mỗi bug report là một signature lỗi thật nuôi Knowledge Bank. |
| 3. Activity Monitor (live) | `docker stats` đã có. Cái CLI *không* có là lịch sử + cảnh báo → phần đó ở Pro. Phase này dựng **collector** mà Pro ghi xuống đĩa. |
| 4. Trang topology mới | Đồ thị container/network/volume/compose. Khó bán một mình; là màn hình gây ấn tượng. |

**Ràng buộc xuyên suốt:** không phase nào ở plan này được import `pro.svelte.ts` hay bọc `ProGate`.

## Phases

| Phase | Name | Status |
|-------|------|--------|
| 1 | [File transfer + Export image TAR](./phase-01-file-transfer-export-image-tar.md) | Done (2026-08-12) |
| 2 | [Diagnostic bundle + Report bug](./phase-02-diagnostic-bundle-report-bug.md) | Done (2026-08-12) |
| 3 | [Activity Monitor (live)](./phase-03-activity-monitor-live.md) | Done (2026-08-12) |
| 4 | [Trang topology mới](./phase-04-graph-view-trong-m-t-c-m.md) | Done (2026-08-12) |
| 5 | [Bố cục đồ thị bằng ELK + nhóm theo network](./phase-05-graph-layout-elk.md) | Done (2026-08-12) |
| 6 | [Ngôn ngữ hình ảnh Topology: icon, chip, rail, tham chiếu chéo](./phase-06-topology-visual-language.md) | Done (2026-08-12) |
| 7 | [Service compose chưa tạo + cạnh depends_on](./phase-07-compose-declared-services.md) | Done (2026-08-12) |

## Dependencies

**Tiên quyết đã xong** (2026-08-12) — `plans/archive/260811-2300-platform-prerequisites/`.
Gỡ khỏi `blockedBy` vì đã thoả mãn; bảng dưới giữ lại để biết phase nào đã dựa
vào phần nào, khi cần truy nguồn:

| Phase ở đây | Chờ phase tiên quyết |
|---|---|
| 1 | P1 (token hardening), P4 (streaming command + path policy) |
| 2 | P1 (token), P2 (redact mở rộng), P3 (crash persistence) |
| 3 | P5 (SSE subscriber registry) |
| 4 | — không chờ gì |
| 5 | — chỉ chờ phase 4 (đã Done) |
| 6 | — chờ 4 và 5 (đã Done) |
| 7 | — chờ 4, 5, 6 (đã Done) |

**blocks** `260811-2245-pro-tier-features` — Pro ghi lên collector do phase 3 ở đây dựng.

## Nền tảng tái dùng — đã grep-verify

| Có sẵn, xác nhận đúng | Dùng ở |
|---|---|
| `redact.rs::redact` (sau khi mở rộng ở tiên quyết P2) | Phase 2 |
| `crash.rs::latest_report` (sau khi xây ở tiên quyết P3) | Phase 2 |
| `containers.rs::all_container_stats:606`, `container_top:624` | Phase 3 |
| `sse.rs::publish_sse_event:29` | Phase 3 |
| `validation.rs::assert_path_within:169` — đã viết tử tế, dùng nguyên | Phase 1 |
| `compose_diagnose.rs::error_signature:83` | Phase 2 |

**Đã gỡ khỏi danh sách (tuyên bố sai):**
- ~~`tauri-plugin-dialog` đã có trong capabilities~~ — vắng ở `capabilities/default.json`, `Cargo.toml`, `package.json`. Thêm nó là thay đổi trust boundary, phase 1 sở hữu việc đó.
- ~~`ClusterTopology.svelte` + `XRay.svelte` là hai trang chồng lấn~~ — chúng là **component con của `Kubernetes.svelte`**, không phải route. Xem phase 4.
- ~~`crash.rs` chỉ cần thêm hàm đọc~~ — nó không ghi gì cả.

## Acceptance chung

- [ ] `cargo clippy` và `pnpm check` sạch trên toàn bộ file đụng tới.
- [ ] Không file nào trong plan này import `lib/pro.svelte` hoặc `components/ProGate.svelte`.
- [ ] Chuỗi UI mới có đủ 4 locale (`en`, `vi`, `ja`, `zh`).
- [ ] Mọi tuyên bố "đã có sẵn" trong các phase đều grep-verify được tại thời điểm implement.

## Red Team Review

### Session — 2026-08-11
**Reviewer:** 4 lens (Security Adversary, Failure Mode Analyst, Assumption Destroyer, Scope & Complexity Critic), Full tier
**Findings áp dụng cho plan này:** 9 (9 accepted, 0 rejected) · 4 Critical, 3 High, 2 Medium
**Phân xử đầy đủ:** `plans/reports/from-red-team-to-owner-260811-2300-free-pro-tier-plans-adjudication-report.md`

| # | Finding | Severity | Disposition | Applied To |
|---|---------|----------|-------------|------------|
| 1 | Token API phát không → route ghi mới thành leo thang đặc quyền | Critical | Accept | Tách sang tiên quyết P1; P1, P2 chờ |
| 2 | `redact()` phủ 1/4 lớp secret; bundle không đạt tiêu chí của chính nó | Critical | Accept | Tách sang tiên quyết P2; P2 chờ |
| 3 | `crash.rs` không ghi gì xuống đĩa | Critical | Accept | Tách sang tiên quyết P3; P2 chờ |
| 4 | P4 xoá 2 component Kubernetes trên tiền đề sai | Critical | Accept | **P4 viết lại hoàn toàn** |
| 5 | SSE không có sổ sách subscriber | High | Accept | Tách sang tiên quyết P5; P3 chờ |
| 6 | SSE mất mẫu im lặng khi lag | High | Accept | Tiên quyết P5; P3 xử lý `stream-lagged` |
| 7 | `tauri-plugin-dialog` không tồn tại | High | Accept | P1 sở hữu việc thêm |
| 8 | `Command::output()` buffer 2 GB vào RAM | High | Accept | Tách sang tiên quyết P4; P1 dùng streaming |
| 9 | Trait `MetricSink` là trừu tượng non; `docker load` thiếu tiêu chí | Medium | Accept | P3 dùng `Option<MetricWriter>`; P1 thêm tiêu chí import |

**Đính chính một finding:** reviewer báo "`validation.rs` không có API confinement dùng được". Đọc code thì `assert_path_within:169-195` **đã được viết tử tế** (canonicalize parent, chống `..` và symlink). Khoảng trống thật là plan không nêu thư mục base — vấn đề chính sách, không phải code. Ghi lại ở tiên quyết P4.

### Whole-Plan Consistency Sweep
- Files reread: `plan.md`, `phase-01`, `phase-02`, `phase-03`, `phase-04`
- Decision deltas checked: 9
- Reconciled stale references: 1 (bảng rủi ro `phase-03` còn nhắc trait `MetricSink`)
- Unresolved contradictions: 0
