---
phase: 1
title: "Pricing & Paid Waitlist"
status: in_progress
priority: P1
effort: "3-4 ngày"
dependencies: []
---

# Phase 1: Pricing & Paid Waitlist

## Overview

Phase rẻ nhất và có giá trị thông tin cao nhất trong toàn bộ plan: **biết có ai chịu trả tiền hay không, trước khi bỏ 4 tuần xây hạ tầng cấp phép.**

Bản plan v1 đặt trang giá ở phase cuối, sau 7-9 tuần xây dựng. Đảo lại: trang giá ở tuần 1, không cần một dòng code cấp phép nào.

## Requirements

**Functional**
- Trang giá công khai: Free và Pro ($6/tháng, $60/năm), nói rõ Pro sẽ có gì và **khi nào**.
- Trang so sánh ColimaUI vs OrbStack vs Docker Desktop.
- Waitlist trả trước: người quan tâm đặt cọc/mua sớm với giá ưu đãi, hoặc tối thiểu là để email + trả lời 2 câu hỏi.
- Trong app: mục "Pro sắp có" ở Settings dẫn tới trang giá (không nag, không chặn).
- Thu thập lý do: người ta muốn Pro vì tính năng nào?

**Non-functional**
- Không hứa ngày ra mắt cụ thể nếu chưa chắc.
- Nếu thu tiền đặt trước: phải hoàn được 100%, nói rõ điều đó.
- Không dark pattern. Free vẫn là lựa chọn hoàn toàn hợp lệ và được nói rõ.

## Thông điệp bán hàng

Trục chính, rút từ báo cáo benchmark:

> **Mọi thứ OrbStack Pro làm — miễn phí, kể cả dùng cho công việc.**
> Trả tiền chỉ khi cần AI tự động sửa lỗi.

Ba điểm khác biệt cần nêu rõ:
1. Free **được dùng thương mại** (OrbStack Free thì không).
2. Có **Linux** (OrbStack chỉ macOS).
3. Có **AI diagnostic + Knowledge Bank** (OrbStack không có).

## Related Code Files

- Create: website — trang giá, trang so sánh, terms, refund policy
- Create: cấu hình waitlist/preorder trên MoR (chế độ test rồi production)
- Modify: `src/pages/Settings.svelte` — mục "Pro sắp có" + link
- Modify: `src/locales/*` — chuỗi liên quan
- Create: `docs/pricing-rationale.md` — ghi lại vì sao chọn mức giá này (cho chính mình về sau)

## Implementation Steps

1. **Chốt ngưỡng thành công trước khi chạy** (Unresolved Question #4 ở plan.md). Ví dụ: 30 email đăng ký hoặc 10 đơn trả trước trong 2 tuần. Không có ngưỡng thì kết quả không diễn giải được.
2. **Nộp hồ sơ MoR ngay** — song song, vì thời gian duyệt nằm ngoài tầm kiểm soát và Phase 2 phụ thuộc.
3. Viết trang so sánh dựa trên báo cáo benchmark đã có (số liệu đã sẵn, chỉ cần trình bày).
4. Viết trang giá; liệt kê **đủ 9 mục** trong Danh mục Pro (`plan.md`), mỗi mục một
   nhãn trạng thái trung thực: `đã có` / `đang làm` / `dự kiến`. Hiện tại đúng **0
   mục** ở trạng thái `đã có` — compose auto-fix mới xong Bước A và chưa qua cổng
   go/no-go. Ghi sai nhãn ở bước này là hứa hão, và red-team đã bắt đúng lỗi đó ở v1.
5. Dựng waitlist: form email + 2 câu hỏi ("bạn dùng Colima cho việc gì?", "tính năng nào khiến bạn trả tiền?").
   **Câu 2 phải là danh sách chọn nhiều** gồm đủ 9 mục trong Danh mục Pro ở `plan.md`,
   cộng một ô tự do. Đây là dữ liệu duy nhất chọn ra 3-4 tính năng cổng launch của
   Phase 6 — câu hỏi mở không cho ra thứ đếm được.
6. Nếu chọn thu tiền đặt trước: cấu hình trên MoR, nêu rõ chính sách hoàn tiền.
7. Thêm mục Settings dẫn ra trang giá — một dòng, không popup, không nag.
8. Công bố: README, GitHub Discussions, các cộng đồng Colima/Docker phù hợp.
9. Chạy 2 tuần, đo, viết lại kết quả vào `plans/reports/`.

## Tests / Validation

- Trang hiển thị đúng trên mobile và desktop.
- Form waitlist gửi thành công, có xác nhận email.
- Nếu có preorder: luồng thanh toán test chạy được, hoàn tiền test chạy được.
- Link từ app ra trang giá hoạt động ở cả desktop và browser mode.

## Success Criteria

- [ ] Ngưỡng thành công được chốt bằng số **trước khi** chạy.
- [ ] Trang giá + trang so sánh live.
- [ ] Waitlist thu được email và lý do.
- [ ] **Có bảng xếp hạng nhu cầu theo từng tính năng** — đầu vào bắt buộc để Phase 6 chọn 3-4 mục cổng launch.
- [ ] Hồ sơ MoR đã nộp.
- [ ] Chạy đủ 2 tuần, có báo cáo kết quả.
- [ ] **Cổng quyết định:** đạt ngưỡng → đi tiếp Phase 2. Không đạt → dừng plan này, quay lại làm bản Free tốt hơn và đo lại sau.

## Tiến độ — 2026-08-11

Đã làm (phần nằm trong repo):

- [x] Bước 7 — mục "ColimaUI Pro / COMING SOON" trong Settings, một dòng mô tả +
      nút "Learn more", không popup, không chặn. `src/pages/Settings.svelte`.
- [x] Chuỗi i18n cho 4 ngôn ngữ: `settings.pro.*` trong `src/locales/{en,vi,zh,ja}.json`.
- [x] `docs/pricing-rationale.md` — ghi lại vì sao $6/$60, vì sao Free được dùng
      thương mại, vì sao BYOK. Ghi rõ giá là **đề xuất, chưa thu tiền**.
- [x] `src/lib/external-links.ts` — hằng số `PRICING_URL` ở một chỗ duy nhất +
      `openExternal()` chạy được ở cả desktop (plugin-opener) lẫn browser mode.

**`PRICING_URL` đang là placeholder** trỏ về GitHub Discussions vì chưa có
domain. Đổi domain = sửa một dòng trong `src/lib/external-links.ts`.

Chưa làm (nằm ngoài repo, cần người quyết định):

- [ ] Bước 1 — chốt ngưỡng thành công bằng số. **Chặn** việc chạy phase.
- [ ] Bước 2 — nộp hồ sơ MoR (còn phụ thuộc Unresolved Question #1: chọn MoR nào).
- [ ] Bước 3-6 — website: trang giá, trang so sánh, terms, refund policy, form waitlist.
- [ ] Bước 8-9 — công bố, chạy 2 tuần, viết báo cáo kết quả.

## Risk Assessment

| Rủi ro | Giảm thiểu |
|---|---|
| Lượng truy cập quá thấp để kết luận | Chủ động đăng ở nơi có người dùng Colima thật; nếu vẫn ít thì bản thân điều đó là dữ liệu về quy mô thị trường |
| Người ta nói sẽ trả nhưng không trả | Ưu tiên preorder có tiền thật hơn là email; tiền là tín hiệu duy nhất đáng tin |
| Công bố giá sớm gây phản ứng | Nói rõ Free không mất gì và được dùng thương mại; đó chính là thông điệp mạnh nhất |
| Hứa tính năng chưa có rồi không làm được | Đánh dấu rõ cái nào chưa có; không hứa ngày cụ thể |
