# Các lỗi thường gặp

Bảng tra nhanh cho những thông báo ColimaUI hiển thị nhiều nhất.

## Permission denied

**Triệu chứng.** *permission denied*, *operation not permitted*, hoặc thư mục
được mount nhìn thấy rỗng bên trong container.

**Nguyên nhân.** Colima mount thư mục host vào VM theo quyền sở hữu của user
bạn. Container chạy bằng UID khác sẽ không ghi được, và macOS còn chặn
Documents, Desktop, Downloads khỏi VM cho tới khi bạn cấp quyền.

**Khắc phục.** Cấp quyền ở System Settings → Privacy & Security → Files and
Folders rồi khởi động lại instance. Với trường hợp lệch UID, chạy container bằng
chính user của bạn:

```bash
docker run --user "$(id -u):$(id -g)" -v "$PWD:/work" -w /work alpine sh
```

## Operation timed out

**Triệu chứng.** *timed out*, *deadline exceeded*, hoặc UI treo rồi báo timeout.

**Nguyên nhân.** VM còn sống nhưng quá bận để trả lời — thường là đang pull một
image lớn, đang build, hoặc thiếu RAM khiến VM rơi vào swap.

**Khắc phục.** Chờ thao tác đang chạy xong, hoặc xem cái gì đang ăn tài nguyên:

```bash
colima ssh -- top -b -n 1 | head -20
```

Nếu lỗi tái diễn cả khi rảnh thì VM bị cấp thiếu tài nguyên — xem
[tinh chỉnh hiệu năng](performance-tuning).

## Lỗi mạng

**Triệu chứng.** *connection refused*, *could not resolve host*,
*x509 certificate*.

**Nguyên nhân.** DNS bên trong VM tách biệt với DNS của host. VPN và proxy công
ty mà host phân giải qua đó thường không hiện diện trong VM, và CA của proxy
chặn TLS cũng không được cài bên trong.

**Khắc phục.** Đặt DNS tường minh ở Settings → Colima Config (ví dụ
`1.1.1.1`) rồi khởi động lại instance. Kiểm chứng từ bên trong:

```bash
colima ssh -- nslookup registry-1.docker.io
```

## Port is already allocated

**Triệu chứng.** *bind: address already in use* khi khởi động container.

**Nguyên nhân.** Một tiến trình khác trên host đang giữ cổng đó. Colima forward
cổng đã publish ra host nên xung đột phía host vẫn áp dụng.

**Khắc phục.**

```bash
lsof -nP -iTCP:8080 -sTCP:LISTEN
```

Dừng tiến trình đó, hoặc publish sang cổng host khác.

## No space left on device

**Triệu chứng.** Build và pull thất bại với *no space left on device* trong khi
host vẫn còn trống.

**Nguyên nhân.** VM có ổ đĩa riêng, kích thước cố định. Đầy ổ VM không liên quan
gì tới ổ host.

**Khắc phục.** Dọn bên trong VM trước:

```bash
docker system df
docker system prune -a --volumes
```

Nếu vẫn đầy, tăng dung lượng disk ở Settings → Colima Config. Disk chỉ có thể
lớn lên, không thu nhỏ lại được.

## Error getting credentials

**Triệu chứng.** Kéo image thất bại — trong app hoặc trên dòng lệnh — với
thông báo *error getting credentials — err: exec:
"docker-credential-osxkeychain": executable file not found in $PATH*.

**Nguyên nhân.** `~/.docker/config.json` khai `"credsStore": "osxkeychain"`,
yêu cầu Docker CLI đọc credential của registry qua một binary helper. Helper đó
do Docker Desktop cung cấp; gỡ Docker Desktop sẽ để lại thiết lập này mà không
còn binary. Khi đó mọi lần pull đều lỗi, kể cả pull ẩn danh image công khai —
CLI gọi helper trước khi biết có cần credential hay không.

**Cách sửa.** Cài lại helper.

```bash
brew install docker-credential-helper
```

Lệnh này đặt `docker-credential-osxkeychain` vào `/opt/homebrew/bin` — thư
mục ColimaUI vốn đã đưa vào PATH cấp cho tiến trình con, nên app nhận ra ngay
mà không cần cấu hình thêm. Credential vẫn nằm trong Keychain của macOS.

Xoá dòng `"credsStore"` khỏi `~/.docker/config.json` cũng khiến pull chạy
lại, nhưng nên tránh: sau đó `docker login` sẽ ghi credential dạng văn bản
thuần vào chính file này.

## Liên quan

- [Khởi động instance Colima](start-colima)
- [Tinh chỉnh hiệu năng](performance-tuning)
