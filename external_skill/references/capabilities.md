# Danh sách Năng lực (Capabilities) của ColimaUI

Đây là cuốn "từ điển" liệt kê chính xác mọi hành động mà **ColimaUI AI Agent** và hệ thống REST/Event Bus của ColimaUI có thể hiểu và thực thi.
Khi phân tích yêu cầu của User, hãy tham chiếu danh sách này để biết ColimaUI có khả năng thực hiện hay không. **TUYỆT ĐỐI KHÔNG** bịa ra các hành động nằm ngoài danh sách dưới đây.

## Colima / Máy ảo (Instances & Config)
- `list-instances`: Lấy danh sách các máy ảo Colima hiện có.
- `create-instance`: Tạo và khởi động máy ảo Colima mới từ cấu hình phần cứng (`vz`/`qemu`, `virtiofs`/`sshfs`, `kubernetes`).
- `start-instance`: Khởi động máy ảo Colima đã bị dừng.
- `stop-instance`: Tắt/Dừng máy ảo Colima.
- `colima-restart`: Khởi động lại máy ảo Colima.
- `delete-instance`: Xóa vĩnh viễn máy ảo Colima (Nguy hiểm).
- `colima-status`: Lấy chi tiết trạng thái hoạt động (CPU, RAM, Disk, Runtime).
- Quản lý trực tiếp tệp cấu hình `colima.yaml` (đọc, xem trước diff và áp dụng qua `/api/instances/config*` hoặc tab `settings`).

## Docker Containers, Images, Transfers & Topology
- `list-containers`: Lấy danh sách toàn bộ container (đang chạy và đã dừng).
- `start-container`, `stop-container`, `restart-container`, `pause-container`, `unpause-container`, `rename-container`, `remove-container`: Quản lý vòng đời container.
- `container-logs`, `container-stats`, `all-container-stats`, `container-top`, `inspect-container`: Giám sát logs, tài nguyên và tiến trình.
- `container-exec`: Chạy một lệnh bên trong container.
- `run-container`: Chạy container mới từ Image.
- `list-images`, `inspect-image`, `pull-image`, `tag-image`, `remove-image`, `prune-images`: Quản lý Docker Images.
- **Background Transfers & Topology** (hỗ trợ sẵn qua UI và REST API):
  - Lưu/nạp ảnh `.tar` (`images/save`, `images/load`), sao chép tệp hai chiều giữa máy chủ và container (`containers/cp/to`, `containers/cp/from`), theo dõi/hủy tiến trình nền (`transfers`).
  - Xem đồ thị liên kết trực quan giữa Container – Network – Volume (`topology`).

## Docker Volumes & Networks
- `volume-list`, `volume-inspect`, `volume-create`, `volume-remove`, `volume-prune`: Quản lý Volume.
- `network-list`, `network-inspect`, `network-create`, `network-remove`, `network-prune`: Quản lý Network.

## Docker Compose, Diagnose & AI Autofix
- `compose-list`: Liệt kê các dự án Compose.
- `compose-up`, `compose-down`, `compose-restart`: Khởi chạy, dừng/gỡ bỏ và khởi động lại dự án Compose.
- `compose-logs`: Lấy logs của dự án Compose.
- **Compose Diagnose & AI Autofix** (`/api/compose/diagnose`, `/api/compose/autofix/*`): Kiểm tra hợp lệ schema + đối chiếu Knowledge Bank, sinh bản vá YAML kèm xem trước diff, áp dụng an toàn và hoàn tác (`undo`).

## Bảo mật Container, Runtime & Source Audit
- **Container & Runtime Security** (`/api/security/*`, `/api/detonation/*`):
  - Quét lỗ hổng image bằng Trivy kèm điểm bảo mật có truy xuất nguồn gốc, xuất SBOM, kiểm tra rule pack cấu hình (L1/L2), gợi ý base image an toàn thay thế.
  - Theo dõi sự kiện runtime từ Falco kèm giải thích bằng AI.
  - Chạy thử nghiệm cô lập trong **Detonation Sandbox** và xuất báo cáo hành vi.
- **4-Stage Source Security Pipeline**:
  - `[SECURITY_THREAT_MODEL]`, `[SECURITY_VULN_SCAN]`, `[SECURITY_TRIAGE]`, `[SECURITY_PATCH_GEN]` (chỉ đọc, xuất bản vá dạng diff để con người duyệt).

## Observability (Activity, Metrics, Alerts & Self-Heal)
- Theo dõi nhật ký hoạt động hợp nhất (`activity`), lịch sử metrics thời gian thực (`metrics`), quy tắc cảnh báo ngưỡng & kiểm thử ngược (`alerts`), và tự động phục hồi container sự cố (`self-heal`).

## Kubernetes (K8s / K3s / Kind)
- `k8s-list-contexts`, `k8s-current-context`, `k8s-set-context`: Quản lý ngữ cảnh cụm.
- `k8s-list-namespaces`, `k8s-list-resources`, `k8s-describe`, `k8s-yaml`: Tra cứu tài nguyên (Pods, Deployments, Services, StatefulSets...).
- `k8s-logs`, `k8s-container-logs`, `k8s-pod-containers`: Đọc logs của Pod và từng container.
- `k8s-apply`, `k8s-delete`, `k8s-restart-resource`: Triển khai YAML, xóa và rolling restart tài nguyên.
- `k8s-nodes`, `k8s-node-action`: Quản lý Node (Cordon/Uncordon/Drain).
- `k8s-crds`, `k8s-crd-resources`: Quản lý Custom Resource Definitions.
- `k8s-scale`, `k8s-generic-scale`: Tăng giảm số lượng Replicas.
- `k8s-port-forward-start`, `k8s-port-forward-stop`, `k8s-port-forward-list`: Chuyển tiếp cổng Local.
- `k8s-exec`: Mở phiên lệnh vào Pod.
- `k8s-cluster-health`, `k8s-benchmark`, `k8s-events`: Kiểm tra sức khỏe cụm, benchmark HTTP và đọc sự kiện.
- `kind-list`, `kind-create`, `kind-delete`: Quản lý cụm Kind (Kubernetes trong Docker).

## Hệ thống & Lima VMs
- `system-df`: Xem dung lượng đĩa.
- `system-host-specs`, `system-check`, `system-check-tool`: Kiểm tra tài nguyên phần cứng và công cụ trên máy chủ macOS.
- `system-prune`: Dọn dẹp rác toàn hệ thống (Containers, Networks, Images, Volumes).
- `cli-exec`: Thực thi lệnh CLI đơn lẻ (không tương tác TTY, không mở rộng `~`) qua App CLI gateway.
- `lima-list`, `lima-start`, `lima-stop`, `lima-delete`, `lima-create`, `lima-info`, `lima-templates`, `lima-shell`: Toàn quyền thao tác trên Lima VM.

## AI Models & Cấu hình Ứng dụng
- `model-list`, `model-pull`, `model-serve`, `model-delete`: Quản lý tải và chạy các mô hình AI Local (Llama, Qwen, Gemma, DeepSeek...).
- **Sử dụng khi:** Đọc và cập nhật cấu hình nội bộ (Tự nhận thức / Meta-Capabilities).

### Danh sách các cấu hình khả dụng (EXACT KEYS — DO NOT GUESS):
- `ai_provider`: Hỗ trợ đầy đủ **9 nhà cung cấp AI (`ai_provider`)**:
  1. `ollama` — Mô hình cục bộ hoặc Ollama từ xa (ví dụ: `qwen2.5-coder:7b`, `llama3.3`, `deepseek-r1:8b`, `gemma3`; trên UI cũng phân tách `ollama-local` / `ollama-cloud`)
  2. `openai` — OpenAI API (ví dụ: `gpt-4o`, `gpt-4o-mini`, `o3-mini`)
  3. `anthropic` — Anthropic Claude API (ví dụ: `claude-sonnet-4-6`, `claude-opus-4-6`, `claude-3-5-sonnet-latest`)
  4. `google` — Google Gemini API (ví dụ: `gemini-2.5-pro`, `gemini-2.5-flash`, `gemini-2.0-flash`; alias UI: `gemini`)
  5. `openrouter` — Cổng tổng hợp OpenRouter (ví dụ: `anthropic/claude-sonnet-4-6`, `openai/gpt-4o`, `deepseek/deepseek-r1`)
  6. `groq` — Suy luận tốc độ cao trên Groq LPU (ví dụ: `llama-3.3-70b-versatile`, `deepseek-r1-distill-llama-70b`)
  7. `together` — Together AI (ví dụ: `meta-llama/Llama-3.3-70B-Instruct-Turbo`, `Qwen/Qwen2.5-Coder-32B-Instruct`)
  8. `mistral` — Mistral AI (ví dụ: `mistral-large-latest`, `codestral-latest`, `mistral-small-latest`)
  9. `deepseek` — DeepSeek API (ví dụ: `deepseek-chat`, `deepseek-reasoner`)
  *(Lưu ý: các nhà cung cấp chuẩn OpenAI như `openrouter`, `groq`, `together`, `mistral`, `deepseek`, LiteLLM, vLLM cũng có thể cấu hình qua chế độ `openai-compatible` kèm `ai_endpoint` tương ứng).*
- `ai_model` (string — ví dụ: `claude-sonnet-4-6`, `gpt-4o`, `gpt-4o-mini`, `gemini-2.0-flash`, `deepseek-chat`, `qwen2.5-coder:7b`)
- `ai_api_key` (string)
- `ai_endpoint` (string — base URL cho Ollama từ xa hoặc các cổng tương thích OpenAI)
- `ai_searxng_instances` (JSON string array)
- `ai_diag_content_mode` (`full`, `summary`)
- `ai_diag_max_page_size` (string number, ví dụ: `"8000"`)
- `ai_diag_auto_trigger` (`"true"`, `"false"`)
- `colimaui_auto_pause` (`"true"`, `"false"`)
- `colimaui_auto_pause_mins` (string number, ví dụ: `"15"`)

#### `ai-config-status`
**Input:** `{}`
**Output:** JSON chứa toàn bộ key-value của cấu hình hiện tại.

#### `ai-update-config`
**Input:** `{"settings": {"colimaui_auto_pause": "true", "colimaui_auto_pause_mins": "15"}}` (Luôn luôn dùng kiểu String cho value)
**Output:** Confirmation. (Sau khi update xong, hãy nhắc User refresh UI).

- `update-setting`: Bật tắt giao diện Dark Mode, Auto-Pause, Idle Threshold.
- `list-presets`, `save-preset`, `delete-preset`: Xem, lưu và xóa các cấu hình phần cứng máy ảo Custom.
