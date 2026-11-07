---
name: colima-ui
description: Trợ lý AI điều khiển toàn diện hệ thống Container, Kubernetes và máy ảo thông qua ColimaUI. Giao tiếp qua Native MCP Server (cui mcp), Headless AI CLI (cui), hoặc gọi trực tiếp qua REST API.
---

# ColimaUI Orchestration Skill

Bạn là một Orchestrator đang điều khiển **ColimaUI** — nền tảng AI thống nhất quản lý Docker, Docker Compose, Kubernetes (K3s/Kind), Lima VMs và bảo mật runtime trên macOS.

## Xác thực & Kết nối (Authentication)

ColimaUI tự động chọn cổng trống đầu tiên trong dải `11420–11429` trên `127.0.0.1` và lưu thông tin phiên làm việc tại thư mục `~/.colima-ui/` với quyền `0600` (chỉ chủ sở hữu mới đọc được):

```bash
PORT=11420
[ -f ~/.colima-ui/api_port ] && read -r PORT < ~/.colima-ui/api_port
read -r TOKEN < ~/.colima-ui/api_token
```

- **Lưu ý bảo mật quan trọng**: Endpoint cấp token qua HTTP đã bị loại bỏ hoàn toàn vì lý do bảo mật để ngăn các tiến trình cục bộ trái phép đọc trộm token.
- **Public route duy nhất**: Chỉ `GET /api/health` không yêu cầu xác thực (dùng để kiểm tra trạng thái sống của server).
- **REST API**: Mọi request khác bắt buộc gửi kèm header `Authorization: Bearer ${TOKEN}` (và `Content-Type: application/json` khi gửi body JSON).
- **Luồng SSE thời gian thực**: Riêng 2 endpoint SSE là `GET /api/events` và `GET /api/k8s/pods/logs/stream` xác thực qua query param `?token=${TOKEN}` (do `EventSource` không gắn được custom header).

## Cách thức tương tác

ColimaUI cung cấp 3 chế độ điều khiển:

1. **Native MCP Server (`cui mcp` / `colimaui mcp` — Khuyên dùng cho IDE & Agent Hosts)**
   Máy chủ JSON-RPC 2.0 MCP tích hợp sẵn qua `stdin`/`stdout`, cung cấp **15 công cụ định kiểu mạnh**: `colima_list_instances`, `colima_instance_action`, `docker_list_containers`, `docker_container_action`, `docker_container_logs`, `docker_list_images`, `docker_pull_image`, `docker_list_volumes`, `docker_list_networks`, `compose_list_projects`, `k8s_list_pods`, `k8s_cluster_status`, `environment_resolutions`, `system_diagnostics`, `colima_ai_ask`. Hoạt động liền mạch ngay cả khi **ColimaUI Desktop đang chạy** (proxy qua `127.0.0.1:<api_port>` + `~/.colima-ui/api_token`) **hoặc đang đóng** (tự động fallback chạy trực tiếp native).
   - **Claude Desktop**, **Cursor**, **Antigravity**, **Windsurf**:
     ```json
     {"mcpServers": {"colima-ui": {"command": "cui", "args": ["mcp"]}}}
     ```
   - **Zed** (`~/.config/zed/settings.json`):
     ```json
     {"context_servers": {"colima-ui": {"command": {"path": "cui", "args": ["mcp"]}}}}
     ```
   - **Claude Code**: `claude mcp add colima-ui -- cui mcp`

2. **Giao tiếp qua Headless AI CLI (`cui`) & AI Chat (`POST /api/cli/chat`)**
   Gửi yêu cầu bằng ngôn ngữ tự nhiên cho vòng lặp AI đa bước của ColimaUI (tự động đọc `app_settings` từ `~/.colima-ui/knowledge.db`):
   ```bash
   # Chạy từ CLI (mặc định chỉ đọc; thêm --yes cho hành động thay đổi trạng thái, --json để lấy JSON)
   cui "Hãy kiểm tra trạng thái Colima và liệt kê các container đang bị exited"
   cui --yes --provider anthropic --model claude-sonnet-4-6 "Khởi động lại container redis"

   # Hoặc gọi trực tiếp qua HTTP POST /api/cli/chat
   curl -s -X POST "http://127.0.0.1:${PORT}/api/cli/chat" \
     -H "Authorization: Bearer ${TOKEN}" \
     -H "Content-Type: application/json" \
     -d '{"prompt": "Hãy kiểm tra trạng thái Colima và liệt kê các container đang bị exited."}'
   ```

3. **Gọi API Trực tiếp (Direct Control Mode)**
   Khi bạn đã biết chính xác hành động cần thực hiện, hãy gọi thẳng REST API của ColimaUI (hoặc CLI trực tiếp `colimaui`) để đạt tốc độ phản hồi tức thì:
   ```bash
   curl -s -H "Authorization: Bearer ${TOKEN}" "http://127.0.0.1:${PORT}/api/containers"
   ```
   - Xem chi tiết toàn bộ danh mục endpoint tại `references/api_reference.md`.

## Tính năng tự nhận thức (Meta-Capabilities)

ColimaUI có khả năng tự đọc và thay đổi cấu hình bộ não AI của chính nó qua **6 chế độ `ai_provider` chuẩn** (`anthropic`, `openai`, `gemini`, `openai-compatible` cho OpenRouter/Groq/DeepSeek/Mistral/Together/LiteLLM/vLLM, `ollama-local`, `ollama-cloud`):
- Cập nhật trực tiếp qua REST API:
  ```bash
  curl -s -X POST "http://127.0.0.1:${PORT}/api/settings" \
    -H "Authorization: Bearer ${TOKEN}" \
    -H "Content-Type: application/json" \
    -d '{"key": "ai_model", "value": "claude-sonnet-4-6"}'
  ```
- Hoặc ra lệnh qua AI Chat:
  ```bash
  curl -s -X POST "http://127.0.0.1:${PORT}/api/cli/chat" \
    -H "Authorization: Bearer ${TOKEN}" \
    -H "Content-Type: application/json" \
    -d '{"prompt": "Hãy chuyển sang dùng model gpt-4o của openai.", "allow_mutations": true}'
  ```

Hãy đọc thêm danh sách đầy đủ **100 sự kiện Event Bus** và **6 `ai_provider` hợp lệ** tại `references/capabilities.md`.

## Anti-Patterns

- **Không bao giờ gọi API lấy token qua HTTP**: Không tồn tại endpoint cấp token công khai; luôn đọc trực tiếp từ `~/.colima-ui/api_token` (`0600`).
- **Không dùng `Authorization` header cho SSE**: Với `/api/events` và `/api/k8s/pods/logs/stream`, bắt buộc truyền `?token=${TOKEN}` trên query string.
- **Không bịa đặt tên sự kiện hoặc endpoint nằm ngoài danh mục**: Luôn đối chiếu `references/api_reference.md` và `references/capabilities.md` trước khi gọi.
