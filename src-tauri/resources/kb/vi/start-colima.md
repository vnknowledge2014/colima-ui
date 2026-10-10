# Khởi động instance Colima

## Triệu chứng

Lỗi dạng *Cannot connect to the Docker daemon*, *Colima is not running*, hoặc
*the connection to the server was refused*. Các trang Container, Image,
Kubernetes trống trơn dù công cụ đã cài đủ.

## Nguyên nhân

Colima đã cài nhưng máy ảo của nó đang tắt. Mọi lệnh Docker và Kubernetes trong
app đều nói chuyện với daemon nằm **bên trong** máy ảo đó, nên khi VM tắt thì
không có gì lắng nghe cả. Đây là trạng thái bình thường sau khi khởi động lại
máy — Colima không tự chạy.

## Cách khắc phục

```bash
colima start

# or, for a named profile
colima start --profile dev
```

Hoặc bấm **Start** trên trang Instances. Lần khởi động đầu của một profile mất
một tới hai phút vì phải tải và tạo disk image; các lần sau chỉ vài giây.

Nếu khởi động thất bại, log của chính VM sẽ nói lý do:

```bash
colima status
colima start --verbose
tail -n 100 ~/.colima/default/daemon/daemon.log
```

Hai nguyên nhân đáng nhớ:

- **Máy host hết dung lượng.** Disk image được cấp phát trước. Hãy dọn ổ đĩa,
  hoặc giảm dung lượng disk ở Settings → Colima Config.
- **VM hỏng do lần khởi động trước bị ngắt giữa chừng.** Tạo lại — thao tác này
  xoá container và image trong VM, nhưng không đụng tới mã nguồn hay thư mục
  host được mount:

  ```bash
  colima stop
colima delete
colima start
  ```

## Liên quan

- [Các lỗi thường gặp](common-errors)
- [Tinh chỉnh hiệu năng](performance-tuning)
