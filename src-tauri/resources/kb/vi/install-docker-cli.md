# Cài đặt Docker CLI

## Triệu chứng

Colima đang chạy, nhưng ColimaUI báo `docker: command not found`, hoặc các
tính năng Compose bị mờ kèm thông báo *docker compose is unavailable*.

## Nguyên nhân

Colima cung cấp **daemon** Docker bên trong máy ảo. Nó không cung cấp
**client** `docker` để nói chuyện với daemon đó — đấy là một gói riêng. Docker
Compose v2 lại là một plugin riêng nữa, nên có thể thiếu Compose trong khi
`docker` thường vẫn chạy.

## Cách khắc phục

```bash
brew install docker docker-compose
docker version
docker compose version
```

Bạn **không** cần Docker Desktop. Thực tế, cài Docker Desktop song song với
Colima chính là nguồn nhầm lẫn phổ biến nhất, vì nó đăng ký daemon riêng và
context riêng của nó.

Nếu `docker` đã cài mà lệnh vẫn lỗi, tức là client đang trỏ sai daemon. Colima
đăng ký một context tên `colima` khi khởi động:

```bash
docker context ls
docker context use colima
docker ps
```

## Liên quan

- [Khởi động instance Colima](start-colima)
- [Các lỗi thường gặp](common-errors)
