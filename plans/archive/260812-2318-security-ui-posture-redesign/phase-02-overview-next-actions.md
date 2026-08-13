# Phase 2 — Overview: posture summary + next actions

## Mục tiêu

Tab Overview trở thành màn hình trả lời "máy tôi có an toàn không, và tôi nên
làm gì tiếp".

## Phụ thuộc

Phase 1 (shell + `summarizePosture`).

## Files

**Tạo**
- `src/components/security/SeverityBar.svelte` — thanh phân bố severity, có
  nhãn số, không dùng màu làm kênh thông tin duy nhất (kèm nhãn chữ).
- `src/components/security/NextActions.svelte` — danh sách việc cần làm.
- `src/components/security/security-actions.ts` — hàm thuần sinh danh sách việc.

**Sửa**
- `src/pages/Security.svelte` — nhánh `tab === "overview"` render
  `SeverityBar` + `NextActions` + bảng xếp hạng image theo điểm; click 1 dòng →
  set `selected` và chuyển `tab = "images"`.

## Các bước

1. `security-actions.ts` — `nextActions(audits, suggestions)` trả mảng
   `{ imageRef, kind: "rule" | "alternative", title, detail, estimatedGain }`:
   - `kind: "rule"` từ `evaluation.results` có `passed === false`; `estimatedGain`
     = `weight` của rule trong component tương ứng.
   - `kind: "alternative"` từ `suggestions[ref].alternatives`; **không** ước tính
     điểm (không biết trước) → `estimatedGain: null`, hiển thị "chưa đo".
   - Sắp giảm dần theo `estimatedGain`, `null` xuống cuối. Giới hạn 8 mục, kèm
     đếm "còn N mục khác" — không cắt im lặng.
2. `NextActions.svelte`: mỗi mục là hàng bấm được → nhảy tới image + tab Images.
   Rỗng → "Không còn việc nào ở mức strictness này" (nêu rõ level, vì L3 sẽ
   sinh thêm việc).
3. Bảng xếp hạng: ref, điểm, mini bar, trạng thái (đã quét / đang quét / chưa
   quét / lỗi). Image chưa quét không có điểm — hiện dấu "—" và nút Scan.
4. Dòng chân Overview: `ScoreInputs` (pack, engine, scanner, db snapshot date),
   theo nguyên tắc "mọi số phải giải thích được". Nếu `mixed` → cảnh báo.

## Validation

- `pnpm lint && pnpm typecheck && pnpm check`.
- Test `security-actions.test.ts`: rule failed sắp trước alternative; `null`
  gain xuống cuối; cắt ở 8 kèm số dư đúng.
- Test `NextActions.test.ts`: click phát `onSelect` đúng `imageRef`.
- Thủ công: 0 image → empty state; 1 image scan lỗi → Overview vẫn dùng được.

## Rủi ro

- `RuleResult` có thể không mang `weight` — kiểm tra `src/lib/api/security.ts`
  trước khi code; nếu thiếu, lấy weight từ `pack.rules` theo `ruleId` và bỏ ước
  tính khi pack chưa tải xong (`estimatedGain: null`), không đoán số.
