# Danh sách Năng lực (Capabilities) của ColimaUI (`v0.3.3`)

Đây là cuốn "từ điển" liệt kê chính xác mọi hành động trên kiến trúc **Hai Bề Mặt (Dual-Surface)** của **ColimaUI `v0.3.3`**:
- **Bên trong ứng dụng (Inside App)**: `AiChatPanel` + `agentCore.ts` + **130** sự kiện App Event Bus đối xứng (`62 SAFE` chỉ đọc + `68 NORMAL/DANGEROUS` thay đổi trạng thái) + Internal Skills (`system_prompt.md`, `router.md`, `src-tauri/resources/skills/*`) + Knowledge Bank RAG + FTS5 Memories + Live UI Bridge (`ui-navigate` & `agent-external-chat-turn`).
- **Bên ngoài ứng dụng (Outside App)**: CLI-First `cui` (`v0.3.3`, bí danh `colimaui`) với **22 lệnh con** (`< 5ms --json` qua HTTP `:11420..11429` hoặc tự động fallback native Rust khi Desktop đóng), **22 công cụ MCP** (`cui mcp`), và giao thức **Native A2A** (`GET /.well-known/agent.json`, `POST /a2a` cô lập mặc định vào `colima-a2a-default-session`, chia sẻ đối xứng đủ **130** sự kiện App Event Bus).

Khi phân tích yêu cầu của User, hãy tham chiếu danh sách **130 sự kiện App Event Bus** dưới đây để biết ColimaUI có khả năng thực hiện hay không. **TUYỆT ĐỐI KHÔNG** bịa ra các hành động nằm ngoài danh sách dưới đây.

## Colima / Máy ảo (Instances & Config — 10 sự kiện)
- `list-instances` (`SAFE`): Lấy danh sách các máy ảo Colima hiện có.
- `colima-status` (`SAFE`): Lấy chi tiết trạng thái hoạt động (CPU, RAM, Disk, Runtime).
- `diagnostic-logs` (`SAFE`, `{"profile?"}`): Thu thập nhật ký chẩn đoán máy ảo Lima/Colima của profile.
- `colima-config-get` (`SAFE`, `{"profile?"}`): Đọc cấu hình `colima.yaml` hiện hành kèm `mtime`.
- `create-instance` (`NORMAL`): Tạo và khởi động máy ảo Colima mới từ cấu hình phần cứng (`vz`/`qemu`, `virtiofs`/`sshfs`, `kubernetes`).
- `start-instance` (`NORMAL`): Khởi động máy ảo Colima đã bị dừng.
- `stop-instance` (`NORMAL`): Tắt/Dừng máy ảo Colima.
- `colima-restart` (`NORMAL`): Khởi động lại máy ảo Colima.
- `colima-config-apply` (`NORMAL`, `{"profile?", "changes", "expectedMtime?"}`): Lưu và áp dụng thay đổi `colima.yaml` có khóa lạc quan (`expectedMtime`).
- `delete-instance` (`DANGEROUS`): Xóa vĩnh viễn máy ảo Colima (Nguy hiểm).

## Docker Containers, Images, Transfers & Topology (22 sự kiện Container/Image + `topology-get`)
- `list-containers`: Lấy danh sách toàn bộ container (đang chạy và đã dừng).
- `start-container`, `stop-container`, `restart-container`, `pause-container`, `unpause-container`, `rename-container`, `remove-container`: Quản lý vòng đời container.
- `container-logs`, `container-stats`, `all-container-stats`, `container-top`, `inspect-container`: Giám sát logs, tài nguyên và tiến trình.
- `container-exec`: Chạy một lệnh đơn lẻ (không tương tác) bên trong container. Để mở shell tương tác trực tiếp vào container (**Interactive Container PTY** — `dockerExec`), điều hướng tới `[NAVIGATE: containers]` hoặc `[NAVIGATE: terminal]`.
- `run-container`: Chạy container mới từ Image.
- `list-images`, `inspect-image`, `build-image` (`{"contextPath": ".", "dockerfile": "Dockerfile", "tag": "my-app:latest"}`), `pull-image`, `tag-image`, `remove-image`, `prune-images`: Quản lý và build Docker Images.
- `topology-get` (`SAFE`): Truy vấn trực tiếp đồ thị liên kết giữa Container – Network – Volume – Project – Image.
- **Background Transfers** (hỗ trợ sẵn qua UI, `cui image save/load` và REST API):
  - Lưu/nạp ảnh `.tar` (`images/save`, `images/load`), sao chép tệp hai chiều giữa máy chủ và container (`containers/cp/to`, `containers/cp/from`), theo dõi/hủy tiến trình nền (`transfers`).

## Docker Volumes & Networks (10 sự kiện)
- `volume-list`, `volume-inspect`, `volume-create`, `volume-remove`, `volume-prune`: Quản lý Volume.
- `network-list`, `network-inspect`, `network-create`, `network-remove`, `network-prune`: Quản lý Network.

## Docker Compose, Diagnose & AI Autofix (15 sự kiện)
- `compose-list` (`SAFE`), `compose-ps` (`SAFE`, `{"projectName"}`), `compose-logs` (`SAFE`, `{"projectName", "lines?"}`): Liệt kê dự án, trạng thái các service và đọc logs Compose.
- `compose-up` (`{"dir"}`), `compose-down` (`{"projectName"}`), `compose-start` (`{"projectName"}`), `compose-stop` (`{"projectName"}`), `compose-restart` (`{"projectName"}`): Khởi tạo, gỡ bỏ, bật/dừng từng dự án và khởi động lại Compose.
- `compose-build` (`{"projectDir": "...", "service"?: "..."}`), `compose-pull` (`{"projectDir": "...", "service"?: "..."}`): Build hoặc kéo mới image cho toàn bộ dự án hoặc từng service trong Compose.
- `compose-diagnose` (`SAFE`, `{"path"}`): Kiểm tra hợp lệ schema YAML + đối chiếu Knowledge Bank trực tiếp qua Event Bus.
- `compose-autofix-history` (`SAFE`), `compose-autofix-propose` (`NORMAL`, `{"path"}`), `compose-autofix-apply` (`NORMAL`, `{"path", "patchedContent", "expectedHash?"}`), `compose-autofix-undo` (`NORMAL`, `{"id"}`): Đề xuất bản vá AI Autofix, áp dụng an toàn có kiểm tra hash và hoàn tác (`undo`).

## Bảo mật Container, Runtime & Source Audit (12 sự kiện Event Bus + 4-Stage Pipeline)
- **Container & Runtime Security Event Bus** (`6 SAFE` + `6 NORMAL/DANGEROUS`):
  - `security-scan` (`NORMAL`, `{"image", "refresh?"}`), `security-audit` (`NORMAL`, `{"image", "level?", "refresh?"}`), `security-sbom` (`NORMAL`, `{"image", "format?"}`), `security-triage` (`NORMAL`, `{"image", "level?"}`): Quét CVE bằng Trivy, chấm điểm bảo mật `/100` có truy xuất nguồn gốc, xuất SBOM (CycloneDX/SPDX) và xếp hạng ưu tiên khắc phục.
  - `security-history` (`SAFE`, `{"digest?", "limit?"}`), `security-rules` (`SAFE`), `security-alternatives` (`SAFE`, `{"image"}`): Lịch sử quét theo digest, bộ quy tắc cấu hình (L1–L3) và danh mục base image an toàn thay thế.
  - `falco-state` (`SAFE`), `falco-events` (`SAFE`, `{"limit?"}`): Kiểm tra trạng thái Falco trong VM và đọc sự kiện runtime bất thường.
  - `detonation-list` (`SAFE`), `detonation-start` (`DANGEROUS`, `{"image", "durationSecs?", "command?"}`), `detonation-cancel` (`NORMAL`, `{"sessionId"}`): Liệt kê, khởi chạy và dừng phiên thử nghiệm cô lập trong **Detonation Sandbox**.
- **4-Stage Source Security Pipeline**:
  - `[SECURITY_THREAT_MODEL]`, `[SECURITY_VULN_SCAN]`, `[SECURITY_TRIAGE]`, `[SECURITY_PATCH_GEN]` (chỉ đọc, xuất bản vá dạng diff để con người duyệt).

## Observability (Activity, Alerts & Self-Heal — 5 sự kiện + `topology-get` = 6 sự kiện)
- `activity-feed` (`SAFE`, `{"limit?", "kind?"}`): Luồng nhật ký hoạt động hợp nhất (activity, transfers, alerts, scans, self-heal).
- `alerts-rules` (`SAFE`), `alerts-events` (`SAFE`, `{"limit?"}`): Danh sách quy tắc cảnh báo ngưỡng tài nguyên và các cảnh báo đã kích hoạt.
- `self-heal-rules` (`SAFE`), `self-heal-log` (`SAFE`, `{"limit?"}`): Danh sách quy tắc tự động phục hồi container sự cố và nhật ký hành động tự phục hồi.

## Kubernetes (K8s / K3s / Kind — 27 sự kiện K8s + 3 sự kiện Kind)
- `k8s-list-contexts`, `k8s-current-context`, `k8s-set-context`: Quản lý ngữ cảnh cụm.
- `k8s-list-namespaces`, `k8s-pods` (`SAFE`, `{"namespace?"}`), `k8s-list-resources`, `k8s-describe`, `k8s-yaml`: Tra cứu nhanh Pods và toàn bộ tài nguyên (Deployments, Services, StatefulSets...).
- `k8s-logs`, `k8s-container-logs`, `k8s-pod-containers`: Đọc logs của Pod và từng container.
- `k8s-apply`, `k8s-delete`, `k8s-restart-resource`: Triển khai YAML, xóa và rolling restart tài nguyên.
- `k8s-nodes`, `k8s-node-action`: Quản lý Node (Cordon/Uncordon/Drain).
- `k8s-crds`, `k8s-crd-resources`: Quản lý Custom Resource Definitions.
- `k8s-scale`, `k8s-generic-scale`: Tăng giảm số lượng Replicas.
- `k8s-port-forward-start`, `k8s-port-forward-stop`, `k8s-port-forward-list`: Chuyển tiếp cổng Local.
- `k8s-exec`: Mở phiên lệnh vào Pod.
- `k8s-cluster-health`, `k8s-benchmark`, `k8s-events`: Kiểm tra sức khỏe cụm, benchmark HTTP và đọc sự kiện.
- `kind-list`, `kind-create`, `kind-delete`: Quản lý cụm Kind (Kubernetes trong Docker).

## Hệ thống, Môi trường, Điều hướng UI & Lima VMs (6 sự kiện System + 2 Gateway + 8 Lima)
- `navigate` (`SAFE`, `{"page": "...", "section"?: "...", "toast"?: "..."}`): Điều hướng tới bất kỳ trang nào trong **17 trang giao diện chính** (`dashboard`, `instances`, `environment`, `containers`, `images`, `volumes`, `networks`, `compose`, `topology`, `activity`, `security`, `kubernetes`, `linux-vms`, `models`, `terminal`, `settings`, `help`) **cộng thêm** `ai-chat` (mở ngăn AI Command Center), hỗ trợ nhắm sâu tới `section` và hiển thị `toast`.
- `environment-resolutions`: Kiểm tra nguồn gốc phân giải đường dẫn (`$COLIMA_HOME` — XDG `~/.config/colima` trên Colima `0.10.x` so với `~/.colima` cũ, Lima `_lima/colima`, Docker socket, `KUBECONFIG`) và đối chiếu biến môi trường giữa `launchd` với shell người dùng.
- `system-df`: Xem dung lượng đĩa.
- `system-host-specs`, `system-check`, `system-check-tool`: Kiểm tra tài nguyên phần cứng và công cụ trên máy chủ macOS.
- `system-prune`: Dọn dẹp rác toàn hệ thống (Containers, Networks, Images, Volumes).
- `cli-exec`: Thực thi lệnh CLI đơn lẻ (không tương tác TTY, không mở rộng `~`) qua App CLI gateway.
- `lima-list`, `lima-start`, `lima-stop`, `lima-delete`, `lima-create`, `lima-info`, `lima-templates`, `lima-shell`: Toàn quyền thao tác trên Lima VM.

## AI Models & Cấu hình Ứng dụng
- `model-list`, `model-pull`, `model-serve`, `model-delete`: Quản lý tải và chạy các mô hình AI Local (Llama, Qwen, Gemma, DeepSeek...).
- **Sử dụng khi:** Đọc và cập nhật cấu hình nội bộ (Tự nhận thức / Meta-Capabilities).

### Danh sách các cấu hình khả dụng (EXACT KEYS — DO NOT GUESS):
- `ai_provider`: Chỉ chấp nhận **ĐÚNG 6 định danh `ai_provider` chuẩn** dưới đây (TUYỆT ĐỐI KHÔNG dùng các tên không hợp lệ như `ollama`, `google`, `openrouter`, `groq`, `together`, `mistral`, `deepseek`):
  1. `anthropic` — Anthropic Claude API (ví dụ: `claude-sonnet-4-6`, `claude-opus-4-6`, `claude-haiku-4-5`)
  2. `openai` — OpenAI API (ví dụ: `gpt-4o`, `gpt-4o-mini`, `o3-mini`)
  3. `gemini` — Google Gemini API (ví dụ: `gemini-2.5-pro`, `gemini-2.5-flash`, `gemini-2.0-flash`)
  4. `openai-compatible` — Mọi cổng proxy và nhà cung cấp tương thích chuẩn OpenAI như **OpenRouter** (`https://openrouter.ai/api/v1`), **Groq** (`https://api.groq.com/openai/v1`), **DeepSeek** (`https://api.deepseek.com/v1`), **Mistral** (`https://api.mistral.ai/v1`), **Together AI** (`https://api.together.xyz/v1`), **LiteLLM** (`http://localhost:4000/v1`), **9router**, **vLLM**, hoặc **LM Studio** — cấu hình kèm `ai_endpoint` + `ai_api_key` + `ai_model` tương ứng.
  5. `ollama-local` — Ollama daemon chạy cục bộ tại `http://localhost:11434` (ví dụ: `qwen2.5-coder:7b`, `llama3.3`, `deepseek-r1:8b`, `gemma3`)
  6. `ollama-cloud` — Máy chủ Ollama từ xa hoặc Ollama Cloud cấu hình qua `ai_endpoint` (và `ai_api_key` nếu có)
- `ai_model` (string — ví dụ: `claude-sonnet-4-6`, `gpt-4o`, `gemini-2.5-flash`, `deepseek-chat`, `qwen2.5-coder:7b`)
- `ai_api_key` (string)
- `ai_endpoint` (string — bắt buộc cho `openai-compatible` và `ollama-cloud`, ví dụ `https://openrouter.ai/api/v1` hoặc `https://api.deepseek.com/v1`)
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

- `list-presets`, `save-preset`, `delete-preset`: Xem, lưu và xóa các cấu hình phần cứng máy ảo Custom (lưu trữ bền vững trong SQLite `~/.colima-ui/knowledge.db`).

## Bề mặt Ngoài Ứng dụng: CLI-First (`cui` v0.3.3), 22 MCP Tools, UI Bridge & Native A2A
- **Live UI Bridge Events**:
  - `ui-navigate`: Chuyển trang/phân mục từ xa và hiển thị thông báo nhanh (`POST /api/ui/navigate` hoặc `cui ui navigate <page> [--section <sec>] [--toast <msg>] --json` hoặc MCP `ui_navigate`).
  - `agent-external-chat-turn`: Phản chiếu trực tiếp lượt hội thoại từ CLI/MCP/A2A bên ngoài vào `AiChatPanel` khi bật `--mirror-ui` / `"mirror_ui": true`.
- **Cô lập Phiên A2A (`colima-a2a-default-session`)**: Mọi tác vụ tự động qua `cui chat`, `POST /api/cli/chat`, MCP `colima_ai_ask`, và Native A2A `POST /a2a` (`message/send`, `tasks/send`, `tasks/get`, `tasks/cancel`) mặc định chạy trên phiên `colima-a2a-default-session` để không xâm phạm lịch sử hội thoại cá nhân của người dùng trên Desktop UI.
- **22 Lệnh con `cui` (`< 5ms --json` + Standalone Fallback, bí danh `colimaui`)**: `vm`, `config`, `presets`, `container`, `image`, `volume`, `network`, `compose`, `k8s`, `kind`, `lima`, `security`, `models`, `alerts`, `self-heal`, `memories` (`kb`), `sessions`, `ui`, `events`, `doctor`, `chat`, `mcp`.
- **22 Công cụ MCP (`cui mcp` / `colimaui mcp`)**: `colima_list_instances`, `colima_instance_action`, `docker_list_containers`, `docker_container_action`, `docker_container_logs`, `docker_list_images`, `docker_pull_image`, `docker_list_volumes`, `docker_list_networks`, `compose_list_projects`, `k8s_list_pods`, `k8s_cluster_status`, `environment_resolutions`, `system_diagnostics`, `colima_ai_ask`, `security_scan_image`, `compose_diagnose_autofix`, `lima_manage`, `colima_config`, `kb_memories`, `chat_sessions`, `ui_navigate`.
