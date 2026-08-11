# Tinh chỉnh hiệu năng

## Triệu chứng

Build chậm, thay đổi file mất vài giây mới thấy trong container, hoặc cả máy ì
ạch khi VM đang chạy.

## Nguyên nhân

Gần như toàn bộ nằm ở ba thiết lập, và mặc định được đặt dè dặt vì phải khởi
động được trên cả cấu hình yếu nhất được hỗ trợ:

- **VM type.** `qemu` là giả lập; `vz` dùng framework Virtualization gốc của
  Apple. Trên Apple Silicon, `vz` nhanh hơn đáng kể.
- **Mount type.** `sshfs` đẩy mọi thao tác file qua đường SSH. `virtiofs` là
  filesystem chia sẻ gốc và là cải thiện lớn nhất cho các workload dùng nhiều
  bind mount như dự án Node hay PHP.
- **Kích thước.** Hai CPU và 2 GiB đủ để khởi động VM, không đủ để build trong
  đó.

## Cách khắc phục

Trên Apple Silicon, đổi VM type và mount type. Thao tác này không tạo lại gì cả
— chỉ là một lần khởi động lại:

```bash
colima stop
colima start --vm-type vz --mount-type virtiofs
```

Rồi cấp thêm tài nguyên. Quy tắc chung: một nửa số nhân và một nửa RAM của host:

```bash
colima stop
colima start --cpu 4 --memory 8 --disk 100
```

Cả hai đều sửa được ở Settings → Colima Config — nơi ghi `colima.yaml` và cho
bạn xem diff trước khi áp dụng. Thay đổi có hiệu lực ở lần khởi động lại sau.

Cuối cùng, dọn dẹp định kỳ — ổ đĩa đầy trông hệt như máy chậm:

```bash
docker system df
docker system prune -a --volumes
```

## Những việc không nên làm

- Đừng cấp toàn bộ số nhân cho VM. Nó tranh chấp với scheduler của host và cả
  máy chậm đi.
- Đừng để bật Kubernetes trên profile chỉ chạy container; nó ăn thường trực
  khoảng 1 GiB RAM.
- Đừng cấp disk thật lớn với ý định sửa lại sau. Disk chỉ lớn lên, không nhỏ đi.

## Liên quan

- [Các lỗi thường gặp](common-errors)
- [Khởi động instance Colima](start-colima)
