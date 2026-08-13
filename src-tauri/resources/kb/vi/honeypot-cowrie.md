# Honeypot SSH với Cowrie

## Đây là gì

Cowrie giả làm một máy chủ SSH có mật khẩu yếu. Khi ai đó đăng nhập, nó đưa cho
họ một shell giả: lệnh có vẻ chạy được, filesystem trông như thật, và không thứ
gì họ gõ chạm tới máy bạn. Nó ghi lại phiên làm việc — từng thông tin đăng nhập
được thử, từng lệnh được chạy, từng file được tải về.

Nó phục vụ được cả Telnet, nhưng cấu hình mặc định tắt sẵn, nên bài này chỉ mở
cổng SSH.

Đây là điểm khởi đầu tốt nhất vì log đọc được ngay mà không cần công cụ gì.

## Trước khi chạy

Đọc **Honeypot trên Colima** trước nếu bạn chưa đọc. Compose file bên dưới bind
vào `127.0.0.1`, nên chỉ máy của bạn tới được. Đổi chỗ đó là phơi một dịch vụ
SSH yếu có chủ đích ra mạng bạn gắn nó vào.

Cowrie không chạy bằng root và không cần chế độ privileged. Nếu một hướng dẫn
bảo bạn thêm `privileged: true`, bạn đang đọc hướng dẫn cho thứ khác.

## Chạy nó

Lưu thành `cowrie-compose.yml`:

```yaml
services:
  cowrie:
    image: cowrie/cowrie:latest
    restart: unless-stopped
    ports:
      # Bind 127.0.0.1 ở phía host chính là tính chất an toàn. Đừng bỏ nó đi
      # nếu chưa đọc phần cảnh báo trong bài tổng quan.
      - "127.0.0.1:2222:2222"
    volumes:
      # Thư mục làm việc của Cowrie là /cowrie/cowrie-git, không phải /cowrie.
      # Mount vào /cowrie/var sẽ tạo ra một thư mục rỗng mà honeypot không bao
      # giờ ghi vào, và mọi phiên đã ghi sẽ mất sạch ở lần `down -v` kế tiếp.
      - cowrie-var:/cowrie/cowrie-git/var
      - cowrie-etc:/cowrie/cowrie-git/etc

volumes:
  cowrie-var:
  cowrie-etc:
```

Khởi động:

```bash
docker compose -f cowrie-compose.yml up -d
```

## Tự tạo lưu lượng

Tự kết nối vào. Mật khẩu nào cũng được — đó là điểm mấu chốt:

```bash
ssh -p 2222 root@127.0.0.1
```

Chấp nhận host key, gõ mật khẩu bất kỳ, và bạn vào shell giả. Thử `ls`,
`cat /etc/passwd`, `wget http://example.com/x`. Không lệnh nào chạm tới hệ thống
của bạn. Gõ `exit` khi xong.

## Bạn sẽ thấy gì

```bash
docker compose -f cowrie-compose.yml logs -f
```

Mỗi dòng là một sự kiện có cấu trúc: mở kết nối, thử đăng nhập, chạy lệnh, đóng
phiên. Cùng các sự kiện đó nằm dạng JSON trong
`/cowrie/cowrie-git/var/log/cowrie/cowrie.json` bên trong container — đó mới là
định dạng đáng parse nếu bạn để nó chạy lâu dài.

Xem **Đọc log honeypot** để biết làm gì với chúng.

## Dừng và dọn dẹp

```bash
docker compose -f cowrie-compose.yml down -v
```

Cờ `-v` xoá luôn các phiên đã ghi. Bỏ nó đi nếu bạn muốn giữ lại.

## Liên quan

Honeypot trên Colima · Đọc log honeypot
