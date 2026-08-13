---
title: ColimaUI — Hotfix bảo mật
description: >-
  Sửa 3 lỗ hổng đang tồn tại trong code hiện tại, phát hiện qua red-team review
  ngày 2026-08-10
status: completed
priority: P1
branch: dev
tags:
  - security
  - hotfix
  - redaction
  - validation
blockedBy: []
blocks:
  - 260810-2258-colimaui-p0-polish-parity
created: '2026-08-10T16:26:43.165Z'
createdBy: 'ck:plan'
source: skill
---

# ColimaUI — Hotfix bảo mật

## Overview

Đây **không phải** lỗi của kế hoạch — là lỗ hổng đang tồn tại trong code trên nhánh `dev` hôm nay. Phát hiện khi red-team soi 2 plan roadmap; các reviewer đi verify plan thì tìm ra code thật đang có vấn đề.

Plan này chặn `260810-2258-colimaui-p0-polish-parity` Phase 1: kế hoạch đó định thêm nút "Copy chi tiết lỗi" và "Hỏi AI về lỗi này" lên đúng luồng đang rò rỉ API key — làm vậy sẽ biến một rò rỉ thụ động thành một nút bấm.

Nguồn: `plans/reports/from-code-reviewer-to-planner-red-team-security-adversary-plan-review-report.md`

## Mức độ phơi nhiễm thực tế

Cần nói thẳng để không thổi phồng: ColimaUI chạy cục bộ, HTTP server bind loopback. Kẻ tấn công phải đã ở trên máy, hoặc dụ được người dùng dán nội dung lỗi ra ngoài. Nhưng:

- Rò rỉ #1 (API key trong thông báo lỗi) kích hoạt bởi hành vi **hoàn toàn bình thường**: người dùng copy lỗi đi hỏi trên GitHub issue hoặc Discord. Không cần kẻ tấn công nào cả.
- Rò rỉ #2 (tên profile không validate) là đường dẫn tới argv injection và, sau khi P0 Phase 5 làm xong, tới ghi file tuỳ ý.
- #3 là gia cố phòng thủ theo chiều sâu.

## Phases

| Phase | Name | Status |
|-------|------|--------|
| 1 | [Secret Redaction](./phase-01-secret-redaction.md) | Completed |
| 2 | [Input Validation](./phase-02-input-validation.md) | Completed |
| 3 | [API Auth Hardening](./phase-03-api-auth-hardening.md) | Completed |

## Ước lượng

| Phase | Ước lượng |
|---|---|
| 1 | 2-3 ngày |
| 2 | 2 ngày |
| 3 | 2-3 ngày |
| **Tổng** | **1.5 tuần** |

## Acceptance Criteria

- [ ] Không có API key, bearer token, hay biến môi trường nhạy cảm nào lọt vào chuỗi lỗi hiển thị, log, hay clipboard.
- [ ] Tên profile/instance được validate trước khi đi vào argv hoặc đường dẫn file.
- [ ] So sánh bearer token là so khớp chính xác, thời gian hằng số; không route nào bypass được auth bằng hậu tố đường dẫn.
- [ ] Có test hồi quy cho cả 3 nhóm, chạy trong CI.

## Dependencies

- **Blocks:** `260810-2258-colimaui-p0-polish-parity` — cụ thể là Phase 1 của plan đó.

## Unresolved Questions

1. Có phát hành bản vá 0.1.11 ngay sau plan này, hay gộp vào bản kế tiếp? (nếu đã có người dùng thật thì nên phát hành ngay)
2. Có cần thông báo cho người dùng hiện tại rằng nên xoay API key AI của họ không? Tuỳ vào việc key đã từng lọt vào issue công khai chưa.
