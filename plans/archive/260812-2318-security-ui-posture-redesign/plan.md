---
name: Security UI — đổi tư tưởng sang posture-first
date: 2026-08-12
status: done — phase 1, 2, 3 xong 2026-08-13; chưa commit
relatedPlans:
  - 260812-1013-security-posture
---

# Security UI — posture-first redesign

Đổi tư tưởng thiết kế trang Security từ **image-first, scan-on-demand** sang
**posture-first**: mở trang thấy ngay tình trạng bảo mật tổng thể của máy, rồi
mới drill-down xuống từng image.

## Vì sao đổi

| Vấn đề hiện tại | Hệ quả |
|---|---|
| Không có bức tranh tổng thể — chỉ có list image + điểm rời rạc | Người dùng không trả lời được "máy tôi có an toàn không?" |
| Mọi thứ phẳng trên 1 trang (score + checklist + findings + alternatives) | Phase 4–7 (AI triage, history, sandbox, Falco) đổ thêm vào là vỡ layout |
| Không có "việc cần làm tiếp" | Điểm thấp nhưng người dùng không biết sửa gì trước |
| Ngôn ngữ hình ảnh lệch Instances/Settings (đã dùng section card) | Trang trông như từ bản build khác |

## Tư tưởng mới (4 nguyên tắc)

1. **Trạng thái trước, thao tác sau.** Trên cùng là posture: điểm tổng hợp,
   phân bố severity, độ tươi dữ liệu. Nút Scan là phương tiện, không phải chủ đề.
2. **Luôn nói vì sao.** Mỗi con số đi kèm nguồn (pack version, scanner,
   db snapshot). Không có số nào không giải thích được.
3. **Việc cần làm là first-class.** "Next actions" nằm ngay dưới posture, sinh
   từ rule failed + alternatives, sắp theo điểm thu được.
4. **Một trang, nhiều lớp.** Khung tab `Overview / Images / Runtime / History`
   dựng sẵn — tab chưa có tính năng hiển thị trạng thái Pro/coming, không ẩn
   im lặng.

## Ràng buộc

- **Không đổi backend, không đổi contract** `src/lib/api/security.ts`. Posture
  tổng hợp hoàn toàn client-side từ map `audits` đang có trong bộ nhớ.
- **Không tự động scan.** Nguyên tắc "scan là việc người dùng yêu cầu" trong
  docblock `Security.svelte` giữ nguyên. Image chưa scan → posture nói rõ đang
  tổng hợp trên N/M image, không đoán.
- Không dùng số liệu giả cho tab chưa có tính năng.

## Phases

| # | Tên | File |
|---|---|---|
| 1 | Khung IA + tab shell + posture header | [phase-01](phase-01-ia-shell-posture-header.md) — **DONE 2026-08-13** |
| 2 | Overview: posture summary + next actions | [phase-02](phase-02-overview-next-actions.md) — **DONE 2026-08-13** |
| 3 | Images tab + detail drawer, đồng bộ ngôn ngữ hình ảnh | [phase-03](phase-03-images-tab-detail.md) — **DONE 2026-08-13** |

Thứ tự bắt buộc 1 → 2 → 3.

## Acceptance

- Mở trang khi chưa scan gì: thấy posture "chưa có dữ liệu" + CTA scan, không
  thấy số 0 giả.
- Scan ≥1 image: posture header hiện điểm trung bình đơn giản trên các image đã
  quét (kèm image thấp điểm nhất), đếm severity, và ghi rõ "N/M image đã quét".
- Next actions ≥1 mục khi có rule failed hoặc alternative; mỗi mục nêu điểm
  ước tính thu được.
- Tab Runtime/History hiện trạng thái "chưa khả dụng" gắn với phase 5/7 của plan
  `260812-1013-security-posture`, không phải placeholder trống.
- `pnpm lint && pnpm typecheck && pnpm check` xanh. Test hiện có của
  `ScoreBreakdownCard` / `RuleChecklist` vẫn pass.

## Quyết định đã chốt khi thực thi

1. Điểm posture = trung bình đơn giản trên image đã quét, kèm image thấp nhất.
   Trọng số theo findings bị loại: khó giải thích, mà nguyên tắc 2 đòi mọi số
   phải giải thích được.
2. Không tách `SeverityBar` riêng — số đếm severity đã nằm trong posture header,
   thêm thanh tỉ lệ ở Overview là hai cách trình bày cùng một dữ liệu (DRY).
3. Tab History chỉ dựng vỏ; store lịch sử thuộc phase 5 của plan
   `260812-1013-security-posture`.

## Unresolved

1. Cancel một lần quét hiện hiển thị như lỗi ("This image could not be scanned ·
   Scan cancelled") — hành vi có sẵn từ trước, nay lộ rõ hơn vì Overview cũng
   đánh dấu image đó là `scan failed`. Sửa hay không là quyết định riêng.
