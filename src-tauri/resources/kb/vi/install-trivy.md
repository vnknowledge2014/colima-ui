# Cài Trivy để quét image

## Triệu chứng

Trang Security báo thiếu trình quét, hoặc bấm quét một image thì không có gì xảy
ra.

## Nguyên nhân

ColimaUI không đóng gói sẵn trình quét lỗ hổng. Nó điều khiển Trivy, và Trivy
phải được cài trên máy bạn. Lý do không đóng gói là kích thước: riêng cơ sở dữ
liệu lỗ hổng của Trivy đã khoảng 1.2 GB và đổi mỗi ngày — đóng gói theo nghĩa là
đóng gói một bản đã cũ.

## Cách sửa

macOS:

```bash
brew install trivy
```

Linux: làm theo hướng dẫn cho bản phân phối của bạn tại
`https://trivy.dev/latest/getting-started/installation/`.

Kiểm tra:

```bash
trivy --version
```

Sau đó quay lại ColimaUI và tải lại trang Security.

## Lần quét đầu tiên sẽ tải cơ sở dữ liệu

Lần quét đầu (hoặc lần đầu sau vài ngày) phải tải cơ sở dữ liệu lỗ hổng trước.
Bước đó được hiển thị riêng trong thanh tiến trình, vì nó mới là phần chậm —
khoảng 40 giây với đường truyền tốt, so với khoảng 2.5 giây để quét một image
200 MB sau đó.

Khi cơ sở dữ liệu đã nằm trên đĩa, việc quét chạy được cả khi không có mạng.

## Cái gì được gửi đi đâu

Việc quét đọc image từ container runtime cục bộ. Tên image, danh sách package và
kết quả quét đều ở lại trên máy bạn. Yêu cầu ra ngoài duy nhất là Trivy tải cơ sở
dữ liệu lỗ hổng của chính nó.

## Khi một image quét hỏng

Một số image trình quét không đọc được — thường là một layer nó không giải nén
được. Lỗi đó chỉ thuộc về image ấy; các image còn lại vẫn quét bình thường. Thông
báo lỗi của trình quét được hiện nguyên văn, vì đó là manh mối duy nhất.
