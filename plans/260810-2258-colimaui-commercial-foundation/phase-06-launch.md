---
phase: 6
title: "Launch"
status: pending
priority: P1
effort: "8-11 ngày"
dependencies: [2, 4, 5]
---

# Phase 6: Launch

## Overview

Mở bán với **3-4 tính năng Pro**, chọn từ dữ liệu — không phải cả 9 mục danh mục,
cũng không phải 2 mục cố định.

Bản v1 ship 5 tính năng đoán trước. Red-team cắt còn 2 để học nhanh hơn. Chủ dự án
ghi đè một phần: 2 có nguy cơ mỏng dưới ngưỡng ai chịu trả tiền, khiến kết quả đo
không diễn giải được. Chi tiết lập luận ở mục "Ghi đè quyết định" trong `plan.md`.

Điều quan trọng được giữ nguyên từ red-team: **không đoán**. Con số là 3-4, và
danh tính của chúng do dữ liệu waitlist Phase 1 quyết định.

## Bộ tính năng Pro v1 — cách chọn

| Vị trí | Tính năng | Cách xác định |
|---|---|---|
| Flagship | Compose auto-fix / chẩn đoán | **Cố định.** Đã xây Bước A ở Phase 5; phải qua cổng go/no-go trước |
| 2 | *chưa xác định* | Mục xếp hạng cao nhất trong waitlist Phase 1 |
| 3 | *chưa xác định* | Mục xếp hạng thứ hai |
| 4 (tuỳ chọn) | *chưa xác định* | Mục thứ ba, chỉ làm nếu ước lượng ≤4 ngày |

Ứng viên là 9 mục trong Danh mục Pro ở `plan.md`.

### Quy tắc chọn — áp dụng trước khi viết dòng code nào

1. **Cổng nguyên tắc 1 trước cổng nhu cầu.** 4 mục đánh dấu ⚠️ trong danh mục
   (vuln scan, registry manager, backup/restore, buildx UI) nằm sát ranh giới
   paywall-thứ-đã-miễn-phí. Mục ⚠️ xếp hạng cao vẫn phải qua rà soát này; trượt thì
   lấy mục kế tiếp. Nhu cầu cao không biến một wrapper CLI thành hàng bán được.
2. **Loại self-healing khỏi vòng đầu** nếu nó thắng phiếu. Nó tự động hành động lên
   máy người dùng — sai một lần là mất niềm tin, và vòng launch không phải lúc để
   học cách làm cái đó an toàn.
3. **Nếu waitlist không đạt ngưỡng Phase 1**, phase này không chạy. Cổng quyết định
   của Phase 1 đứng trên mọi thứ ở đây.

**Hoãn sang Phase 7:** mọi mục danh mục không được chọn.
**Loại hẳn:** đồng bộ cấu hình (cần tài khoản), Review PR (lệch trọng tâm), command
palette và container domains (thuộc Free).

## Requirements

**Functional**
- Mỗi tính năng được chọn phải có một phép đo giá trị khách quan, không phải lời hứa. Ví dụ với Dockerfile optimizer: **đo dung lượng trước-sau bằng build thật**, không ước lượng.
- Mọi tính năng được chọn chạy qua sidecar, khai báo capability lúc handshake.
- Mọi `ProGate` dẫn tới `UpgradeDialog` nói rõ được gì; **không nag chặn thao tác**.
- **UI Pro theo hệ thiết kế** ở `plans/reports/pro-ui-design-system-260811-1615-badges-blur-preview-gating-report.md`:
  badge PRO tím, preview blur (chỉ tease nội dung Pro-độc-quyền, không che kết quả Free),
  và **bốn trạng thái `ProStatus`** — đặc biệt `NeedsUpdate` ≠ locked (không để khách
  trả tiền thấy màn chưa-mua). **`ProBadge` + `ProLockedPreview` + `ProGate` 4-trạng-thái
  đã dựng (2026-08-11)**; phase này chỉ cần lắp vào tính năng thật, không dựng lại.
- Đo được phễu: cài đặt → activation → chạm ProGate → mở checkout → mua.

**Non-functional**
- Hết hạn Pro → tính năng Pro ẩn sạch, Free nguyên vẹn, không mất dữ liệu.
- Tính năng nào chạy tác vụ dài (vd Dockerfile optimizer build thật để đo) phải xử lý được trường hợp lỗi, chạy lâu, hết dung lượng.

## Related Code Files

Danh sách dưới đây giả định Dockerfile optimizer được chọn. Nếu waitlist chọn khác,
thay bằng module + panel tương ứng — cấu trúc giữ nguyên.

**Repo private:**
- Create: `src/dockerfile_opt/{analyze,generate,measure}.rs` *(nếu được chọn)*
- Create: một module cho mỗi tính năng được chọn khác

**Core MIT:**
- Create: `src/components/pro/DockerfileOptimizerPanel.svelte` *(nếu được chọn)*
- Create: một panel cho mỗi tính năng được chọn khác
- Modify: `src/pages/Images.svelte` — nút "Tối ưu Dockerfile" *(nếu được chọn)*
- Modify: `src/components/ProGate.svelte`, `src/components/UpgradeDialog.svelte`
- Modify: `src/locales/*` (4 locale)

**Website:**
- Modify: trang giá (từ Phase 1) — chuyển nhãn sang `đã có` cho các mục đã ship, giữ `dự kiến` cho phần còn lại của danh mục
- Create: changelog, tài liệu Pro

## Implementation Steps

1. **Cổng kiểm tra:** xác nhận Plan 1 (P0) hoàn tất, Phase 5 đã qua cổng go/no-go, Phase 2/4 xong.
2. **Chốt danh sách 3-4 tính năng** từ bảng xếp hạng waitlist, chạy qua 3 quy tắc
   chọn ở trên, ghi kết quả + lý do loại vào `plans/reports/`. Không code trước bước này.
3. Với **mỗi** tính năng được chọn: xây ở repo private, đăng ký capability qua
   protocol Phase 3, tạo panel tương ứng ở core, dùng lại `DiffView.svelte` khi hợp.
3b. *(Nếu Dockerfile optimizer được chọn)* phân tích layer, phát hiện anti-pattern phổ
   biến (COPY toàn bộ context, cài rồi không dọn cache, không multi-stage, base image
   nặng); sinh bản multi-stage; **đo bằng build thật** — build cả 2 bản, so dung
   lượng. Xử lý build lỗi/lâu/hết chỗ tử tế. Panel:
   `src/components/pro/DockerfileOptimizerPanel.svelte` + nút ở `Images.svelte`.
4. Rà toàn bộ ProGate: không chỗ nào chặn thao tác vốn miễn phí.
5. Đo phễu bằng telemetry Phase 4.
6. Cập nhật trang giá theo tính năng thực tế đã có.
7. **Beta riêng ~20 người** (ưu tiên người trong waitlist Phase 1), thu ≥10 phản hồi có ý nghĩa.
8. Xử lý phản hồi, rồi mở bán công khai.

## Tests / Validation

- Unit: phần logic thuần của **mỗi** tính năng được chọn.
- *(Nếu Dockerfile optimizer được chọn)* Unit: mỗi anti-pattern (dương/âm). Integration: sinh Dockerfile **build được** và ảnh **nhỏ hơn thật** trên ≥5 ảnh mẫu; build lỗi → báo rõ, không treo UI.
- Thủ công: hành trình đầy đủ Free → chạm ProGate → mua → dùng Pro.
- Thủ công: hết hạn Pro → tính năng Pro ẩn sạch, Free nguyên vẹn.
- Thủ công: cập nhật app khi đang là Pro → vẫn là Pro (test skew Phase 3).
- Beta 20 người: ≥10 phản hồi.

## Success Criteria

- [ ] 3-4 tính năng Pro chạy qua sidecar, có ProGate.
- [ ] Danh sách chọn + lý do loại đã ghi vào `plans/reports/` **trước khi** code.
- [ ] Mọi mục ⚠️ được chọn đã qua cổng nguyên tắc 1.
- [ ] Mỗi tính năng được chọn có phép đo giá trị khách quan đã chạy thật *(vd optimizer giảm dung lượng trên ≥5 ảnh mẫu, đo bằng build)*.
- [ ] Không ProGate nào chặn thao tác miễn phí.
- [ ] Phễu đo được từ cài đặt tới mua.
- [ ] Beta 20 người xong, đã xử lý phản hồi.
- [ ] Mở bán công khai; có ≥1 giao dịch thật thành công.

## Cổng launch (tất cả phải đạt)

- [ ] Plan hotfix bảo mật hoàn tất.
- [ ] Plan P0 (`260810-2258-colimaui-p0-polish-parity`) hoàn tất.
- [ ] Phase 5 qua cổng go/no-go bằng số liệu.
- [ ] Core MIT chạy đầy đủ khi không có sidecar.
- [ ] Version skew không hạ cấp khách trả tiền (có test).
- [ ] E2E mua hàng production chạy thật, có đường lấy lại key.
- [ ] Auto-update đã kiểm chứng trên bản ký + notarize.
- [ ] `docs/pro-boundary.md` + `docs/telemetry.md` công khai.

## Risk Assessment

| Rủi ro | Mức | Giảm thiểu |
|---|---|---|
| Giỏ hàng không đủ giá trị cho $6 | Cao | 3-4 tính năng do dữ liệu waitlist chọn, không do đoán; beta 20 người trước; sẵn sàng hạ giá hoặc bổ sung |
| Phạm vi phình từ 3-4 lên nhiều hơn khi đang làm | Cao | Con số 3-4 là trần cứng. Mục thứ 4 chỉ làm nếu ước lượng ≤4 ngày. Muốn thêm thì để Phase 7 |
| Ra mắt khi bản Free chưa đủ tốt | Cao | Cổng launch bắt buộc P0 xong — không thoả hiệp |
| Dockerfile optimizer sinh file không build được | TB | Build thật để verify là một phần của tính năng, không phải bước kiểm thử tuỳ chọn |
| Phản ứng tiêu cực khi công bố bản trả phí | TB | Đã công bố giá từ Phase 1 → không bất ngờ; Free được dùng thương mại |
| Cám dỗ thêm tính năng trước khi bán | TB | Danh sách hoãn ghi rõ trong phase này; thêm gì cũng phải sau giao dịch đầu tiên |
