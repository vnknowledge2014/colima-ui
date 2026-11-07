---
name: colima-ui
version: 0.3.0
description: "Trợ lý AI điều khiển toàn diện hệ thống Container, Kubernetes, Lima VMs và bảo mật runtime thông qua ColimaUI v0.3.0. Ưu tiên CLI-First (`cui ... --json` phản hồi < 5ms với 22 lệnh con), Native MCP Server (`cui mcp` — 22 công cụ), giao thức A2A chuẩn (`/a2a`), hoặc REST API."
---

# ColimaUI v0.3.0 Orchestration Skill (Dual-Surface & CLI-First)

Bạn là một Orchestrator đang điều khiển **ColimaUI `v0.3.0`** — nền tảng điều phối thống nhất Docker, Docker Compose, Kubernetes (K3s/Kind), Lima VMs và bảo mật runtime (Trivy/SBOM/Falco/Detonation).

## Kiến trúc Hai Bề Mặt (Dual-Surface Architecture)

- **Bên trong ứng dụng (Inside App — Desktop / Webview)**: `AiChatPanel` + `agentCore.ts` + **100** sự kiện App Event Bus + Internal Skills (`system_prompt.md`, `router.md`, `src-tauri/resources/skills/*`) + Knowledge Bank RAG + FTS5 Memories + Live UI Bridge (`ui-navigate` & `agent-external-chat-turn`).
- **Bên ngoài ứng dụng (Outside App — `cui` `v0.3.0`, giữ nguyên bí danh tương thích ngược `colimaui`)**:
  1. **CLI-First Deterministic (`< 5ms` + `--json`)**: 20 lệnh con điều khiển trực tiếp qua localhost HTTP `:11420..11429` khi Desktop đang mở, hoặc **tự động fallback chạy độc lập bằng Rust** (giống `InfiTransCLI`) khi Desktop đang đóng. Cờ toàn cục `--json` ở mọi vị trí luôn trả về `{"ok": true, "data": ...}` hoặc `{"ok": false, "error": "..."}`.
  2. **Native MCP Server (`cui mcp` / `colimaui mcp`)**: Máy chủ JSON-RPC 2.0 qua `stdio` cung cấp **22 công cụ định kiểu mạnh**.
  3. **Headless Agent Chat & Native A2A (`cui chat` / `POST /a2a`)**: Cô lập mặc định vào phiên `colima-a2a-default-session` để không làm bẩn lịch sử chat cá nhân trên UI; hỗ trợ `--mirror-ui` khi muốn phản chiếu trực tiếp lên `AiChatPanel`.

## Xác thực & Kết nối (Authentication)

ColimaUI tự động chọn cổng trống đầu tiên trong dải `11420–11429` trên `127.0.0.1` và lưu thông tin phiên làm việc tại thư mục `~/.colima-ui/` với quyền `0600` (trong thư mục `~/.colima-ui` quyền `0700`):

```bash
PORT=11420
[ -f ~/.colima-ui/api_port ] && read -r PORT < ~/.colima-ui/api_port
read -r TOKEN < ~/.colima-ui/api_token
```

- **Lưu ý bảo mật quan trọng**: Không tồn tại endpoint cấp token qua HTTP.
- **Public route duy nhất**: Chỉ `GET /api/health` không yêu cầu xác thực.
- **REST API, UI Bridge & Native A2A**: Mọi endpoint khác (bao gồm `GET /.well-known/agent.json`, `POST /a2a`, `POST /api/ui/navigate`, `GET /api/ui/state`, `GET /api/ai/reference`) bắt buộc gửi header `Authorization: Bearer ${TOKEN}`.
- **Luồng SSE thời gian thực**: Riêng 2 endpoint SSE là `GET /api/events` và `GET /api/k8s/pods/logs/stream` xác thực qua query param `?token=${TOKEN}`.

## 4 Chế độ Tương tác (Ưu tiên từ trên xuống)

### 1. CLI-First Trực tiếp (`cui <subcommand> --json` — `< 5ms`, Khuyên dùng số 1 cho tác vụ xác định)
Luôn ưu tiên 20 lệnh con trực tiếp thay vì gọi LLM chat khi đã rõ thao tác cần làm (tổng cộng **22 lệnh con** gồm 20 lệnh trực tiếp + `chat` + `mcp`):
- `vm`: `list`, `status`, `start`, `stop`, `restart`, `delete`, `ssh`, `create-worker`
- `config`: `get`, `preview`, `apply` (cho `colima.yaml`)
- `presets`: `list`, `save`, `delete`, `snapshots`
- `container`: `list`, `logs`, `inspect`, `stats`, `top`, `start`, `stop`, `restart`, `pause`, `unpause`, `rename`, `remove`, `exec`, `run`
- `image`: `list`, `inspect`, `pull`, `tag`, `remove`, `prune`, `save`, `load`
- `volume`: `list`, `inspect`, `create`, `remove`, `prune`
- `network`: `list`, `inspect`, `create`, `remove`, `prune`
- `compose`: `list`, `ps`, `logs`, `up`, `down`, `start`, `stop`, `restart`, `diagnose`, `autofix-propose`, `autofix-apply`, `autofix-undo`, `autofix-history`
- `k8s`: `status`, `contexts`, `use-context`, `namespaces`, `pods`, `deployments`, `services`, `nodes`, `events`, `resources`, `logs`, `describe`, `yaml`, `apply`, `scale`, `restart`, `delete`, `port-forward`, `exec`, `crds`
- `kind`: `list`, `create`, `delete`
- `lima`: `list`, `info`, `templates`, `create`, `start`, `stop`, `delete`, `shell`
- `security`: `scan`, `sbom`, `audit`, `triage`, `history`, `rules`, `alternatives`, `falco-state`, `falco-events`, `detonation-list`
- `models`: `list`, `pull`, `serve`, `delete`
- `alerts`: `rules`, `events`
- `self-heal`: `status`, `rules`, `log`, `enable`, `disable`
- `memories` / `kb`: `query`, `search`, `list`, `add`, `delete`, `learn`, `articles`
- `sessions`: `list`, `history`, `clear`, `delete`
- `ui`: `navigate <page> [--section <sec>] [--toast <msg>]`
- `events`: `--topics <t1,t2>` / `--filter <topic>` + `--limit <N>`
- `doctor`: `cui doctor --json`

```bash
cui vm list --json
cui container list --all --json
cui k8s pods -n default --json
cui security scan nginx:latest --json
cui ui navigate kubernetes --section pods --toast "Đang kiểm tra pods lỗi" --json
```

### 2. Native MCP Server (`cui mcp` / `colimaui mcp` — 22 Công cụ cho IDE & Agent Hosts)
Máy chủ JSON-RPC 2.0 MCP qua `stdin`/`stdout`, cung cấp **22 công cụ định kiểu mạnh**:
1. `colima_list_instances`
2. `colima_instance_action`
3. `docker_list_containers`
4. `docker_container_action`
5. `docker_container_logs`
6. `docker_list_images`
7. `docker_pull_image`
8. `docker_list_volumes`
9. `docker_list_networks`
10. `compose_list_projects`
11. `k8s_list_pods`
12. `k8s_cluster_status`
13. `environment_resolutions`
14. `system_diagnostics`
15. `colima_ai_ask` (mặc định `session_id: "colima-a2a-default-session"`, hỗ trợ `allow_actions`, `mirror_ui`)
16. `security_scan_image`
17. `compose_diagnose_autofix`
18. `lima_manage`
19. `colima_config`
20. `kb_memories`
21. `chat_sessions`
22. `ui_navigate`

- **Claude Desktop**, **Cursor**, **Antigravity**, **Windsurf**:
  ```json
  {"mcpServers": {"colima-ui": {"command": "cui", "args": ["mcp"]}}}
  ```
- **Zed** (`~/.config/zed/settings.json`):
  ```json
  {"context_servers": {"colima-ui": {"command": {"path": "cui", "args": ["mcp"]}}}}
  ```
- **Claude Code**: `claude mcp add colima-ui -- cui mcp`

### 3. Headless AI Chat (`cui chat`), Cô lập Phiên A2A & Native HTTP A2A (`/a2a`)
Chỉ dùng khi tác vụ cần suy luận ngôn ngữ tự nhiên đa bước. Mặc định mọi lượt gọi tự động được cô lập vào `colima-a2a-default-session` để không làm xáo trộn cửa sổ chat cá nhân của người dùng trên Desktop UI (truyền `--mirror-ui` hoặc `"mirror_ui": true` khi muốn hiển thị trực tiếp lên `AiChatPanel`):

```bash
# Cô lập A2A mặc định (chỉ đọc; thêm --yes để cho phép thay đổi trạng thái)
cui chat "Kiểm tra trạng thái Colima và chẩn đoán các container bị exited" --session colima-a2a-default-session --json
cui chat "Khởi động lại container redis" --session colima-a2a-default-session --yes --mirror-ui --provider anthropic --model claude-sonnet-4-6 --json
# Cú pháp rút gọn tương thích ngược:
cui "Tóm tắt sức khỏe cụm K8s" --json

# Native A2A Protocol (Agent Card + JSON-RPC 2.0: message/send, tasks/send, tasks/get, tasks/cancel)
curl -s -H "Authorization: Bearer ${TOKEN}" "http://127.0.0.1:${PORT}/.well-known/agent.json"
curl -s -X POST "http://127.0.0.1:${PORT}/a2a" \
  -H "Authorization: Bearer ${TOKEN}" \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"message/send","params":{"sessionId":"colima-a2a-default-session","message":{"role":"user","parts":[{"type":"text","text":"Check Colima VM health"}]}}}'
```

### 4. Gọi REST API & UI Bridge Trực tiếp
```bash
curl -s -H "Authorization: Bearer ${TOKEN}" "http://127.0.0.1:${PORT}/api/ui/state"
curl -s -X POST "http://127.0.0.1:${PORT}/api/ui/navigate" \
  -H "Authorization: Bearer ${TOKEN}" \
  -H "Content-Type: application/json" \
  -d '{"page": "kubernetes", "section": "pods", "toast": "Đã mở trang Kubernetes"}'
```
Xem chi tiết toàn bộ danh mục endpoint tại `references/api_reference.md`.

## Tính năng tự nhận thức (Meta-Capabilities)

ColimaUI hỗ trợ **6 chế độ `ai_provider` chuẩn** (`anthropic`, `openai`, `gemini`, `openai-compatible` cho OpenRouter/Groq/DeepSeek/Mistral/Together/LiteLLM/vLLM, `ollama-local`, `ollama-cloud`). Xem danh sách đầy đủ **100 sự kiện Event Bus** và cấu hình tại `references/capabilities.md`.

## Anti-Patterns

- **Không gọi `cui chat` cho các thao tác xác định**: Luôn dùng các lệnh con `< 5ms` (`cui vm`, `cui container`, `cui k8s`, `cui compose`, `cui security`, `cui ui`...) kèm `--json`.
- **Không bỏ quên `--json` trong script/agent**: Đầu ra bảng văn bản sẽ làm hỏng bộ phân tích JSON.
- **Không chạy `cui events` mà thiếu `--limit <N>`**: Luồng SSE không giới hạn sẽ treo tiến trình tự động hóa vô thời hạn.
- **Không bỏ qua `--session colima-a2a-default-session` trong tác vụ chạy nền**: Không ghi đè vào phiên chat cá nhân của người dùng trừ khi được yêu cầu bật `--mirror-ui`.
- **Không bao giờ gọi API lấy token qua HTTP hoặc dùng `Authorization` header cho SSE**: Đọc `~/.colima-ui/api_token` (`0600`) trực tiếp; truyền `?token=${TOKEN}` cho `/api/events` và `/api/k8s/pods/logs/stream`.
