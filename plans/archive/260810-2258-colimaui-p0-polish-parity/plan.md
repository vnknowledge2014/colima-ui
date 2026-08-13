---
title: ColimaUI P0 — Polish & Parity (đã red-team)
description: >-
  Sửa các lỗ hổng UX hiện tại và bù gap tính năng so với OrbStack, sau khi
  red-team bác bỏ 4 giả định sai của bản plan đầu
status: completed
priority: P1
branch: dev
tags:
  - ux
  - polish
  - parity
  - p0
blockedBy:
  - 260810-2258-colimaui-security-hotfix
blocks:
  - 260810-2258-colimaui-commercial-foundation
created: '2026-08-10T16:26:43.352Z'
createdBy: 'ck:plan'
source: skill
---

# ColimaUI P0 — Polish & Parity (đã red-team)

## Overview

Bản plan này là **phiên bản 2**, viết lại sau khi 4 reviewer đối kháng bác bỏ các giả định của bản đầu. Đọc mục "Giả định đã bị bác bỏ" trước khi làm bất cứ việc gì — bản đầu có 4 dữ kiện sai và chúng là lý do biện minh cho 3 phase.

Nguồn: `plans/reports/from-research-to-product-strategy-260810-2212-colimaui-orbstack-benchmark-freemium-roadmap-report.md` và 4 báo cáo red-team `from-code-reviewer-to-planner-red-team-*-plan-review-report.md`.

## Giả định đã bị bác bỏ (bản v1 sai — đã tự kiểm chứng lại)

| Giả định v1 | Thực tế đã verify | Hệ quả |
|---|---|---|
| "Chỉ 2 file chạm toast → lỗi im lặng khắp nơi" | Grep phân biệt hoa-thường nên trượt `globalToast`. Thực tế **114 call site** trên 15 file. `Containers.svelte:96,113,206` đã có toast lỗi | Vấn đề **không phải** thiếu toast. Vấn đề là **chất lượng** lỗi và **rò rỉ bí mật**. Phase 1 viết lại hoàn toàn |
| "`ColimaError` chỉ cần thêm field optional" | `error.rs:4-13` là enum newtype `#[serde(tag="type", content="message")]`. Thêm field biến `message` thành object → `client.ts:39-43` render `[object Object]` | Cần hợp đồng lỗi **v2** tường minh, không phải sửa tại chỗ |
| "`routes/capabilities.rs` sẵn có, mở rộng nó (DRY)" | File đó là *schema API cho AI Agent* — 35 dòng `json!` tĩnh với phân loại `SAFE`/`DANGEROUS` | Phải tạo route mới. Bỏ chỉ dẫn "DRY" sai |
| "`useHotkeys.ts` sẵn có nên phím tắt rẻ" | `src/hooks/useHotkeys.ts:1` = `import { useEffect } from "react"`. Repo không có React. Zero importer | Code chết. Phải xoá và viết lại nếu cần |

Ngoài ra: `labels` = **0 match** trong cả `docker_state.rs` lẫn `commands/containers.rs` → grouping compose hiện **không có nguồn dữ liệu**; và `sse.rs:152` là mapper container **thứ hai** mà plan v1 không hề liệt kê.

## Thay đổi so với v1

- **Bỏ Phase 6 (container domains + DNS + CA + HTTPS proxy).** Lý do: 4 crate mới vi phạm nguyên tắc không thêm dependency của chính plan; ước lượng 8-12 ngày cho DNS server + CA + TLS proxy + ma trận resolver Linux là không thực tế; và nó mở SSRF/DNS-rebinding vào chính API server. Ghi nhận là **gap lớn nhất so với OrbStack vẫn còn tồn tại** — xem "Đã hoãn" bên dưới.
- **Bỏ framework `DataTable`.** `Containers.svelte` không hề có thẻ `<table>` (chỉ `class="table-actions"` tại `:378`); chỉ `Kubernetes.svelte` và `Dashboard.svelte` có. Trừu tượng 5 module cho 0 consumer là premature abstraction.
- **Bỏ `EmptyState` component mới + Dashboard checklist.** `SetupWizard.svelte` (568 dòng) + `GettingStartedTour.svelte` (171 dòng) + empty state rải ở 11 file đã là 2 bề mặt onboarding; thêm cái thứ ba làm tệ hơn. Giữ lại phần có giá trị thật: **capability detection**.
- **Bỏ editor YAML thô** trong Phase 5.
- Phase 1 đổi trọng tâm từ "thêm toast" sang "hợp đồng lỗi v2 + chuẩn hoá tại choke point".

## Nguyên tắc

- Mọi input mới (tray menu id, deep link) phải đi qua validator của plan hotfix bảo mật.
- Giữ dual-mode (Tauri IPC + HTTP :11420 + SSE). **Mọi mapper container phải sửa ở cả 2 đường** — `docker_state.rs` và `sse.rs`.
- Không thêm dependency mới trong plan này.
- Chuỗi hiển thị đi qua `src/lib/i18n.svelte.ts`; đếm cả 4 locale khi ước lượng.

## Phases

| Phase | Name | Status |
|-------|------|--------|
| 1 | [Error Contract v2](./phase-01-error-contract-v2.md) | Completed |
| 2 | [Capability Detection](./phase-02-capability-detection.md) | Completed |
| 3 | [Tray & Menubar](./phase-03-tray-menubar.md) | Completed |
| 4 | [Compose Labels & Grouping](./phase-04-compose-labels-grouping.md) | Completed |
| 5 | [Colima Config & Knowledge Base](./phase-05-colima-config-knowledge-base.md) | Completed |

## Ước lượng

| Phase | v1 | v2 (sau red-team) | Ghi chú |
|---|---|---|---|
| 1 | 4-6 ngày | **6-9 ngày** | Completed |
| 2 | 3-4 ngày | **3-4 ngày** | Completed |
| 3 | 2-3 ngày | **3-4 ngày** | Tăng: lớp pending-state, trạng thái chỉ Running\| Completed |
| 4 | 4-5 ngày | **3-4 ngày** | Completed |
| 5 | 4-5 ngày | **4-5 ngày** | Bỏ YAML editor, thêm migration KB |
| 6 | 8-12 ngày | **cắt** | |
| **Tổng** | 5-7 tuần | **~4 tuần** | |

## Acceptance Criteria

- [x] Hợp đồng lỗi v2 chạy giống hệt ở cả Tauri mode và browser mode.
- [x] Không chỗ nào trong UI hiển thị lỗi thô chưa chuẩn hoá.
- [x] Người dùng chưa cài Colima/Docker biết chính xác phải làm gì tiếp theo — capability detection + `doc_id` mở thẳng bài Help tương ứng.
- [x] Tray hoạt động và không double-invoke khi menu bị stale.
- [x] Container nhóm theo compose project **ở cả 2 mode**.
- [x] Sửa được config Colima mà không mất field người dùng tự thêm — có test round-trip chứng minh.
- [~] `cargo test` (72) và `npx vitest` (112) xanh; `pnpm run build` xanh. **`pnpm test` không tồn tại** như một script trong `package.json` — tiêu chí này viết sai từ đầu. `pnpm run check` báo 152 lỗi trên 39 file, **toàn bộ có từ trước** các phase này (import thừa, `src/hooks/useHotkeys.ts` là code React chết theo finding #9); không file nào do plan này tạo hoặc sửa nằm trong danh sách.

## Việc còn lại trước khi coi plan là đóng hẳn

1. Thêm script `test` vào `package.json` (hoặc sửa tiêu chí). Vitest với `environment: jsdom` hiện sập do undici 8 không tương thích Node 20.19 — cần ghim phiên bản hoặc nâng Node.
2. Dọn 152 lỗi typecheck có sẵn, bắt đầu bằng việc xoá `src/hooks/useHotkeys.ts` (finding #9).
3. Kiểm chứng thủ công Phase 5 trên máy có Colima: sửa CPU → restart → `colima status`; config hỏng có chủ ý bị chặn và còn `.bak`; ngắt mạng → Help vẫn đầy đủ.

## Đã hoãn (ghi nhận, không quên)

- **Container domains + auto HTTPS** — vẫn là gap lớn nhất so với OrbStack. Quay lại khi có nhu cầu thực tế được đo bằng số liệu, và làm như một plan riêng có spike bắt buộc + review bảo mật. Phương án rẻ hơn nếu cần sớm: reverse proxy HTTP theo Host header, dùng `/etc/hosts`, **không** cài CA, không DNS server.
- Table resize / tooltip / command palette — làm khi có trang thứ 2, thứ 3 thực sự cần bảng chung.

## Dependencies

- **Blocked by:** `260810-2258-colimaui-security-hotfix` — Phase 1 của plan này sẽ khuếch đại rò rỉ bí mật nếu hotfix chưa xong; Phase 3 và 5 cần validator tên profile.
- **Blocks:** `260810-2258-colimaui-commercial-foundation` — ở thời điểm launch (Phase 6 của plan đó).

## Red Team Review

### Session — 2026-08-10
**Findings:** 15 (15 accepted, 0 rejected) — sau dedupe từ 41 finding của 4 reviewer
**Severity:** 8 Critical, 7 High/Medium
**Reviewers:** Security Adversary, Failure Mode Analyst, Assumption Destroyer, Scope & Complexity Critic

| # | Finding | Sev | Disposition | Áp dụng |
|---|---|---|---|---|
| 1 | `ColimaError` là enum tag/content — không thể thêm field additively | Critical | Accept | Completed |
| 2 | "Chỉ 2 file chạm toast" sai — thực tế 114 call site | Critical | Accept | Completed |
| 3 | Lỗi hiển thị rò rỉ API key (`ai_chat.rs:174,180,477`) | Critical | Accept | Completed |
| 4 | Grouping compose không có nguồn dữ liệu (`labels` = 0 match); `sse.rs:152` là mapper thứ 2 | Critical | Accept | Completed |
| 5 | `instance_reader::ColimaConfig` chỉ 7 field, không flatten → ghi sẽ xoá config người dùng | Critical | Accept | Completed — `colima_config.rs` làm việc trên `serde_yml::Value`; test round-trip là điều kiện merge |
| 6 | Phase 6 (DNS+CA+proxy) 8-12 ngày không thực tế + SSRF/rebinding | Critical | Accept | **Cắt Phase 6** |
| 7 | Tên profile không validate → argv injection (`colima.rs:93-95`) | High | Accept | → plan hotfix bảo mật |
| 8 | `routes/capabilities.rs` là schema AI Agent, không phải tool detection | High | Accept | Phase 2 tạo route mới |
| 9 | `useHotkeys.ts` là code React chết | High | Accept | Ghi nhận, xoá |
| 10 | `DataTable` có 0 consumer — `Containers.svelte` không có `<table>` | High | Accept | **Cắt framework bảng** khỏi Phase 4 |
| 11 | `SYSTEM_INFO_CACHE` TTL 300s cứng, không invalidate, Mutex qua process spawn | High | Accept | Phase 2 dùng cache riêng |
| 12 | Tray: trạng thái chỉ Running\|Stopped; poller 5s; `sse.rs:195` publisher thứ 2 | High | Accept | Phase 3 thêm pending-state, sửa tiêu chí |
| 13 | 115 điểm `err(e.to_string())` trong `routes/*` bị bỏ sót | High | Accept | Phase 1 đưa vào phạm vi |
| 14 | Onboarding đã có 2 bề mặt (SetupWizard 568 dòng + Tour) | Medium | Accept | **Cắt EmptyState + checklist** khỏi Phase 2 |
| 15 | `start_instance` (`colima.rs:87-130`) đã ghi config → 2 nguồn ghi | Medium | Accept | Completed — YAML là nguồn sự thật; `start_instance` chỉ truyền cờ khi profile chưa có `colima.yaml` |

### Whole-Plan Consistency Sweep
Đã đọc lại `plan.md` + 5 phase file sau khi áp dụng. Kiểm tra:
- Phase 6 đã cắt khỏi bảng phases, ước lượng, acceptance criteria; ghi nhận ở mục "Đã hoãn" — nhất quán.
- `DataTable`/`EmptyState`/`OnboardingChecklist`/editor YAML thô: không còn xuất hiện như hạng mục cần tạo ở bất kỳ phase nào; các phase liên quan có dòng "**Không** tạo" tường minh.
- `routes/capabilities.rs`: không còn phase nào bảo mở rộng nó.
- `instance_reader::ColimaConfig`: Phase 5 nêu rõ **không** dùng cho việc ghi.
- `DiffView.svelte`: tạo ở Phase 5 của plan này, plan thương mại Phase 5 ghi "Reuse" — một nguồn duy nhất.
- Chuỗi phụ thuộc: hotfix → P0 → thương mại, khai báo 2 chiều khớp nhau.
- **Không còn mâu thuẫn chưa giải quyết.**

## Unresolved Questions

1. ~~Hợp đồng lỗi v2: giữ tương thích ngược hay đổi dứt điểm?~~ Đã đổi dứt điểm ở Phase 1.
2. Tray: app có chạy nền khi đóng cửa sổ chính không? — vẫn chưa chốt.
3. `useHotkeys.ts` — xoá luôn, hay viết lại bằng Svelte? Hiện không ai dùng nên xoá là mặc định. **Vẫn chưa xoá**; nó là một trong 152 lỗi typecheck còn tồn.
4. `package.json` không có script `test`. Thêm script hay sửa tiêu chí acceptance?
