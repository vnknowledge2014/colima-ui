# Phân xử red-team — 2 plan Free/Pro tier

Ngày: 2026-08-11 · 4 reviewer (Security Adversary, Failure Mode Analyst, Assumption Destroyer, Scope & Complexity Critic) · Full tier verification

**26 finding thô → 15 sau khử trùng lặp.** Tất cả có `file:line`; không finding nào bị loại ở bộ lọc bằng chứng. **Đề xuất: chấp nhận cả 15.**

Bốn reviewer độc lập hội tụ vào cùng một kết luận: **bản kiểm kê "đã có sẵn" trong cả hai plan là sai**. Đây là lỗi ở bước scout của tôi — tôi đọc tên file và chữ ký hàm rồi suy ra hành vi, thay vì đọc thân hàm.

---

## Critical (9)

### C1. Token API phát không, không xác thực → mọi route ghi mới thành đường leo thang đặc quyền
**Reviewer:** Security, Assumption Destroyer · **Ảnh hưởng:** Free P1, P2; Pro P1, P4
`GET /api/auth/token` nằm trong router **public** và trả thẳng bearer token (`src-tauri/src/api_server.rs:256`, `src-tauri/src/routes/system.rs:263-265`). CORS (`api_server.rs:53-58`) chỉ chặn browser, không chặn tiến trình local.
**Kịch bản:** tiến trình bất kỳ trên máy `GET /api/auth/token` → `POST /api/containers/cp` ghi file tuỳ ý lên host, hoặc `/api/compose/autofix/apply`, hoặc route self-heal.
**Sửa:** hardening endpoint này là **điều kiện tiên quyết**, không phải việc trong plan. Không thêm route ghi/thực thi nào trước khi vá.

### C2. Gate backend là blob client gửi lên, không ký
**Reviewer:** Security, Assumption Destroyer · **Ảnh hưởng:** Pro plan.md "Quy ước gating", Pro P2
`EntitlementPayload` là `Deserialize` thuần; `subscription_store` ghi verbatim; `api_subscription_store` phơi qua HTTP (`subscription/mod.rs:99,125,176`, `api_server.rs:142`).
**Kịch bản:** một `POST` với `entitled: true` mở khoá toàn bộ. Câu "gate ở backend, không chỉ UI" trong plan của tôi là không cưỡng chế được.
**Sửa:** hoặc ký payload, hoặc thừa nhận gate chỉ là UI-level và ngừng tuyên bố ngược lại.

### C3. Năm capability id resolve về hư vô — khoá 100% người dùng, kể cả người trả tiền
**Reviewer:** Scope, Assumption Destroyer, Security · **Ảnh hưởng:** toàn bộ Pro plan
`hasCapability()` đòi sidecar khai báo (`pro.svelte.ts:111-113`); `pro/mod.rs:20-21` ghi rõ chưa bundle sidecar; socket path trong `pro/bridge.rs:32-41` là placeholder tự ghi chú "once the binary exists". `GatedCapability` (`telemetry/events.rs:30-34`) chỉ là enum **telemetry**, không phải nguồn entitlement.
**Kịch bản:** mọi khách trả tiền rơi vào `isPaidButUnavailable` → màn hình "khôi phục Pro component" cho cả 5 tính năng.
**Bonus:** 5 capability id là đúng **feature ladder** mà `docs/pricing-rationale.md:24-27` tuyên bố từ chối, và `subscription/mod.rs:35-36` ghi `paid` là "the only field a gate may read".
**Sửa:** gate trên `proState.paid`. Xoá toàn bộ khái niệm capability id khỏi plan.

### C4. `crash.rs` không ghi gì xuống đĩa
**Reviewer:** cả 3 · **Ảnh hưởng:** Free P2 bước 1
Panic hook chỉ `eprintln!` (`crash.rs:48-55`); doc `:13-15` nói việc truyền đi là để sau.
**Kịch bản:** `latest_report()` tôi đề xuất không có file để đọc. Persistence + redact-lúc-ghi là **feature chưa được scope**, không phải "thêm một hàm đọc".

### C5. `redact()` phủ 1 trong 4 lớp secret cần thiết
**Reviewer:** Security, Assumption Destroyer · **Ảnh hưởng:** Free P2 bước 9 + Success Criteria
Regex query-param neo ở `[?&]` (`redact.rs:26`) → `POSTGRES_PASSWORD=hunter2` trong log container lọt. `KEY_SHAPES` (`:54-72`) không có AKIA, JWT, PEM. Không gì che `/Users/<tên>`. Module **cố ý** từ chối entropy matching tổng quát (`:49-53`).
**Kịch bản:** bundle chứa mật khẩu DB đi thẳng vào GitHub issue công khai.
**Sửa:** mở rộng `redact.rs` là hạng mục bắt buộc có phạm vi riêng, không phải "bổ sung nếu test lộ thiếu sót" như tôi viết.

### C6. Free P4 xoá hai component Kubernetes dựa trên tiền đề sai
**Reviewer:** Scope, Assumption Destroyer
`XRay.svelte` là graph tài nguyên k8s nhận prop `namespace` (`:3-15`); `ClusterTopology.svelte` liệt kê colima instance (`:3-11`). Cả hai là **component con** của `Kubernetes.svelte:13,14,577,579` — không phải route. `App.svelte:8-20` (13 import) và `Sidebar.svelte:31-58` (13 menu id) không có cái nào. Grep `xray|topology` trong locale: **0 kết quả**.
**Kịch bản:** làm theo plan → mất hai view k8s duy nhất, trong khi chính phase đó tuyên bố k8s ngoài phạm vi.
**Sửa:** viết lại P4 hoàn toàn, hoặc bỏ.

### C7. Không tồn tại khái niệm "instance" ở tầng docker
**Reviewer:** Failure Mode, Security, Assumption Destroyer · **Ảnh hưởng:** Free P4, Pro P2, Pro P5
`detect_docker_host()` quét `~/.colima` và lấy profile chạy **đầu tiên** (`path_util.rs:172-202`); `run_cmd` set `DOCKER_HOST` từ đó (`helpers.rs:103-107`); `docker --context` xuất hiện **0 lần** trong repo (engine selection theo socket, `docker_state.rs:11-22`).
**Kịch bản:** `?instance=` là tham số nhận-rồi-bỏ; pipeline `--context` của Pro P5 không có đường ống bên dưới; **schema metrics thiếu cột instance** → mẫu của hai instance trộn vào nhau, không gỡ lại được sau khi đã ghi.
**Sửa:** cột instance phải có **trước khi ghi dòng dữ liệu đầu tiên**. Lớp targeting đa instance là hạ tầng riêng.

### C8. Pro P1 nhảy qua một cổng go/no-go chưa chạy
**Reviewer:** Scope
`260810-2258-colimaui-commercial-foundation/phase-05-compose-auto-fix-spike.md` đặt auto-fix là **spike có cổng**: `:44,52,59` ghi "Không đặt auto-patch đầy đủ làm cổng launch"; `:144` ghi corpus hiện có **0 file**. `:19` đã kết luận giữ comment YAML là bất khả thi với `serde_yml 0.0.12` (`Cargo.toml:29`, chưa đổi).
**Kịch bản:** Pro P1 cam kết lại đúng điều đã bị bác, bỏ qua cổng quyết định đã thoả thuận. Lỗi ở bước quét plan chồng lấn của tôi. Plan cũng ghi sai tên crate (`serde_yaml` vs `serde_yml`).

### C9. Round-trip LLM auto-fix xoá secret, `compose_validate` không phát hiện được
**Reviewer:** Security · **Ảnh hưởng:** Pro P1 Architecture + bước 5
`llm_payload_preview` dựng từ `redact_compose()` — hàm này **bôi trắng** `environment:` / `secrets:` / `env_file:` (`compose_diagnose.rs:293`). Bảo model trả file đầy đủ rồi ghi đè = xoá sạch các block đó; `docker compose config --quiet` (`:212-256`) vẫn pass.
**Kịch bản:** người dùng bấm Áp dụng, file "hợp lệ", toàn bộ biến môi trường biến mất. Tiêu chí "0 file bị hỏng thêm" của tôi **không cưỡng chế được**. Gửi YAML thô để né thì thành rò rỉ credential.
**Sửa:** LLM chỉ được trả **patch có phạm vi giới hạn**, và phải diff-check rằng không key nào bị mất — validate cú pháp là không đủ.

---

## High (6)

### H10. `sse.rs` không có sổ sách subscriber
`sse.rs:17-27` là một broadcast channel process-global; `routes/misc.rs:7` subscribe không ghi sổ; `receiver_count()` đếm cả client k8s/KB/docker. Thiết kế "đếm subscriber để dừng collector" — điểm tôi gọi là quan trọng nhất Free P3 — **không có gì để bám**. Fallback HTTP subscribe/unsubscribe rò rỉ khi reload/crash → collector (và sink SQLite của Pro) chạy mãi.

### H11. SSE âm thầm mất mẫu khi lag
`broadcast::channel(64)` (`sse.rs:22`) dùng chung với burst refresh của docker watcher; `routes/misc.rs:9-12` map `Lagged(n)` → `None`. Biểu đồ vẽ đường thẳng qua lỗ hổng; biện pháp "bỏ qua gap lớn" ở Pro P3 không thể kích hoạt vì gap vô hình.

### H12. Crash-loop detection không có nguồn sự kiện
`docker_state.rs` không expose event stream; `:118-160` áp debounce trailing-edge 500 ms và tiêu thụ event tại chỗ. Restart loop nhanh gộp thành một refresh → luật 2 của Pro P4 **không bao giờ bắn**, nên luật 1 cứ restart mãi. Đúng kịch bản plan bảo là đã phòng.

### H13. Quota self-heal chỉ nằm trong bộ nhớ
Restart app reset cả quota lẫn đồng hồ vi phạm; chỉ `ExitRequested` được hook (`lib.rs:329-331`). Quota — biện pháp giảm thiểu số một của rủi ro cao nhất roadmap — **không chặn gì** qua vòng đời tiến trình.

### H14. Self-healing sống lâu hơn entitlement; công tắc tắt nằm sau gate
`ProGate` ở trạng thái locked render upsell **thay cho** children (`ProGate.svelte:45-51,63-80`). Hết hạn (`subscription/cache.rs:54`) → trang Self-healing và **công tắc tắt toàn cục biến mất**, trong khi executor (P4 không gắn kiểm tra entitlement nào) vẫn restart container và VM. Cộng thêm: `pro_status()` re-detect mỗi lần gọi (`pro/mod.rs:56-58`) nhưng đăng ký sink là boot-time → nâng cấp phải restart, hạ cấp vẫn ghi tiếp.

### H15. Ba tuyên bố "đã có" khác đều sai
- `tauri-plugin-dialog`: vắng ở `capabilities/default.json`, `Cargo.toml`, `package.json`. Thêm nó là thay đổi trust boundary — và dialog **không phải** cơ chế uỷ quyền cho một HTTP route.
- `validation.rs` không có API giới hạn thư mục dùng được: `contains_shell_injection` (`:7-21`) là denylist metachar; `assert_path_within` (`:174`) cần base mà plan không nêu. `~/Library/LaunchAgents/x.plist` không chứa `..` và qua sạch mọi kiểm tra tôi liệt kê.
- Không có primitive pipe: mọi lệnh docker đi qua `Command::output()` buffered (`helpers.rs:99-125`) → `save` 500 MB nằm hết trong RAM, và huỷ giữa chừng để lại layer treo mà `docker images` không hiện.

---

## Medium — thu gọn phạm vi (không phải lỗi, là thừa)

### M16. Gộp và cắt
- **Pro P2 → gộp vào P3.** P2 không ship UI, có đúng một consumer.
- **Ba bảng downsample là over-engineering.** Raw giữ 1 giờ → không bao giờ vượt ~36k dòng, tự phản bác con số 864k/ngày tôi dùng để biện minh.
- **`metrics.db` trộn dữ liệu prune được (samples) với `heal_rules`/`heal_log`/`alert_rules`** — mâu thuẫn với chính lập luận tách DB của P2. Tuyên bố WAL cũng không đứng vững: pattern SQLite duy nhất trong repo là `static DB: OnceLock<Mutex<Connection>>`, không pragma, `.expect()` panic, không có câu chuyện async (`knowledge_bank.rs:7-14,25`).
- **`FixStrategy` trait + 5 struct = một match statement.** `compose_diagnose.rs:45` đã trả về tập category `&'static str` đóng.
- **`MetricSink` "không có `if pro`" là sai** — nhánh chỉ dời sang `lib.rs`. Thêm nữa cả 5 module Pro đều được lên kế hoạch đặt trong `commands/` (MIT core), mâu thuẫn `pro/mod.rs:3-6` và `pricing-rationale.md:108`.
- **Luật heal 3-4 vĩnh viễn chỉ là advisory** → tiêu chí "mỗi kịch bản phục hồi đúng ở chế độ Auto" không thoả mãn được với chúng.
- **`docker load` thêm "cho đối xứng"** — không có tiêu chí nghiệm thu, YAGNI.

### M17. Pro P5 nằm ngoài danh mục đã thoả thuận
Không có trong 9 mục của `commercial-foundation/plan.md:92-100`; bỏ qua entry gate Wave 2 và trần 3 mục (`phase-07:24,38,90`).
**Đề xuất của reviewer: cắt hẳn Pro P5** — không ai phụ thuộc, phạm vi lớn nhất, ngoài danh mục. Cắt nó cũng cho phép Free P4 bỏ tham số `?instance=` mang tính đầu cơ.

---

## Đánh giá tổng thể

Hai plan không sửa vá được. Chúng giả định một nền tảng chưa tồn tại:

| Plan giả định | Thực tế |
|---|---|
| Entitlement gate hoạt động | Capability resolve về hư vô; backend gate là blob client gửi |
| API cục bộ an toàn | Token phát không cho mọi tiến trình local |
| `redact()` che secret | Che 1/4 lớp; cố ý từ chối generic matching |
| `crash.rs` lưu crash | Chỉ `eprintln!` |
| SSE đếm được subscriber | Broadcast channel không sổ sách |
| Có khái niệm instance | `docker --context`: 0 lần trong repo |
| Có nguồn event container | Debounce nuốt burst |

**Bảy hạng mục tiên quyết** trên đều không nằm trong hai plan, và mỗi hạng mục là một phase thật sự chứ không phải một bước.

## Câu hỏi chưa giải quyết

1. Hardening `/api/auth/token` — sửa ngay như hotfix bảo mật, hay đưa vào plan tiên quyết?
2. Gate `proState.paid` có đủ không, khi backend gate không cưỡng chế được? Hay chấp nhận Pro là "honor system" ở bản desktop MIT?
3. Cổng go/no-go của compose auto-fix spike (`commercial-foundation/phase-05`) — chạy trước, hay tuyên bố bỏ qua có chủ đích?
4. Pro P5 (cluster transfer): cắt theo đề xuất reviewer, hay giữ và chấp nhận phải xây lớp targeting đa instance trước?
