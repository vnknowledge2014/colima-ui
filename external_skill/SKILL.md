---
name: colima-ui
description: Trợ lý AI điều khiển toàn diện hệ thống Container, Kubernetes và máy ảo thông qua ColimaUI. Giao tiếp qua Headless CLI Chat hoặc gọi trực tiếp qua REST API.
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

ColimaUI cung cấp 2 chế độ điều khiển:

1. **Giao tiếp qua AI Chat (Delegation Mode)**
   Thay vì tự bạn (Orchestrator) phải phân tích từng bước, bạn có thể gửi yêu cầu bằng ngôn ngữ tự nhiên (Prompt) cho AI của ColimaUI qua `POST /api/cli/chat`. AI nội bộ sẽ tự thực thi chuỗi công cụ và trả về `{"result": "..."}`.
   ```bash
   PORT=11420
   [ -f ~/.colima-ui/api_port ] && read -r PORT < ~/.colima-ui/api_port
   read -r TOKEN < ~/.colima-ui/api_token

   curl -s -X POST "http://127.0.0.1:${PORT}/api/cli/chat" \
     -H "Authorization: Bearer ${TOKEN}" \
     -H "Content-Type: application/json" \
     -d '{"prompt": "Hãy kiểm tra trạng thái Colima và liệt kê các container đang bị exited."}'
   ```
   - **Use Case**: Tác vụ đa bước, cần chẩn đoán ngữ cảnh hệ thống hoặc muốn ủy quyền trọn gói cho ColimaUI AI.

2. **Gọi API Trực tiếp (Direct Control Mode)**
   Khi bạn đã biết chính xác hành động cần thực hiện, hãy gọi thẳng REST API của ColimaUI để đạt tốc độ phản hồi tức thì (không tốn vòng lặp suy luận LLM).
   ```bash
   curl -s -H "Authorization: Bearer ${TOKEN}" "http://127.0.0.1:${PORT}/api/containers"
   ```
   - Xem chi tiết toàn bộ danh mục endpoint tại `references/api_reference.md`.

## Tính năng tự nhận thức (Meta-Capabilities)

ColimaUI có khả năng tự đọc và thay đổi cấu hình bộ não AI của chính nó (ví dụ chuyển đổi giữa Anthropic, OpenAI, Google Gemini, Ollama, OpenRouter, DeepSeek...):
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
    -d '{"prompt": "Hãy chuyển sang dùng model gpt-4o của openai."}'
  ```

Hãy đọc thêm danh sách đầy đủ năng lực và các `ai_provider` hỗ trợ tại `references/capabilities.md`.

## Anti-Patterns

- **Không bao giờ gọi API lấy token qua HTTP**: Không tồn tại endpoint cấp token công khai; luôn đọc trực tiếp từ `~/.colima-ui/api_token` (`0600`).
- **Không dùng `Authorization` header cho SSE**: Với `/api/events` và `/api/k8s/pods/logs/stream`, bắt buộc truyền `?token=${TOKEN}` trên query string.
- **Không bịa đặt tên sự kiện hoặc endpoint nằm ngoài danh mục**: Luôn đối chiếu `references/api_reference.md` và `references/capabilities.md` trước khi gọi.
