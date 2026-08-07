# Danh sách Năng lực (Capabilities) của ColimaUI

Đây là cuốn "từ điển" liệt kê chính xác mọi hành động mà **ColimaUI AI Agent** có thể hiểu và thực thi. 
Khi phân tích yêu cầu của User, hãy tham chiếu danh sách này để biết ColimaUI có khả năng thực hiện hay không. **TUYỆT ĐỐI KHÔNG** bịa ra các hành động nằm ngoài danh sách dưới đây.

## Colima / Máy ảo (Instances)
- `list-instances`: Lấy danh sách các máy ảo Colima hiện có.
- `create-instance`: Tạo và khởi động máy ảo Colima mới từ một cấu hình preset.
- `start-instance`: Khởi động máy ảo Colima đã bị dừng.
- `stop-instance`: Tắt/Dừng máy ảo Colima.
- `colima-restart`: Khởi động lại máy ảo Colima.
- `delete-instance`: Xóa vĩnh viễn máy ảo Colima (Nguy hiểm).
- `colima-status`: Lấy chi tiết trạng thái hoạt động (CPU, RAM, Disk).

## Docker Containers
- `list-containers`: Lấy danh sách toàn bộ container (đang chạy và đã dừng).
- `start-container`: Bắt đầu (start) một container.
- `stop-container`: Dừng (stop) một container.
- `restart-container`: Khởi động lại container.
- `remove-container`: Xóa container.
- `container-logs`: Lấy nội dung logs của container.
- `container-stats`: Lấy thống kê sử dụng CPU/RAM của 1 container.
- `all-container-stats`: Lấy thống kê toàn bộ container.
- `container-top`: Xem các process đang chạy trong container.
- `inspect-container`: Xem toàn bộ cấu hình JSON của container.
- `container-exec`: Chạy một lệnh (exec) bên trong container.
- `run-container`: Chạy container mới từ Image.
- `pause-container`: Tạm ngưng (Pause) container.
- `unpause-container`: Kích hoạt lại (Unpause) container.
- `rename-container`: Đổi tên container.

## Docker Images
- `list-images`: Lấy danh sách Images.
- `remove-image`: Xóa Image.
- `pull-image`: Kéo Image từ Registry.
- `prune-images`: Dọn dẹp các Image rác/dangling.
- `inspect-image`: Xem chi tiết cấu hình Image.
- `tag-image`: Gắn tag mới cho Image.
- `build-image`: Xây dựng Image mới từ Dockerfile.

## Docker Volumes & Networks
- `volume-list`, `volume-inspect`, `volume-create`, `volume-remove`, `volume-prune`: Quản lý Volume.
- `network-list`, `network-inspect`, `network-create`, `network-remove`, `network-prune`: Quản lý Network.

## Docker Compose
- `compose-list`: Liệt kê các dự án compose.
- `compose-up`: Khởi chạy compose.
- `compose-down`: Tắt và dỡ bỏ compose.
- `compose-restart`: Khởi động lại compose.
- `compose-logs`: Lấy logs của compose.
- `compose-build`: Xây dựng/Build lại các service trong compose.
- `compose-pull`: Kéo (pull) các image cần thiết cho dự án compose.

## Kubernetes (K8s / K3s / Kind)
- `k8s-list-contexts`, `k8s-current-context`, `k8s-set-context`: Quản lý cụm.
- `k8s-list-namespaces`: Danh sách Namespace.
- `k8s-list-resources`: Danh sách tài nguyên (Pods, Deployments, Services,...).
- `k8s-describe`: Xem chi tiết (Describe) tài nguyên.
- `k8s-logs`, `k8s-container-logs`: Đọc logs của Pod.
- `k8s-apply`: Triển khai tệp YAML.
- `k8s-delete`: Xóa tài nguyên K8s.
- `k8s-yaml`: Lấy file YAML gốc của tài nguyên.
- `k8s-pod-containers`: Lấy danh sách các containers trong một Pod.
- `k8s-nodes`, `k8s-node-action`: Quản lý Node (Cordon/Drain).
- `k8s-crds`, `k8s-crd-resources`: Quản lý Custom Resource Definitions.
- `k8s-scale`, `k8s-generic-scale`: Tăng giảm số lượng Replica.
- `k8s-restart-resource`: Tự động khởi động lại (Rolling restart).
- `k8s-port-forward-start`, `k8s-port-forward-stop`, `k8s-port-forward-list`: Mở cổng Local.
- `k8s-exec`: Mở phiên lệnh (Exec) vào Pod.
- `k8s-cluster-health`, `k8s-benchmark`, `k8s-events`: Theo dõi tình trạng cụm.
- `kind-list`, `kind-create`, `kind-delete`: Quản lý cụm Kind (K8s trong Docker).

## Hệ thống & Lima VMs
- `system-df`: Xem dung lượng đĩa (Docker df).
- `system-host-specs`, `system-check`: Kiểm tra tài nguyên và công cụ của máy chủ MacOS.
- `system-prune`: Dọn dẹp rác toàn hệ thống (Containers, Networks, Images, Volumes).
- `cli-exec`: Thực thi lệnh bash tùy ý qua App CLI gateway.
- `lima-list`, `lima-start`, `lima-stop`, `lima-delete`, `lima-create`, `lima-info`, `lima-templates`, `lima-shell`: Toàn quyền thao tác trên Lima VM.

## AI Models & Cấu hình Ứng dụng
- `model-list`, `model-pull`, `model-serve`, `model-delete`: Quản lý tải và chạy các mô hình AI Local (Llama, Gemma,...).
**Sử dụng khi:** Đọc và cập nhật cấu hình nội bộ (Tự nhận thức).

### Danh sách các cấu hình khả dụng (EXACT KEYS - DO NOT GUESS):
- `ai_provider` (anthropic, openai, ollama)
- `ai_model` (gpt-4o, claude-3, v.v)
- `ai_api_key` (string)
- `ai_endpoint` (string)
- `ai_searxng_instances` (JSON string array)
- `ai_diag_content_mode` (full, summary)
- `ai_diag_max_page_size` (string number, e.g., "8000")
- `ai_diag_auto_trigger` ("true", "false")
- `colimaui_auto_pause` ("true", "false")
- `colimaui_auto_pause_mins` (string number, e.g., "15")

#### `ai-config-status`
**Input:** `{}`
**Output:** JSON chứa toàn bộ key-value của cấu hình hiện tại.

#### `ai-update-config`
**Input:** `{"settings": {"colimaui_auto_pause": "true", "colimaui_auto_pause_mins": "15"}}` (Luôn luôn dùng kiểu String cho value)
**Output:** Confirmation. (Sau khi update xong, hãy nhắc User refresh UI).

- `update-setting`: Bật tắt giao diện Dark Mode, Auto-Pause, Idle Threshold.
- `list-presets`: Lấy danh sách toàn bộ cấu hình máy ảo Custom đã lưu.
- `save-preset`: Lưu cấu hình máy ảo Custom.
- `delete-preset`: Xóa một cấu hình máy ảo Custom.
