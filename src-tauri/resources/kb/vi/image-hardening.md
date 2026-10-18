# Gia cố một container image

## Trang Security kiểm cái gì

Hai câu hỏi tách biệt. **Lỗ hổng đã biết** do Trivy trả về: các package bên trong
image đã có cảnh báo được công bố. **Cấu hình** là các rule ColimaUI áp lên chính
image — nó chạy bằng quyền gì, đến từ đâu, cũ tới mức nào. Một image có thể vá
đầy đủ mà vẫn cấu hình tệ; đó là lý do điểm có bốn phần chứ không phải một.

## Những thay đổi đáng làm trước

**Đừng chạy bằng root.** Không có `USER` thì tiến trình khởi động bằng uid 0.
Tạo một tài khoản trong image và chuyển sang nó ở cuối Dockerfile:

```dockerfile
RUN adduser --system --no-create-home app
USER app
```

Nếu dịch vụ cần cổng 80, hãy lắng nghe ở 8080 bên trong container rồi ánh xạ ra
ngoài bằng `-p 80:8080`. Việc phải bind một cổng thấp thường chính là lý do image
vẫn đang chạy bằng root.

**Đưa thông tin bí mật ra khỏi image.** Mọi thứ trong `ENV` đi theo từng bản sao
của image và hiện ra trong `docker inspect`. Hãy truyền lúc chạy:

```bash
docker run --env-file ./secrets.env myapp:1.2.3
```

Chỉ build lại mà bỏ giá trị đi là chưa đủ — layer từng chứa nó vẫn nằm ở mọi
registry đã được push lên. Phải xoay vòng lại thông tin đó.

**Ghim cái bạn triển khai.** Tag là một vị trí, không phải một image: `latest`,
`stable`, và cả `1.27`, đều được xuất bản lại khi có bản vá. Hãy đặt tag bằng số
phiên bản đầy đủ, và triển khai theo digest ở những chỗ quan trọng:

```bash
docker pull nginx@sha256:...
```

**Build lại theo lịch.** Base image liên tục nhận cập nhật package. Một image sáu
tháng chưa build lại thì không thể chứa bản vá nào công bố sau ngày build, dù kết
quả quét hôm nay có sạch đến đâu.

**Ghi lại nguồn gốc.** Hai nhãn biến một image lạ trên máy thành thứ truy được:

```dockerfile
LABEL org.opencontainers.image.source="https://github.com/you/app"
LABEL org.opencontainers.image.revision="$GIT_SHA"
```

## Image nhỏ đi thường an toàn lên

Phần lớn finding đến từ các package ứng dụng không bao giờ gọi tới. Một base
`-slim` hoặc `-alpine`, hoặc build multi-stage chỉ ship artefact đã biên dịch, sẽ
loại chúng đi cùng với lý do phải vá chúng. Trang Security có gợi ý thay thế cho
các base image phổ biến.

Cần biết việc đổi ảnh hưởng gì: Alpine dùng musl thay vì glibc nên các phụ thuộc
native có thể phải build lại; image distroless không có shell — chính điều đó làm
nó an toàn hơn, và cũng chính điều đó làm việc debug bên trong khó hơn.

## Về điểm số

Con số dùng để sắp xếp danh sách. Bốn thành phần mới là câu trả lời: chúng nói
vấn đề nằm ở CVE, ở cấu hình, ở nguồn gốc hay ở tuổi image — mỗi thứ cần một việc
khác nhau.

Điểm chỉ so được với điểm tính bằng **cùng** trình quét, **cùng** ngày cơ sở dữ
liệu lỗ hổng và **cùng** phiên bản rule pack — vì thế cả ba đều được in ngay cạnh
nó. Hai trình quét khác nhau có thể lệch hơn mười lần trên cùng một image; đó là
bản chất của dữ liệu lỗ hổng, không phải lỗi của công cụ nào.

## Các mức nghiêm ngặt

L1 là những gì gần như mọi image đều nên làm. L2 thêm các khuyến nghị đúng với
phần lớn dịch vụ. L3 chặt, và một phần trong đó sẽ không áp dụng cho image cụ thể
của bạn. Mức cao hơn chỉ có thể làm điểm thấp đi — nó bật thêm rule, không bao
giờ bớt.

## Rule đến từ đâu

Rule của ColimaUI do chính dự án này viết. Chúng tham chiếu số hiệu mục của các
chuẩn đã công bố (ví dụ CIS Docker Benchmark §4.1) để bạn tra được bàn luận gốc,
nhưng câu chữ, cách chấm điểm và lời khuyên là của chúng tôi. ColimaUI không được
CIS hay OWASP chứng nhận, không liên kết và không được họ bảo trợ.
