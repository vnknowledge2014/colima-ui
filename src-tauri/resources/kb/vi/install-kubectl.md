# Cài kubectl và trỏ nó vào Colima

## Triệu chứng

Các trang Kubernetes trống, hoặc báo `kubectl: command not found`, hoặc
*The connection to the server localhost:8080 was refused*.

## Nguyên nhân

Cần hai thứ tách biệt và thiếu cái nào cũng hỏng:

1. Binary `kubectl` trên máy host.
2. Một cluster Kubernetes bên trong máy ảo Colima. Colima **không** tự bật —
   phải bật riêng cho từng profile.

Riêng thông báo `localhost:8080` có nghĩa kubectl chạy bình thường nhưng chưa
được cấu hình context nào, nên nó rơi về địa chỉ mặc định dựng sẵn.

## Cách khắc phục

Cài client:

```bash
brew install kubectl
kubectl version --client
```

Sau đó bật Kubernetes trong VM và chọn context của nó:

```bash
colima start --kubernetes
kubectl config get-contexts
kubectl config use-context colima
kubectl get nodes
```

Bạn cũng có thể bật Kubernetes ở Settings → Colima Config. Thao tác đó ghi cấu
hình vào `colima.yaml`; nó có hiệu lực ở lần khởi động lại instance kế tiếp.

Bật Kubernetes tốn thêm khoảng 1 GiB RAM và một phút khởi động, nên hãy để tắt
với những profile chỉ chạy container.

## Liên quan

- [Khởi động instance Colima](start-colima)
- [Tinh chỉnh hiệu năng](performance-tuning)
