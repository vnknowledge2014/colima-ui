# Phân tích xây dựng: Graph view · Activity Monitor · File transfer + Export image TAR

Ngày: 2026-08-12 · Nhánh: `dev` · Phương pháp: sequential-thinking (explicit)
Plan liên quan: `plans/260811-2241-free-tier-foundation/` (phase 1, 3, 4), `plans/260811-2300-platform-prerequisites/`

---

## Thought 1/8 — Ba tính năng này khác hạng nhau, không cùng loại việc

| Tính năng | Bản chất | Rủi ro chính |
|---|---|---|
| Export image TAR + file transfer | I/O dài, nhiều GB, huỷ được | Bộ nhớ, path traversal, tiến trình mồ côi |
| Activity Monitor | Vòng lấy mẫu nền + push liên tục | CPU nền, vòng đời collector, thiết kế schema |
| Graph view | Đọc-then-vẽ, thuần hiển thị | Không có rủi ro hệ thống; chỉ chi phí UI |

Suy ra: Graph view **rẻ và độc lập**, Activity là **quyết định kiến trúc**, Transfer là **thay đổi trust boundary**. Không nên làm song song ba cái với cùng mức độ cẩn trọng.

## Thought 2/8 — Kiểm chứng nền tảng: 3 tiền đề trong plan hiện đã SAI/thiếu

Grep-verify hôm nay (`src-tauri/src`):

| Thứ plan giả định có | Thực tế |
|---|---|
| `sse::subscriber_count` (gate bật/tắt collector) | **KHÔNG tồn tại.** `sse.rs:29` chỉ có `publish_sse_event`. Không có registry subscriber, không có `stream-lagged`. |
| `helpers::run_cmd_streaming` | **KHÔNG tồn tại.** Chỉ có `run_cmd:99` = `Command::output()`, buffer toàn bộ vào RAM. |
| `tauri-plugin-dialog` | **KHÔNG có** ở `Cargo.toml`, `package.json`, `capabilities/default.json`. |

Đã có thật, dùng được ngay:
- `containers.rs:606 all_container_stats`, `:624 container_top`
- `networks.rs:45 list_networks`, `volumes.rs:57 list_volumes`, `inspect_*` cho cả 3 loại
- `validation.rs:169 assert_path_within` (canonicalize parent, chống `..` + symlink)
- `publish_sse_event`, `docker_state.rs`, `poller.rs`

Chưa có: `src/pages/Activity.svelte`, `src/pages/Topology.svelte`, `d3`/`d3-force` (0 lần trong `package.json`).
`ClusterTopology.svelte` + `XRay.svelte` là component con của `Kubernetes.svelte` — **không** phải trang topology Docker, không chồng lấn.

## Thought 3/8 — Hệ quả: thứ tự bắt buộc, không phải sở thích

- Activity Monitor **bị chặn** bởi subscriber-count trong `sse.rs`. Nếu bỏ qua, collector sẽ chạy mãi hoặc phải tự chế endpoint subscribe/unsubscribe — cái đó rò rỉ mỗi lần client reload/crash. Đây là lỗi không sửa được bằng patch UI.
- Export TAR **bị chặn** bởi streaming exec. Với `run_cmd` hiện tại, export image 2 GB nạp trọn vào RSS của app. Không có cách vòng.
- Graph view **không bị chặn bởi gì cả**. Mọi command nó cần đã tồn tại.

## Thought 4/8 [REVISION của giả định "làm theo số phase"] — Nên đảo thứ tự

- Ban đầu (plan free-tier): phase 1 (transfer) → 3 (activity) → 4 (graph).
- Vì sao đảo: phase 1 và 3 mỗi cái cần một tiên quyết chưa viết; phase 4 không cần gì. Làm graph trước cho ra tính năng hoàn chỉnh sớm, không nợ kỹ thuật, và dựng luôn khuôn route/sidebar/locale mà Activity sẽ tái dùng.
- Ảnh hưởng: thứ tự đề xuất **Graph → (streaming exec) → Transfer → (subscriber registry) → Activity**.

## Thought 5/8 — Graph view: chốt thiết kế

- Gộp dữ liệu ở backend, **một** endpoint `GET /api/topology`. Không để UI gọi 4 API rồi ghép.
- `commands/topology.rs` chỉ gọi command có sẵn (`list_containers`/`list_networks`/`list_volumes`/compose ps), không gọi `docker` trực tiếp.
- Node: container, network, volume, compose project, image. Edge: container→network/volume/project/image.
- Layout: tự viết force-directed (~150 dòng), tách module riêng để test độc lập. **Không thêm `d3-force`** — 50 node không cần, và nó là dependency cho đúng một trang.
- Bỏ tham số `?instance=`: repo không có lớp targeting đa instance (`docker --context`: 0 lần). Thêm tham số không ai đọc là nợ.

## Thought 6/8 — Activity Monitor: giá trị thật nằm ở collector, không ở đồ thị

- Chốt `MetricSample` **phẳng, có cột `instance`** trước khi code UI. Struct này sẽ thành schema bảng cho tầng lưu trữ Pro; sửa sau = migration.
- `MetricWriter` là `Option<...>`: kênh hiển thị (SSE) và kênh bền vững tách nhau ngay từ commit đầu. `metrics_collector.rs` không import gì từ `pro/`; nhánh Pro nằm ở nơi khởi tạo.
- Một tick = một lệnh `stats --no-stream` cho **tất cả** container (đã có `all_container_stats`), gom một batch, phát **một** SSE event. Không một event/container, không lặp từng container.
- `container_top` gọi on-demand khi mở panel process, không nằm trong vòng tick.
- UI: cửa sổ trượt giới hạn cứng (120 mẫu). Gặp `stream-lagged` → vẽ khoảng trống, không nội suy qua chỗ mất.

## Thought 7/8 — File transfer + Export TAR: đây là việc bảo mật, không phải việc UI

- Ba command: `container_cp`, `image_save`, `image_load`. Registry `Mutex<HashMap<id, JobHandle>>` để huỷ được — không có registry thì không có nút Huỷ đúng nghĩa.
- Validate path **trước** khi spawn: dùng `assert_path_within` nguyên bản, base = thư mục người dùng chọn qua dialog. Đừng dùng `contains_shell_injection` cho path — nó là denylist metachar cho lệnh exec, sai chỗ.
- Tiến độ: `docker save` không báo %. Đo **kích thước file đích** là cách trung thực nhất. Đừng bịa % từ số dòng stderr.
- Huỷ giữa chừng phải xoá `.tar` dang dở và không để lại `docker save` mồ côi.
- Thêm `tauri-plugin-dialog` là mở rộng trust boundary: phải sửa đồng thời `Cargo.toml`, `package.json`, `capabilities/default.json`. Việc này thuộc phase transfer, không "đã có sẵn".

## Thought 8/8 [FINAL] — Kế hoạch xây dựng đề xuất

| Thứ tự | Việc | Chặn bởi | Ghi chú |
|---|---|---|---|
| 1 | **Graph view** (`commands/topology.rs`, `GET /api/topology`, `Topology.svelte`, layout module, route + sidebar + 4 locale) | — | Ra được ngay, dựng khuôn cho trang sau |
| 2 | **Streaming exec** (`run_cmd_streaming` trong `helpers.rs`) | — | Tiên quyết, nhỏ, đo được bằng RSS |
| 3 | **File transfer + Export TAR** | (2) + dialog plugin | Đi kèm việc mở trust boundary |
| 4 | **SSE subscriber registry + `stream-lagged`** (`sse.rs`) | — | Có thể làm song song với (2)–(3), khác file |
| 5 | **Activity Monitor** | (4) | Chốt `MetricSample` trước khi code UI |

Gate nghiệm thu tối thiểu cho mỗi bước:
- Graph: 50 container/10 network vẽ mượt; trang Kubernetes không hồi quy.
- Transfer: RSS app **không** tăng theo kích thước image khi export; path `../` bị chặn trước khi spawn; huỷ để lại hệ thống sạch.
- Activity: đóng trang → collector dừng hẳn; 20 container tick 2s → CPU app < 2% (Apple Silicon); mở 1 giờ không phình bộ nhớ.

---

## Câu hỏi chưa giải quyết

1. Bước 4 (subscriber registry) và bước 2 (streaming exec) có được làm song song bởi hai người/hai nhánh không, hay tuần tự một người?
2. Graph view có cần node image ngay bản đầu, hay cắt để giảm số edge cho gọn?
3. Chu kỳ lấy mẫu mặc định của Activity: 2s hay 5s? Ảnh hưởng trực tiếp tới ngân sách CPU nền.
