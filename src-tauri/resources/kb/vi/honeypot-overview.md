# Honeypot trên Colima

## Honeypot là gì

Honeypot là một dịch vụ tồn tại chỉ để bị tấn công. Không có gì hợp lệ kết nối
tới nó, nên mọi kết nối nó ghi lại đều là thứ bạn không mong đợi. Đó chính là ý
tưởng: không có báo động giả từ lưu lượng bình thường, vì không có lưu lượng
bình thường nào cả.

Container khiến honeypot trở nên dễ chạy tại chỗ. Mỗi cái là một tiến trình dùng
một lần với filesystem riêng, và Colima vốn đã giữ chúng trong một VM.

## Hai lý do rất khác nhau để chạy

Phải rõ bạn đang làm cái nào, vì hồ sơ rủi ro của chúng ngược nhau.

**Để học.** Chạy trên máy của bạn, bind vào localhost, và tự kết nối vào để xem
log trông ra sao. Không có gì thù địch chạm tới nó. Cách này an toàn và là thứ
các bài viết ở đây mô tả.

**Để phát hiện.** Đặt nó trên một mạng mà lẽ ra không gì được chạm tới, và coi
mọi lần bị chạm là tín hiệu thật. Cách này thực sự hữu ích và cũng thực sự có
hậu quả — đọc cảnh báo bên dưới trước khi tới gần nó.

## Trước khi phơi ra mạng thật

Đây không phải thủ tục cho có.

- **Honeypot đặt trên internet công cộng thu hút lưu lượng thù địch về địa chỉ
  IP của bạn.** Đó là công việc của nó. Bạn đang chọn trở thành mục tiêu, và
  lưu lượng đó không dừng lại khi bạn hết hứng thú.
- **Kiểm tra chính sách nơi bạn làm việc trước.** Trên mạng công ty, chạy một
  dịch vụ giả mạo hạ tầng có thể vi phạm chính sách ngay cả khi ý định của bạn
  là phòng thủ. Hỏi trước, đừng hỏi sau.
- **Nhà cung cấp hosting cũng có thể có quy định.** Một số cấm hẳn; số khác yêu
  cầu thông báo trước.
- **Honeypot bị chiếm là một bàn đạp.** Các dịch vụ này yếu có chủ đích. Nếu ai
  đó thoát ra được, kẻ tấn công giờ đang ở bên trong mạng bạn đã đặt nó vào.

Mọi compose file trong các bài này đều bind vào `127.0.0.1` vì lý do đó. Nếu bạn
đổi thành `0.0.0.0`, bạn đã ra một quyết định — hãy ra nó một cách có ý thức.

## Bắt đầu từ đâu

Đọc **Honeypot SSH với Cowrie** trước. Nó cho log dễ đọc nhất và bạn tự tạo được
lưu lượng bằng một lệnh, nên sẽ thấy trọn vòng lặp hoạt động trước khi quyết
định có muốn đi xa hơn không.

**Dịch vụ mồi với OpenCanary** gần với cách chúng được dùng để phát hiện thật:
im lặng cho tới khi có gì đó chạm vào.

**Đọc log honeypot** nói về việc làm gì với output khi đã có.

## Liên quan

Honeypot SSH với Cowrie · Dịch vụ mồi với OpenCanary · Đọc log honeypot
