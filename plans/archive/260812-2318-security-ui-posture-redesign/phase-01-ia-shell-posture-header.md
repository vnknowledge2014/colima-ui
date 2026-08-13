# Phase 1 — Khung IA + tab shell + posture header

## Mục tiêu

Tách `src/pages/Security.svelte` thành shell điều hướng: header posture + 4 tab.
Nội dung cũ (master–detail) chuyển nguyên trạng vào tab Images, chưa restyle.

## Bối cảnh

- File hiện tại: `src/pages/Security.svelte` (463 dòng) — state `images`,
  `selected`, `level`, `audits`, `suggestions`, `scanning`, `scanErrors`,
  `rescoring`. Toàn bộ giữ nguyên ở phase này.
- Không có class `tab-btn` trong `src/styles/*.css`; `src/pages/Kubernetes.svelte:722`
  tự khai báo cục bộ. Không tạo component tab dùng chung ở phase này (YAGNI) —
  khai báo cục bộ trong Security, theo cùng token với Kubernetes.

## Files

**Tạo**
- `src/components/security/PostureHeader.svelte` — dải trạng thái trên cùng
  (điểm tổng hợp, đếm severity, độ phủ "N/M image đã quét", strictness selector).
- `src/components/security/security-posture.ts` — hàm thuần tổng hợp posture từ
  `Record<string, SecurityAudit>`; không import Svelte.

**Sửa**
- `src/pages/Security.svelte` — thêm state `tab`, render `PostureHeader` + tab
  bar; bọc nội dung cũ trong nhánh `tab === "images"`.

## Các bước

1. `security-posture.ts`:
   - `summarizePosture(audits, totalImages)` → `{ scanned, total, average,
     lowest: { ref, score } | null, severity: Record<Severity, number>,
     inputs: ScoreInputs | null }`.
   - `average` là trung bình đơn giản trên image đã quét, làm tròn xuống; trả
     `null` khi `scanned === 0` (không trả 0 — 0 điểm ≠ chưa đo).
   - `inputs` lấy từ audit mới nhất; nếu các audit khác `packVersion`/
     `scannerVersion` thì trả cờ `mixed: true` để header nói "kết quả trộn
     nhiều phiên bản, quét lại để so sánh".
2. `PostureHeader.svelte`: nhận `posture`, `level`, `busy`, `onChangeLevel`,
   `onReload`. Trạng thái rỗng: tiêu đề "Chưa quét image nào" + câu giải thích
   scan là hành động thủ công. Chuyển `select` strictness và nút Reload từ
   `content-header-actions` vào đây.
3. `Security.svelte`: `let tab = $state<"overview"|"images"|"runtime"|"history">("overview")`.
   Tab bar 4 nút; `runtime`/`history` render card "chưa khả dụng" trỏ tới
   phase 5/7 của plan `260812-1013-security-posture` (kèm nút mở KB article
   qua `openHelpArticle` nếu có bài phù hợp, không thì bỏ nút).
   Tab `overview` tạm render lại `PostureHeader` + hint → phase 2 thay thế.
4. i18n: khóa mới dưới `security.*` (`security.tab_overview`, `tab_images`,
   `tab_runtime`, `tab_history`, `posture_empty`, `posture_coverage`,
   `posture_mixed`, `soon_pro`). Dùng `t(key, { default })` như code hiện có.

## Validation

- `pnpm lint && pnpm typecheck && pnpm check`.
- Unit test mới `src/components/security/security-posture.test.ts`: rỗng →
  `average === null`; 2 audit → trung bình đúng + `lowest` đúng; khác
  `packVersion` → `mixed === true`.
- Thủ công: chưa có Trivy → banner scanner-missing vẫn hiện trên mọi tab.

## Rủi ro

- Di chuyển strictness selector khỏi content-header có thể vỡ test nào tìm theo
  selector cũ → grep `security.level` trong `src/**/*.test.ts` trước khi sửa.
- Rollback: phase này thuần thêm lớp bọc; revert 1 commit là về nguyên trạng.
