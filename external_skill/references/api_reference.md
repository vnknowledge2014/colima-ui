# ColimaUI API Reference

Tài liệu tham khảo đầy đủ các REST API và luồng SSE mà Orchestrator có thể gọi trực tiếp để điều khiển **ColimaUI** (đồng bộ với `src-tauri/src/api_server.rs`).

## Cơ chế Cổng & Xác thực (Port & Auth)

```bash
PORT="$(cat ~/.colima-ui/api_port 2>/dev/null || echo 11420)"
TOKEN="$(cat ~/.colima-ui/api_token)"
```

- **Cổng hoạt động**: Mặc định `11420` trên `127.0.0.1` (tự động dò cổng trống trong dải `11420–11429` và ghi vào `~/.colima-ui/api_port`).
- **Bearer Token**: Đọc trực tiếp từ file `~/.colima-ui/api_token` (quyền `0600`).
- **Không cấp token qua HTTP**: Endpoint cấp token qua HTTP trước đây đã bị loại bỏ hoàn toàn vì lý do bảo mật.
- **Phân loại xác thực**:
  - **Public (không cần token)**: Duy nhất `GET /api/health`.
  - **SSE Streams (xác thực qua query `?token=<TOKEN>`)**: `GET /api/events` và `GET /api/k8s/pods/logs/stream`.
  - **Tất cả REST endpoints còn lại**: Bắt buộc có header `Authorization: Bearer <TOKEN>` (kèm `Content-Type: application/json` cho các lệnh `POST` có JSON body).

---

## 1. Health, Capabilities, Entitlement, Telemetry & Realtime SSE
- `GET /api/health`: Kiểm tra trạng thái sống của API server (public, không cần token).
- `GET /api/capabilities`: Lấy schema năng lực và Event Bus (100 sự kiện) dành cho AI Agent / Orchestrator.
- `GET /api/announcements`: Lấy thông báo hệ thống mới nhất.
- `GET /api/telemetry/consent`, `POST /api/telemetry/consent`, `GET /api/telemetry/should-prompt`, `GET /api/telemetry/preview`: Quản lý trạng thái đồng ý (opt-in) và xem trước dữ liệu telemetry ẩn danh.
- `GET /api/pro/status`: Kiểm tra trạng thái Pro sidecar cục bộ (mặc định `{"state": "free"}` trên bản MIT).
- `GET /api/subscription`, `POST /api/subscription/store`: Đọc / lưu bộ nhớ đệm quyền sử dụng offline cục bộ.
- `GET /api/events?token=<TOKEN>&topics=instances-update,docker-state-updated,metrics.sample`: Đăng ký luồng sự kiện thời gian thực (SSE). Tham số `topics` tùy chọn (tối đa 16 topics; các topic chuẩn gồm `docker-state-updated`, `instances-update`, `metrics.sample`, `transfer.progress`, `transfer.done`, `transfer.failed`, `security-scan-progress`, `detonation.event`, `detonation.status`).

## 2. AI Chat, Đa hội thoại (Multi-Conversation History) & Settings
- `POST /api/cli/chat`: Ủy quyền tác vụ cho vòng lặp Headless AI Agent đa bước. Body: `{"prompt": "...", "provider": "...", "model": "...", "allow_mutations": false}` → trả về `{"result": "..."}`.
- `POST /api/ai/chat`: Gửi hội thoại trực tiếp tới LLM đang cấu hình (`provider`, `model`, `api_key`, `endpoint`, `messages`).
- `POST /api/ai/models`: Liệt kê danh sách model khả dụng từ provider.
- `POST /api/ai/search`, `POST /api/ai/fetch-page`: Tìm kiếm tài liệu web (SearXNG) và trích xuất trang thành Markdown.
- `GET /api/ai/context`: Đọc ngữ cảnh hệ thống (`system_prompt.md`, `router.md`, core skill & plugins).
- `GET /api/ai/conversations`: Lấy danh sách các cuộc hội thoại AI.
- `POST /api/ai/conversations`: Tạo cuộc hội thoại mới (`{"id": "...", "title": "..."}`).
- `POST /api/ai/conversations/rename`: Đổi tên cuộc hội thoại (`{"id": "...", "title": "..."}`).
- `POST /api/ai/conversations/delete`: Xóa cuộc hội thoại (`{"id": "..."}`).
- `GET /api/ai/history`: Tải lịch sử tin nhắn (truyền `?conversationId=...`).
- `POST /api/ai/history`: Lưu tin nhắn vào lịch sử (`{"conversationId": "...", "message": {"id": "...", "role": "...", "content": "..."}}`).
- `POST /api/ai/history/clear`: Xóa lịch sử của một cuộc hội thoại (`{"conversationId": "..."}`).
- `GET /api/models`, `POST /api/models/pull`, `POST /api/models/serve`, `POST /api/models/delete`: Quản lý AI models chạy cục bộ.
- `GET /api/settings`, `POST /api/settings`: Đọc toàn bộ cài đặt hoặc cập nhật từng khóa (`{"key": "ai_provider", "value": "anthropic"}`).

## 3. Docker, Topology & Background Transfers
- **Containers**:
  - `GET /api/containers`: Liệt kê toàn bộ containers (`?all=true`).
  - `POST /api/containers/start`, `stop`, `restart`, `remove`, `pause`, `unpause`, `rename`: Quản lý vòng đời container.
  - `GET /api/containers/logs`, `inspect`, `stats`, `top`: Tra cứu chi tiết container (`?containerId=...&lines=...`).
  - `GET /api/containers/stats/all`: Thống kê tài nguyên toàn bộ containers đang chạy.
  - `POST /api/containers/exec`: Thực thi lệnh trong container.
  - `POST /api/containers/run`: Khởi chạy container mới từ image.
- **Images, Volumes, Networks & Disk**:
  - `GET /api/images`, `POST /api/images/pull`, `remove`, `prune`, `tag`, `GET /api/images/inspect`: Quản lý Docker images.
  - `GET /api/volumes`, `POST /api/volumes/create`, `remove`, `prune`, `GET /api/volumes/inspect`: Quản lý Docker volumes.
  - `GET /api/networks`, `POST /api/networks/create`, `remove`, `prune`, `GET /api/networks/inspect`: Quản lý Docker networks.
  - `GET /api/docker/df`, `POST /api/docker/prune`: Kiểm tra dung lượng và dọn rác Docker.
- **Topology Graph**:
  - `GET /api/topology`: Lấy đồ thị liên kết giữa containers, networks và volumes.
- **Background Transfers** (trả về `jobId` ngay lập tức, tiến độ phát qua SSE `/api/events`):
  - `POST /api/images/save`: Xuất image ra file `.tar` (`{"images": [...], "destDir": "...", "fileName": "..."}`).
  - `POST /api/images/load`: Nạp image từ file `.tar` (`{"tarPath": "..."}`).
  - `POST /api/containers/cp/to`: Sao chép file/thư mục từ máy chủ vào container (`{"containerId": "...", "hostPath": "...", "containerPath": "..."}`).
  - `POST /api/containers/cp/from`: Sao chép dữ liệu từ container ra máy chủ (`{"containerId": "...", "containerPath": "...", "destDir": "...", "fileName": "..."}`).
  - `GET /api/transfers`: Liệt kê trạng thái các tiến trình truyền tải nền.
  - `POST /api/transfers/cancel`: Hủy tiến trình truyền tải (`{"jobId": "..."}`).

## 4. Docker Compose, Diagnose & AI Autofix
- `GET /api/compose`: Liệt kê các dự án Docker Compose.
- `POST /api/compose/up`, `down`, `restart`, `stop`, `start`: Quản lý vòng đời dự án Compose.
- `GET /api/compose/logs`, `GET /api/compose/ps`: Đọc logs và danh sách tiến trình (`?projectName=...`).
- `POST /api/compose/diagnose`: Phân tích cấu trúc schema + đối chiếu Knowledge Bank cho file Compose (`{"filePath": "/path/to/docker-compose.yml"}`).
- `POST /api/compose/autofix/propose`: Sinh đề xuất bản vá AI Autofix kèm giải thích (`{"filePath": "..."}`).
- `POST /api/compose/autofix/apply`: Áp dụng bản vá Compose an toàn có kiểm soát xóa khóa (`{"filePath": "...", "newContent": "...", "explanation": "...", "declaredRemovals": [...]}`).
- `POST /api/compose/autofix/undo`: Hoàn tác bản vá Autofix theo `{"fixId": "..."}`.
- `GET /api/compose/autofix/history`: Xem lịch sử các lần áp dụng/hoàn tác Autofix.

## 5. Bảo mật Container, Trivy, SBOM, Falco & Detonation
- **Image Scanning, SBOM & Policy (`Trivy`)**:
  - `POST /api/security/scan`, `POST /api/security/scan/cancel`: Quét lỗ hổng CVE trên image (`{"scanId": "...", "imageRef": "...", "refresh": false}`).
  - `POST /api/security/sbom`: Xuất SBOM cho image.
  - `POST /api/security/audit`: Quét kết hợp đánh giá bộ quy tắc (L1/L2) và chấm điểm bảo mật có truy xuất nguồn gốc.
  - `POST /api/security/triage`: Xếp hạng ưu tiên các vi phạm bảo mật từ kết quả audit.
  - `POST /api/security/autofix/propose`: Đề xuất hướng khắc phục cấu hình / nâng cấp image.
  - `GET /api/security/history`: Lịch sử các lần quét bảo mật.
  - `GET /api/security/policy`, `POST /api/security/policy`, `POST /api/security/policy/delete`: Quản lý chính sách bảo mật.
  - `GET /api/security/watch`, `POST /api/security/watch`: Xem/bật chế độ theo dõi quét tự động.
  - `GET /api/security/rules`, `GET /api/security/alternatives`: Lấy rule pack cấu hình và danh mục base image thay thế an toàn hơn.
- **Falco Runtime Security**:
  - `GET /api/security/falco/state`, `GET /api/security/falco/events`: Kiểm tra trạng thái Falco và đọc sự kiện runtime bất thường.
  - `GET /api/security/falco/watch`, `POST /api/security/falco/watch`: Cấu hình theo dõi Falco runtime.
  - `POST /api/security/falco/explain`: Giải thích sự kiện Falco bằng AI.
- **Container Detonation Sandbox**:
  - `POST /api/detonation/start`, `POST /api/detonation/cancel`: Khởi chạy / hủy phiên kích nổ kiểm thử hành vi image cô lập (timeline phát qua SSE).
  - `POST /api/detonation/session`, `GET /api/detonation/list`, `POST /api/detonation/export`: Truy vấn chi tiết, liệt kê danh sách và xuất báo cáo phiên Detonation.

## 6. Observability (Activity, Metrics, Alerts & Self-Heal)
- **Activity**:
  - `GET /api/activity`, `GET /api/activity/feed`, `POST /api/activity/export`: Truy vấn nhật ký hoạt động, luồng sự kiện hợp nhất và xuất báo cáo.
- **Metrics**:
  - `POST /api/metrics/interval`: Điều chỉnh chu kỳ lấy mẫu (`{"ms": 2000}`, mẫu phát qua SSE topic `metrics.sample`).
  - `GET /api/metrics/history`, `GET /api/metrics/health`: Lịch sử chuỗi thời gian và sức khỏe bộ thu thập metrics.
- **Alerts**:
  - `GET /api/alerts/rules`, `POST /api/alerts/rules`, `POST /api/alerts/rules/delete`: Quản lý quy tắc cảnh báo ngưỡng tài nguyên.
  - `GET /api/alerts/events`, `POST /api/alerts/backtest`: Xem cảnh báo đã kích hoạt và kiểm thử quy tắc trên dữ liệu lịch sử.
- **Self-Heal**:
  - `GET /api/self-heal/rules`, `POST /api/self-heal/rules`: Quản lý quy tắc tự phục hồi container.
  - `GET /api/self-heal/log`: Nhật ký các hành động tự phục hồi.
  - `GET /api/self-heal/enabled`, `POST /api/self-heal/enabled`: Kiểm tra / bật tắt cơ chế tự phục hồi toàn cục.

## 7. Kubernetes & Kind API
- `GET /api/k8s/check`, `GET /api/k8s/cluster-health`: Kiểm tra kết nối `kubectl` và sức khỏe cụm.
- `GET /api/k8s/contexts`, `GET /api/k8s/contexts/current`, `POST /api/k8s/contexts/set`: Quản lý K8s contexts.
- `GET /api/k8s/namespaces`, `pods`, `services`, `deployments`, `nodes`, `nodes/json`, `events`, `events/json`, `crds`, `crds/resources`: Liệt kê tài nguyên cụm.
- `GET /api/k8s/resources`, `GET /api/k8s/describe`, `GET /api/k8s/resources/yaml`: Truy vấn tài nguyên tổng quát, mô tả chi tiết và đọc manifest YAML.
- `POST /api/k8s/apply`: Triển khai manifest YAML.
- `POST /api/k8s/scale`, `POST /api/k8s/scale-generic`: Điều chỉnh số lượng replicas.
- `POST /api/k8s/pods/delete`, `POST /api/k8s/resources/delete`, `POST /api/k8s/resources/restart`, `POST /api/k8s/nodes/action`: Xóa pod/tài nguyên, rolling restart và cordon/drain node.
- `GET /api/k8s/pods/logs`, `GET /api/k8s/pods/containers`, `GET /api/k8s/pods/container-logs`: Đọc logs pod/container.
- `GET /api/k8s/pods/logs/stream?token=<TOKEN>&namespace=...&pod=...&tail=100`: Stream logs pod trực tiếp qua SSE.
- `POST /api/k8s/port-forward/start`, `POST /api/k8s/port-forward/stop`, `GET /api/k8s/port-forward/list`: Quản lý port-forwarding.
- `POST /api/k8s/exec`, `POST /api/k8s/benchmark`: Mở phiên lệnh vào pod và chạy benchmark HTTP.
- `GET /api/kind`, `POST /api/kind/create`, `POST /api/kind/delete`: Quản lý cụm Kind (Kubernetes in Docker).

## 8. Colima Instances, Presets, Colima Config (`colima.yaml`) & Lima VMs
- **Colima Instances & Worker Nodes**:
  - `GET /api/instances`, `GET /api/instances/status` (`?profile=default`), `GET /api/instances/ssh`: Tra cứu máy ảo Colima.
  - `POST /api/instances/start`, `stop`, `delete`, `k8s`: Khởi động, dừng, xóa instance hoặc thao tác K3s trên instance.
  - `POST /api/instances/create-worker`: Tạo và ghép nối node K3s worker Colima vào cụm master (`{"master_profile": "...", "worker_profile": "...", "cpu": 2, "memory": 4}`).
- **Custom Presets & Container Snapshots**:
  - `GET /api/presets`, `POST /api/presets`, `POST /api/presets/delete`: Đọc, lưu và xóa cấu hình phần cứng máy ảo tùy chỉnh (SQLite `user_presets`).
  - `GET /api/presets/snapshot` (`?preset_id=...&instance_profile=...`), `POST /api/presets/snapshot`, `GET /api/presets/snapshots` (`?profile=...`): Quản lý snapshot trạng thái containers gắn với từng preset/profile.
- **Colima Config (`colima.yaml`)**:
  - `GET /api/instances/config`: Đọc cấu hình `colima.yaml` hiện hành của profile.
  - `POST /api/instances/config/preview`: Xem trước bản so sánh thay đổi (`diff`) cấu hình `colima.yaml`.
  - `POST /api/instances/config/apply`: Lưu và áp dụng cấu hình `colima.yaml` an toàn.
- **Lima VMs**:
  - `GET /api/lima`, `GET /api/lima/info`, `GET /api/lima/templates`: Danh sách máy ảo, thông tin Lima và kho template.
  - `POST /api/lima/create`, `start`, `stop`, `delete`, `shell`: Quản lý vòng đời và chạy lệnh one-shot trong Lima VM.

## 9. System, Reference, Diagnostics, Knowledge Bank & Sandbox
- **System & Reference**:
  - `GET /api/system/check`, `version`, `homebrew`, `check-tool` (`?name=...`), `platform`, `host-specs`, `runtime`, `engine-resources`, `df`: Thông tin phần cứng, runtime đang hoạt động (`docker`/`nerdctl`), công cụ và dung lượng hệ thống.
  - `GET /api/system/capabilities`: Thăm dò trạng thái các công cụ dòng lệnh trên máy chủ (`colima`, `docker`, `kubectl`, `trivy`...) kèm gợi ý cài đặt (`install_hint`) — phân biệt với `GET /api/capabilities` dành cho AI Agent schema.
  - `GET /api/system/reference`: Đọc tài liệu tham chiếu nội bộ trong `resources/` (`?path=skills/docker/SKILL.md`, có bảo vệ chống path traversal).
  - `POST /api/system/install`, `POST /api/system/prune`: Cài đặt công cụ phụ thuộc và dọn dẹp toàn hệ thống.
  - `POST /api/system/resource-saver`: Bật/tắt chế độ tiết kiệm tài nguyên tự động dừng VM nhàn rỗi (`{"enabled": true, "idle_minutes": 15}`).
  - `GET /api/system/autostart`, `POST /api/system/autostart`: Trạng thái và cấu hình tự khởi động cùng macOS.
- **Diagnostics & Environment**:
  - `GET /api/diagnostics/logs`: Thu thập nhanh nhật ký chẩn đoán hệ thống.
  - `POST /api/diagnostics/bundle`, `POST /api/diagnostics/save`: Tạo và xuất gói chẩn đoán lỗi (đã tự động che giấu thông tin nhạy cảm).
  - `GET /api/environment/resolutions`: Xem bản ghi nguồn gốc phân giải đường dẫn (`colima-home`, `lima-home`, `docker-socket`, `kubeconfig`) và đối chiếu biến môi trường giữa `launchd` với shell người dùng.
- **Knowledge Bank & Help Articles**:
  - `GET /api/kb/articles`, `GET /api/kb/articles/get`, `GET /api/kb/articles/search`: Duyệt và tìm kiếm bài viết hướng dẫn tích hợp.
  - `POST /api/kb/query`, `feedback`, `learn`, `search`: Truy vấn mẫu lỗi, đánh giá giải pháp và ghi nhận tri thức mới.
  - `GET /api/kb/memories`, `POST /api/kb/memories/add`, `update`, `delete`: Quản lý bộ nhớ dài hạn (LTM) của AI.
- **Shell Sandbox**:
  - `POST /api/sandbox/execute`: Kiểm tra chính sách sandbox và thực thi ngay nếu lệnh thuộc nhóm an toàn (`{"command": "docker ps"}`).
  - `POST /api/sandbox/execute-approved`: Thực thi lệnh đã được người dùng/orchestrator phê duyệt tường minh (`{"command": "..."}`).

*Ghi chú: Để đối chiếu các sự kiện Event Bus tương ứng của AI Agent, hãy xem thêm `capabilities.md`.*
