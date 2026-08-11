# Cài đặt Colima

## Triệu chứng

ColimaUI báo `colima: command not found`, hoặc trang Setup hiển thị Colima ở
trạng thái **Missing**. Trang Instances không tải được gì.

## Nguyên nhân

Colima là một công cụ dòng lệnh riêng. ColimaUI chỉ là giao diện cho nó — ứng
dụng không đóng gói sẵn VM runtime, nên Colima phải nằm trong `PATH` thì mọi
thứ mới chạy. Ngoài ra, khi mở app từ Finder hoặc Dock, app nhận một `PATH`
ngắn hơn PATH của shell, nên một bản Colima cài ở thư mục không chuẩn có thể vô
hình với app dù chạy tốt trong terminal.

## Cách khắc phục

**macOS (Homebrew):**

```bash
brew install colima docker docker-compose
```

**Linux:**

```bash
# Debian/Ubuntu
sudo apt install colima docker.io docker-compose-v2

# Arch
sudo pacman -S colima docker docker-compose
```

Sau đó kiểm tra binary có phản hồi không:

```bash
colima version
colima status
```

Nếu `colima version` chạy được trong terminal mà ColimaUI vẫn báo Missing,
nghĩa là Colima nằm ngoài các thư mục app tìm kiếm. Tạo symlink vào
`/usr/local/bin` rồi khởi động lại app:

```bash
sudo ln -s "$(which colima)" /usr/local/bin/colima
```

## Liên quan

- [Khởi động instance Colima](start-colima)
- [Cài đặt Docker CLI](install-docker-cli)
