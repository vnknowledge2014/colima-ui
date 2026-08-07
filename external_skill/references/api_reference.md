# ColimaUI API Reference

Đây là tài liệu tham khảo các REST API chính mà Orchestrator có thể gọi trực tiếp để điều khiển ColimaUI.
Cổng mặc định của API Server là `11420`. Tất cả các request đều yêu cầu header `Authorization: Bearer <TOKEN>`.
Token có thể lấy bằng cách đọc file `~/.colimaui/api_token` hoặc gọi `GET /api/auth/token`.

## 1. AI, Trợ lý ảo & Cấu hình (AI Agent & Settings API)
- `POST /api/cli/chat`: Tương tác với ColimaUI AI bằng Prompt. Body: `{"prompt": "...", "provider": "...", "model": "..."}`.
- `GET /api/settings`: Lấy toàn bộ cấu hình.
- `POST /api/settings`: Đọc và cập nhật các cấu hình. Ví dụ: `{"key": "ai_provider", "value": "anthropic"}`.
- `GET /api/auth/token`: Lấy Bearer token để xác thực các request khác.

## 2. Docker & Container API
- `GET /api/containers`: Lấy danh sách toàn bộ containers.
- `POST /api/containers/start`, `stop`, `restart`, `remove`, `pause`, `unpause`: Quản lý trạng thái container. Body: `{"id": "..."}`.
- `GET /api/containers/logs`: Lấy logs. Truyền `?id=...&lines=...`.
- `GET /api/containers/inspect`, `stats`, `top`: Lấy thông tin chi tiết. Truyền `?id=...`.
- `POST /api/containers/exec`: Chạy lệnh trong container. Body: `{"id": "...", "cmd": ["..."]}`.
- `POST /api/containers/run`: Chạy container mới từ image.
- `GET /api/images`: Lấy danh sách Docker images.
- `POST /api/images/pull`, `remove`, `prune`, `tag`: Quản lý images.
- `GET /api/images/inspect`: Thông tin image. Truyền `?id=...`.
- `GET /api/networks`: Lấy danh sách networks.
- `POST /api/networks/create`, `remove`, `prune`: Quản lý networks.
- `GET /api/volumes`: Lấy danh sách volumes.
- `POST /api/volumes/create`, `remove`, `prune`: Quản lý volumes.
- `GET /api/docker/df`: Xem dung lượng đĩa docker.
- `POST /api/docker/prune`: Dọn dẹp system prune.

## 3. Kubernetes API
- `GET /api/k8s/contexts`: Lấy danh sách các K8s cluster contexts.
- `GET /api/k8s/contexts/current`: Lấy context hiện tại.
- `POST /api/k8s/contexts/set`: Chuyển context. Body: `{"context": "..."}`.
- `GET /api/k8s/namespaces`, `pods`, `services`, `deployments`, `nodes`, `events`, `crds`: Lấy danh sách tài nguyên.
- `GET /api/k8s/resources`: Lấy danh sách tài nguyên tổng quát. Truyền `?kind=...&namespace=...`.
- `POST /api/k8s/apply`: Apply một resource YAML vào cụm K8s. Body: `{"yaml": "..."}` hoặc đường dẫn file.
- `POST /api/k8s/scale`, `scale-generic`: Thay đổi replica. Body: `{"namespace": "...", "name": "...", "replicas": ...}`.
- `POST /api/k8s/resources/delete`, `resources/restart`: Quản lý tài nguyên chung.
- `GET /api/k8s/pods/logs`, `pods/container-logs`: Đọc logs.
- `POST /api/k8s/port-forward/start`, `stop`: Mở cổng local.
- `GET /api/k8s/port-forward/list`: Danh sách cổng đang forward.
- `POST /api/k8s/exec`: Mở phiên lệnh vào pod.

## 4. Colima / Lima VMs API
- `GET /api/instances`: Lấy danh sách các máy ảo Colima.
- `POST /api/instances/start`, `stop`, `delete`: Quản lý máy ảo Colima.
- `GET /api/instances/status`: Lấy trạng thái chi tiết. Truyền `?profile=...`.
- `GET /api/lima`: Lấy danh sách máy ảo Lima.
- `POST /api/lima/start`, `stop`, `delete`, `create`: Quản lý Lima VM.
- `POST /api/lima/shell`: Chạy lệnh trong Lima VM. Body: `{"name": "...", "command": "..."}`.

## 5. System API
- `GET /api/system/check`, `version`, `host-specs`, `platform`: Lấy thông tin hệ thống.
- `GET /api/system/check-tool`: Kiểm tra công cụ được cài đặt. Truyền `?tool=...`.
- `POST /api/system/install`: Cài đặt dependencies (như docker, colima).

## 6. Docker Compose API
- `GET /api/compose`: Danh sách projects.
- `POST /api/compose/up`, `down`, `restart`: Quản lý compose.
- `GET /api/compose/logs`, `ps`: Xem logs và process.

## 7. Knowledge Bank & Sandbox API
- `POST /api/kb/query`, `search`: Truy vấn Knowledge Bank.
- `GET /api/kb/memories`, `POST /api/kb/memories/update`, `delete`: Quản lý bộ nhớ AI.
- `POST /api/sandbox/execute`, `execute-approved`: Thực thi lệnh bash an toàn (sandbox).

*Ghi chú: Để gọi chính xác nhất, hãy tham khảo các payload trong `capabilities.md` và ánh xạ sang API tương ứng, hoặc dùng chế độ AI Chat (Delegation Mode) để ColimaUI tự gọi API.*
