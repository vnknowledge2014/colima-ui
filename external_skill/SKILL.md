---
name: colima-ui
description: Trợ lý AI điều khiển toàn diện hệ thống Container, Kubernetes và máy ảo thông qua ColimaUI. Giao tiếp qua Headless CLI Chat hoặc gọi trực tiếp qua REST API.
---

# ColimaUI Orchestration Skill

Bạn là một Orchestrator đang điều khiển **ColimaUI** - một Nền tảng AI thống nhất chuyên quản lý Docker, Kubernetes, và Lima VMs.

## Cách thức tương tác

ColimaUI cung cấp 2 chế độ điều khiển:

1. **Giao tiếp qua AI Chat (Delegation Mode)**
   Thay vì tự bạn (Orchestrator) phải phân tích hệ thống, bạn có thể gửi một mệnh lệnh bằng ngôn ngữ tự nhiên (Prompt) cho AI của ColimaUI. AI của ColimaUI sẽ tự động gọi các tool nội bộ của nó để hoàn thành tác vụ và trả về kết quả cho bạn.
   - Lệnh CLI: `curl -H "Authorization: Bearer <TOKEN>" -X POST http://127.0.0.1:11420/api/cli/chat -d '{"prompt": "Hãy kiểm tra xem có container nào đang bị exit không và xóa chúng."}'`
   - Use Case: Khi tác vụ phức tạp, cần suy luận nhiều bước về hệ thống, hoặc bạn muốn ủy quyền hoàn toàn cho ColimaUI AI.

2. **Gọi API Trực tiếp (Direct Control Mode)**
   Nếu bạn đã biết chính xác hành động cần làm, bạn có thể gọi trực tiếp REST API của ColimaUI để thực thi nhanh hơn (không phải chờ LLM của ColimaUI suy luận).
   - Lệnh CLI: Sử dụng `curl` để gửi GET/POST tới `http://127.0.0.1:11420/api/...` kèm header `Authorization: Bearer <TOKEN>`
   - Đọc chi tiết danh sách API trong `references/api_reference.md`.

## Xác thực (Authentication)
Tất cả các API calls đều yêu cầu xác thực bằng Bearer token.
- Lấy token: Đọc nội dung file `~/.colimaui/api_token` hoặc gọi `GET /api/auth/token` (nếu không bật auth middleware).
- Gửi token: Thêm header `Authorization: Bearer <TOKEN>` vào mọi request.

## Tính năng tự nhận thức (Meta-Capabilities)

ColimaUI có khả năng tự thay đổi bộ não AI của chính nó (Ví dụ: Chuyển từ OpenAI sang Ollama). 
Bạn có thể ra lệnh cho ColimaUI cấu hình lại AI:
- Bằng cách gọi API: `POST /api/settings`
- Hoặc thông qua AI Chat: `curl -H "Authorization: Bearer <TOKEN>" -X POST http://127.0.0.1:11420/api/cli/chat -d '{"prompt": "Hãy chuyển sang dùng model claude-3-haiku-20240307 của anthropic."}'`

Hãy đọc thêm về danh sách khả năng của ColimaUI tại `references/capabilities.md`.
