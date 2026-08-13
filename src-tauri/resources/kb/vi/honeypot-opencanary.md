# Dịch vụ mồi với OpenCanary

## Đây là gì

OpenCanary chạy một tập dịch vụ giả — FTP, HTTP, banner SSH và nhiều thứ khác —
chúng không làm gì ngoài việc ghi lại ai đã chạm vào. Trong khi Cowrie mời kẻ
tấn công vào rồi quan sát, OpenCanary chỉ ra mở cửa và ghi lại rằng có người gõ.

Khác biệt đó quan trọng: OpenCanary mới là thứ bạn thực sự triển khai để phát
hiện, vì nó im lặng, rẻ, và gần như không sinh output nào cho tới khi có lượt
chạm thật.

## Trước khi chạy

Đọc **Honeypot trên Colima** trước. Cảnh báo về mạng công ty áp dụng mạnh nhất ở
đây — một máy chủ FTP giả trên mạng công ty trông y hệt một dịch vụ trái phép
dưới mắt người quản trị mạng đó.

Compose file bên dưới bind vào `127.0.0.1`. Khi triển khai thật bạn sẽ đổi chỗ
đó, và đó chính là quyết định mà bài tổng quan yêu cầu bạn ra một cách có ý thức.

## Chạy nó

OpenCanary cần một file cấu hình nói rõ bật dịch vụ nào. Lưu thành
`opencanary.conf`:

```json
{
  "device.node_id": "colima-canary",
  "ftp.enabled": true,
  "ftp.port": 21,
  "ftp.banner": "FTP server ready",
  "http.enabled": true,
  "http.port": 80,
  "http.banner": "Apache/2.2.22 (Ubuntu)",
  "http.skin": "nasLogin",
  "logger": {
    "class": "PyLogger",
    "kwargs": {
      "handlers": {
        "console": { "class": "logging.StreamHandler", "stream": "ext://sys.stdout" }
      }
    }
  }
}
```

Lưu thành `opencanary-compose.yml`:

```yaml
services:
  opencanary:
    image: thinkst/opencanary:latest
    restart: unless-stopped
    ports:
      # Bind 127.0.0.1 ở phía host chính là tính chất an toàn.
      - "127.0.0.1:2121:21"
      - "127.0.0.1:8080:80"
    volumes:
      - ./opencanary.conf:/root/.opencanary.conf:ro
```

Hai file phải nằm cùng một thư mục, và thư mục đó phải là thư mục Colima có chia
sẻ vào VM — thư mục home của bạn thì có, `/tmp` trên host thì không. Nếu VM
không thấy `opencanary.conf`, Docker sẽ tạo ra một *thư mục* mang tên đó thay vì
mount file của bạn. OpenCanary khi ấy không tìm thấy cấu hình, không khởi động
dịch vụ nào, và `restart: unless-stopped` lặng lẽ khởi động lại nó mãi mãi.
Triệu chứng là một container trông vẫn khoẻ mà không bao giờ ghi log gì.

Khởi động:

```bash
docker compose -f opencanary-compose.yml up -d
```

## Tự tạo lưu lượng

```bash
curl -s http://127.0.0.1:8080/ >/dev/null
```

Một request đó là một "lượt chạm". Lẽ ra không gì khác được tạo ra lượt nào.

## Bạn sẽ thấy gì

```bash
docker compose -f opencanary-compose.yml logs -f
```

Im lặng, rồi một dòng JSON cho mỗi lượt chạm, kèm địa chỉ nguồn, dịch vụ bị
chạm, và mốc thời gian. Sự im lặng chính là tính năng — bất cứ thứ gì xuất hiện
trong log này đều đáng để bạn chú ý, điều không đúng với hầu hết log bạn đang có.

Cấu hình trên là điểm khởi đầu với hai dịch vụ. OpenCanary hỗ trợ nhiều hơn
nữa; thêm chúng sau khi bạn đã thấy vòng lặp cơ bản chạy được.

## Dừng và dọn dẹp

```bash
docker compose -f opencanary-compose.yml down
```

## Liên quan

Honeypot trên Colima · Honeypot SSH với Cowrie · Đọc log honeypot
