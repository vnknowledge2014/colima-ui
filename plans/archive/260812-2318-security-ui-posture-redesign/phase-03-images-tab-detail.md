# Phase 3 — Images tab + detail, đồng bộ ngôn ngữ hình ảnh

## Mục tiêu

Tab Images kế thừa master–detail cũ nhưng theo ngôn ngữ hình ảnh của
Instances/Settings (section card, spacing, typography, badge severity), và bỏ
trùng lặp với Overview.

## Phụ thuộc

Phase 2.

## Files

**Sửa**
- `src/pages/Security.svelte` — nhánh `tab === "images"`: bỏ `.block` tự chế,
  dùng cùng cấu trúc card như `src/components/settings/SettingsSection.svelte`
  nếu component đó nhận được nội dung tuỳ ý; nếu không, khai báo `.section-card`
  cục bộ theo cùng token (`--border-primary`, `--bg-secondary`, `--text-*`).
- `src/components/security/ScoreBreakdownCard.svelte` — điểm thành phần dạng
  bar + nhãn, giữ nguyên props và test hiện có.
- `src/components/security/FindingsTable.svelte` — badge severity thống nhất
  với `SeverityBar` (phase 2), thêm lọc theo severity nếu chưa có.
- `src/components/security/RuleChecklist.svelte`, `AlternativesPanel.svelte` —
  chỉ chỉnh trình bày, không đổi props.

## Các bước

1. Rà `SettingsSection.svelte` xem tái dùng được không. Chỉ tạo component mới
   nếu nó không nhận slot nội dung tự do.
2. Chuẩn hoá màu severity vào token dùng chung (`security-severity.ts` hoặc
   biến CSS trong `src/styles/features.css`) — hiện `FindingsTable` và
   `SeverityBar` sẽ là 2 nguồn màu nếu không gom.
3. Danh sách image bên trái: thêm mini bar điểm giống Overview, giữ hành vi
   `selected`/`aria-current` hiện có.
4. Head detail: gom Scan / Scan again / Ignore cache / Cancel thành một cụm
   nhất quán; giữ nguyên logic `scanning[ref]` theo image (không quay lại cờ
   đơn — xem docblock hiện tại).
5. Bỏ CSS chết trong `<style>` của `Security.svelte` sau khi chuyển.

## Validation

- `pnpm lint && pnpm typecheck && pnpm check`.
- Test hiện có `ScoreBreakdownCard.test.ts`, `RuleChecklist.test.ts` pass không
  sửa assertion; nếu buộc phải sửa → dừng, báo người dùng (đổi contract).
- Thủ công: quét 2 image song song → mỗi image giữ trạng thái riêng, Cancel chỉ
  dừng đúng image của nút.
- Kiểm tra light/dark theme và màn hẹp (<900px) — layout đổ 1 cột.

## Rủi ro

- Đây là phase dễ vô tình đổi hành vi nhất (gom nút, đổi CSS chung). Không sửa
  logic scan/cancel trong phase này.
- Rollback: 3 phase là 3 commit riêng; revert phase 3 vẫn còn Overview dùng được.
