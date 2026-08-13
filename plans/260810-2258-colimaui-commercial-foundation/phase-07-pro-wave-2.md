---
phase: 7
title: "Pro Wave 2"
status: pending
priority: P2
effort: "10-15 ngày"
dependencies: [6]
---

# Phase 7: Pro Wave 2

## Overview

Xây tiếp danh mục Pro **sau khi đã có doanh thu thật**, theo thứ tự do khách hàng
trả tiền quyết định — không phải theo thứ tự waitlist nữa.

Phân biệt này quan trọng. Waitlist là người nói sẽ trả. Phase 6 đã bán thật, nên từ
đây trở đi tín hiệu tốt hơn đã tồn tại: người **đã** trả tiền muốn gì tiếp theo, và
ProGate nào bị chạm nhiều nhất mà không có gì phía sau.

Phase này cố ý không chốt trước tính năng nào. Chốt trước là quay lại đúng sai lầm
của plan v1.

## Điều kiện vào phase

Không chạy phase này nếu chưa thoả **cả ba**:

1. Phase 6 đã mở bán công khai và có giao dịch thật.
2. Có ít nhất một chu kỳ gia hạn — biết được tỉ lệ churn, không chỉ tỉ lệ mua.
3. Telemetry Phase 4 đã chạy đủ lâu để bảng ProGate có ý nghĩa thống kê.

Chưa đủ ba điều kiện thì việc đúng là cải thiện những tính năng đã bán, không phải
thêm tính năng mới.

## Requirements

**Functional**
- Chọn 2-3 mục còn lại trong Danh mục Pro (`plan.md`), theo thứ tự ưu tiên bên dưới.
- Mỗi mục đi qua đúng cổng nguyên tắc 1 như Phase 6.
- Mỗi mục đăng ký capability qua protocol Phase 3; core không hardcode gì.
- Cập nhật trang giá: chuyển nhãn từ `dự kiến` sang `đã có` khi ship xong.

**Non-functional**
- Hết hạn Pro → mọi tính năng Wave 2 ẩn sạch, Free nguyên vẹn, không mất dữ liệu.
- Không mục nào của Wave 2 được làm suy giảm trải nghiệm Free so với trước đó.

## Thứ tự ưu tiên

Xếp theo tín hiệu, tốt nhất trước:

1. **Tần suất chạm ProGate** — tính năng người trả tiền cố dùng mà chưa có.
2. **Lý do churn** — cái người ta nêu khi huỷ.
3. **Yêu cầu từ khách đã trả tiền** — nặng hơn yêu cầu từ người dùng Free.
4. **Xếp hạng waitlist Phase 1** — dùng làm tie-break, không dùng làm nguồn chính.

## Ứng viên

Toàn bộ mục chưa xây trong Danh mục Pro. Hai ghi chú riêng:

- **Self-healing** — nếu vào Wave 2, phải mặc định ở chế độ chỉ-đề-xuất. Tự động
  hành động lên máy người dùng chỉ được bật sau khi có dữ liệu chứng minh tỉ lệ
  đề xuất đúng đủ cao. Nêu rõ tỉ lệ đó là bao nhiêu trước khi xây.
- **Import từ Docker Desktop / OrbStack** — mục duy nhất trong danh mục ảnh hưởng
  tăng trưởng chứ không chỉ doanh thu. Nếu tăng trưởng đang là nút thắt lớn hơn
  chuyển đổi, ưu tiên mục này bất kể xếp hạng.

## Tests / Validation

- Mỗi tính năng: test đơn vị cho phần logic thuần, như Phase 5 đã làm.
- Hết hạn license → chạy lại toàn bộ luồng Free, xác nhận không lỗi, không mất dữ liệu.
- Rà ProGate: không chỗ nào chặn thao tác vốn miễn phí.
- Không hồi quy hiệu năng khởi động — mỗi capability thêm vào không được làm chậm
  handshake sidecar.

## Success Criteria

- [ ] Ba điều kiện vào phase đã thoả và ghi lại bằng số.
- [ ] Thứ tự chọn dựa trên dữ liệu ProGate/churn, có báo cáo trong `plans/reports/`.
- [ ] 2-3 mục ship xong, mỗi mục qua cổng nguyên tắc 1.
- [ ] Trang giá khớp đúng thực tế — không mục nào ghi `đã có` mà chưa có.
- [ ] Free không suy giảm so với trước Wave 2.

## Risk Assessment

| Rủi ro | Mức | Giảm thiểu |
|---|---|---|
| Thêm tính năng thay vì sửa tính năng đã bán | Cao | Ba điều kiện vào phase; churn phải biết trước khi xây thêm |
| Self-healing hành động sai lên máy người dùng | Cao | Mặc định chỉ-đề-xuất; ngưỡng chính xác phải nêu trước khi xây |
| Mục ⚠️ lọt qua vì nhu cầu cao | TB | Cổng nguyên tắc 1 chạy trước cổng nhu cầu, như Phase 6 |
| Danh mục phình ra thay vì hội tụ | TB | Wave 2 tối đa 3 mục. Muốn thêm thì mở plan mới, không nối dài phase này |
