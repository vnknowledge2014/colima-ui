# Đọc log honeypot

## Một quy tắc duy nhất

Một dòng log honeypot không phải nhiễu để lọc bỏ. Không có gì hợp lệ kết nối tới
honeypot, nên mọi dòng đều nghĩa là có thứ gì đó đã chạm vào một dịch vụ mà lẽ
ra không ai được chạm. Coi số lượng là tín hiệu, không phải vấn đề cần tinh
chỉnh cho hết.

Đây là ngược lại với cách bạn đọc log ứng dụng, và cũng là lý do honeypot đáng
chạy.

## Trước khi hành động dựa trên những gì bạn đọc

Log honeypot ghi lại những gì kẻ tấn công tự khai về mình, và những lời khai đó
rất rẻ để làm giả.

- **Địa chỉ nguồn thường bị giả mạo hoặc đi mượn.** IP chạm vào bạn thường là
  một bên thứ ba đã bị chiếm, không phải kẻ điều khiển. Đừng trả đũa, đừng quét
  ngược lại, và đừng đưa nó vào một blocklist mà bạn khó gỡ ra.
- **File được tải vào honeypot là file thù địch.** Cowrie lưu lại thứ kẻ tấn
  công định tải về. Đừng mở, chạy, hay giải nén chúng trên máy bạn vì tò mò.
- **Đừng công bố log thô.** Chúng chứa địa chỉ của bên thứ ba và đôi khi cả
  thông tin đăng nhập tái sử dụng từ các vụ lộ dữ liệu thật. Che đi trước khi
  chia sẻ.

## Theo dõi trong ColimaUI

Các honeypot trong những bài này là project Compose bình thường, nên công cụ bạn
đã có đều dùng được:

- **Trang Compose** — tìm project, mở service, đọc log ngay trong app. Cách
  nhanh nhất để xem một phiên đang diễn ra.
- **Trang Containers** — xác nhận honeypot thực sự đang chạy và đã chạy bao lâu.
- **Trang Activity** — CPU và bộ nhớ theo thời gian. Một honeypot bỗng nhiên ăn
  CPU thật là điều tự nó đáng điều tra: nghĩa là có thứ gì đó đang làm nhiều hơn
  là gõ cửa.

## Cần nhìn gì

**Thông tin đăng nhập được thử.** Danh sách username và mật khẩu kẻ tấn công
dùng cho biết họ nghĩ bạn là ai. Toàn mật khẩu mặc định của router nghĩa là bạn
bị quét đại trà; username thật của ứng dụng bạn xuất hiện nghĩa là có thứ gì đó
nhắm vào bạn.

**Lệnh được chạy.** Trong Cowrie, lệnh đầu tiên sau khi đăng nhập là lệnh nhiều
thông tin nhất. `uname -a` là thăm dò. Một lệnh `wget` hay `curl` tới một địa
chỉ IP là nỗ lực kéo payload về — và URL nó tải chính là thông tin cụ thể bạn
hành động được.

**Sự lặp lại.** Cùng một địa chỉ nguồn quay lại khác hẳn với một nghìn kết nối
một lần. Kiểu thứ hai là nhiễu nền của internet; kiểu thứ nhất là ai đó đã để ý
tới bạn.

**Thời điểm.** Các lượt chạm dồn dập vài phút sau khi bạn phơi một dịch vụ nghĩa
là quét đại trà đã tìm thấy bạn. Điều này bình thường trên địa chỉ công cộng và
không nói gì về mức an toàn của bạn — nó chỉ nói internet đang bận.

## Giữ lại output

Cả hai honeypot đều ghi JSON có cấu trúc song song với log dạng người đọc:

```bash
# Cowrie — sao ra ngoài rồi đọc trên host.
docker compose -f cowrie-compose.yml cp \
  cowrie:/cowrie/cowrie-git/var/log/cowrie/cowrie.json ./cowrie.json
tail -20 cowrie.json
```

Dùng `cp` thay vì `exec cat` không phải chuyện phong cách: image Cowrie không có
shell và cũng không có `cat`, nên `exec` sẽ báo `executable file not found`.
Lệnh `cp` đi qua daemon và không cần gì bên trong container.

Nếu bạn để honeypot chạy lâu hơn một lần thử nghiệm, hãy sao file đó ra ngoài
định kỳ. Log container bị xoay vòng và volume bị dọn; bản ghi chỉ có giá trị nếu
nó sống sót.

## Khi nào nên dừng

Nếu bạn chạy honeypot để học cách nó hoạt động, hãy dừng khi đã xem trọn một
phiên từ đầu tới cuối. Một dịch vụ chạy mãi mà không ai đọc output không phải là
phát hiện — nó chỉ là một thứ dư thừa trên máy bạn, đang trả lời mạng.

## Liên quan

Honeypot trên Colima · Honeypot SSH với Cowrie · Dịch vụ mồi với OpenCanary
