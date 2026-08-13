---
title: Nền tảng tiên quyết — 7 hạng mục hai plan Free/Pro đang giả định là đã có
description: >-
  Bảy hạng mục nền mà red-team 2026-08-11 chứng minh là chưa tồn tại, dù hai
  plan Free/Pro đều viết như thể đã có. Không phase nào của hai plan đó bắt đầu
  được trước khi phần tương ứng ở đây xong.
status: completed
completed: '2026-08-12'
priority: P1
branch: dev
tags:
  - prerequisites
  - security
  - redaction
  - observability
  - entitlement
blockedBy: []
blocks:
  - 260811-2241-free-tier-foundation
  - 260811-2245-pro-tier-features
created: '2026-08-11T16:06:35.300Z'
createdBy: 'ck:plan'
source: skill
---

# Nền tảng tiên quyết

## Overview

Red-team ngày 2026-08-11 (4 reviewer, Full tier) tìm ra **9 finding Critical**, phần lớn cùng một dạng: hai plan Free/Pro tuyên bố tái dùng thứ đã có, nhưng thứ đó không tồn tại hoặc không làm điều được mô tả.

Plan này xây đúng bảy thứ đó. Nó **không có tính năng người dùng nào** — đó là chủ ý. Mỗi phase gỡ một tiền đề sai cụ thể.

> Cột đầu dùng tiền tố `P` có chủ ý: `ck plan check` coi một bảng có cột đầu là
> số trần là bảng trạng thái phase và ghi đè cột cuối. Nó đã làm đúng thế với
> bảng này một lần.

| # | Tiền đề sai | Bằng chứng |
|---|---|---|
| P1 | "API cục bộ an toàn" | `/api/auth/token` ở router public, phát token không xác thực — `api_server.rs:256`, `routes/system.rs:263-265` |
| P2 | "`redact()` che secret" | Phủ 1/4 lớp; regex neo `[?&]` bỏ sót `KEY=VALUE` — `redact.rs:26,49-53,54-72` |
| P3 | "`crash.rs` lưu crash" | Chỉ `eprintln!` — `crash.rs:48-55` |
| P4 | "`Command` chạy được lệnh dài + `validation.rs` giới hạn được path" | `Command::output()` buffered — `helpers.rs:99-125`. (`assert_path_within` thì **đã** dùng được — xem phase 4.) |
| P5 | "SSE đếm được subscriber" | Broadcast channel process-global không sổ sách; `Lagged(n)` → `None` — `sse.rs:17-27`, `routes/misc.rs:7-12` |
| P6 | "Có nguồn sự kiện container" | Debounce 500 ms nuốt burst — `docker_state.rs:118-160` |
| P7 | "Capability gate hoạt động" | Sidecar chưa tồn tại; `pro/bridge.rs:32-41` là placeholder |

## Phases

| Phase | Name | Status |
|-------|------|--------|
| 1 | [Hardening token API cục bộ](./phase-01-hardening-token-api-c-c-b.md) | Completed |
| 2 | [Mở rộng phủ sóng redact](./phase-02-m-r-ng-ph-s-ng-redact.md) | Completed |
| 3 | [Lưu crash report xuống đĩa](./phase-03-l-u-crash-report-xu-ng-a.md) | Completed |
| 4 | [Streaming command + path confinement](./phase-04-streaming-command-path-confinement.md) | Done (2026-08-12) |
| 5 | [SSE subscriber registry + lag hiển thị](./phase-05-sse-subscriber-registry-lag-hi-n-th.md) | Completed |
| 6 | [Nguồn sự kiện container không debounce](./phase-06-ngu-n-s-ki-n-container-kh-ng-debounce.md) | Completed |
| 7 | [Entitlement gate theo paid](./phase-07-entitlement-gate-theo-paid.md) | Completed |

**Phase 1 làm trước tất cả** — nó là lỗ hổng đang tồn tại trong code hiện tại, không phải nợ của tính năng tương lai. Các phase còn lại độc lập nhau, làm song song được.

## Dependencies

- **blocks** `260811-2241-free-tier-foundation`: P1→(Free P1, P2), P2→(Free P2), P3→(Free P2), P4→(Free P1), P5→(Free P3)
- **blocks** `260811-2245-pro-tier-features`: P5→(Pro P2), P6→(Pro P3), P7→(toàn bộ Pro)

Không phải mọi phase của hai plan kia đều chờ cả 7. Bảng ánh xạ trên là chính xác — dùng nó thay vì chờ toàn bộ plan này xong.

## Quyết định đã chốt với chủ dự án (2026-08-11)

**Entitlement là honor system.** App MIT chạy trên máy người dùng thì không chống crack được thật. Gate trên `proState.paid`; ngừng tuyên bố "gate ở backend"; không đầu tư chống giả mạo. Xem phase 7.

Kéo theo: finding về `subscription_store` nhận `entitled: true` từ client (`subscription/mod.rs:126-143`) được **ghi nhận và chấp nhận**, không sửa. Nó không còn là lỗ hổng khi ta không tuyên bố cưỡng chế.

## Acceptance chung

- [x] Mỗi phase có test chứng minh tiền đề cũ đã sai và tiền đề mới đúng.
- [x] Sau plan này, mọi tuyên bố "đã có sẵn" trong hai plan Free/Pro đều grep-verify được.
- [~] `cargo clippy` + `pnpm check` sạch — **không đánh giá được như đã viết**, xem dưới.

## Nghiệm thu khi đóng plan — 2026-08-12

Bảy tiền đề sai ban đầu, kiểm lại bằng grep:

| # | Tiền đề cũ | Bằng chứng đã sửa |
|---|---|---|
| P1 | `/api/auth/token` phát token không xác thực | Route đã gỡ; `api_server::api_token` (IPC) đăng ký ở `lib.rs`; test router khẳng định endpoint không trả 200 |
| P2 | `redact()` phủ 1/4 lớp | `ASSIGN_EQ`/`ASSIGN_FLAG`/`ASSIGN_JSON`/`ASSIGN_YAML_*`/`PEM_BLOCK`/`URL_CREDENTIAL`/`HOME_PATH` — 40 test |
| P3 | `crash.rs` không ghi gì | `write_report_in`/`rotate_in`/`latest_report`; repro EPIPE: trước exit 134 không file, sau có file |
| P4 | `Command::output()` buffer cả image vào RAM | `streaming_cmd.rs`; đo 256 MB qua `ToFile`, RSS tăng < 32 MB |
| P5 | SSE không có sổ sách subscriber | `SubscriptionGuard` + `subscriber_count`; test qua router thật 0→1→0 |
| P6 | Debounce nuốt burst, không đếm được | `docker_events.rs`; test Docker thật: `on-failure:5` ra 6 `die` riêng biệt, 3/3 lần chạy |
| P7 | Capability gate khoá cả khách trả tiền | `ProGate` gate theo `isPaid()`; nhánh capability giữ nguyên cho sidecar tương lai |

**78 test thuộc các module của plan: pass hết.** Clippy sạch trên mọi file plan
này tạo ra; warning duy nhất còn lại ở `path_util.rs` nằm trong `fix_path_env`,
code có sẵn mà plan không đụng.

### Vì sao tiêu chí clippy/pnpm không đánh giá được như đã viết

Nó được viết ở mức **toàn repo**, mà repo chưa bao giờ ở trạng thái đó: `pnpm check`
có 143 lỗi có sẵn từ trước plan này. Tại thời điểm đóng, `cargo clippy` còn 1 error
và ~31 warning, và 1 test fail — **toàn bộ nằm ở `commands/diagnostics.rs`**, thuộc
Free P2 đang `pending` và được viết dở ở phiên song song.

Bài học cho plan sau: tiêu chí nghiệm thu phải nêu phạm vi. "Clippy sạch trên các
file phase này chạm tới" kiểm được; "clippy sạch" thì bị mọi công việc song song
làm cho vô nghĩa.

## Nguồn

- `plans/reports/from-red-team-to-owner-260811-2300-free-pro-tier-plans-adjudication-report.md`
- 4 report chi tiết cùng thư mục, tiền tố `from-code-reviewer-to-planner-red-team-*`
