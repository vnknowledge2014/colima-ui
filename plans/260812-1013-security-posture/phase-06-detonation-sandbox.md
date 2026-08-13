# Phase 6: Detonation sandbox

**Tier:** Pro · **Chờ:** phase 1, prereq P4 · **Trạng thái: XONG 2026-08-13** (v1, `--network none`)

## Đã giao

| Thành phần | Đường dẫn |
|---|---|
| Lõi phiên, collector, teardown, sweep | `src-tauri/src/commands/detonation.rs` |
| HTTP | `src-tauri/src/routes/detonation.rs`, 5 route trong `api_server.rs` |
| Client | `src/lib/api/detonation.ts` |
| UI | `src/components/security/{DetonationPanel,DetonationTimeline}.svelte`, tab Runtime trong `Security.svelte` |
| Chuỗi | 23 khoá × 4 locale |
| Bảo vệ chéo | `path_util.rs` — bỏ qua profile detonation khi dò daemon |

Kiểm: clippy `-D warnings` sạch · 483 test Rust qua (14 test detonation) ·
`pnpm lint`/`typecheck`/`check` sạch. Parse của collector đối chiếu với output
`docker top`/`diff`/`inspect` thật.

Hai vòng review: `plans/reports/from-code-reviewer-to-cook-260813-0823-*` và
`…-260813-0851-detonation-phase6-fix-verification.md`.

### Hai lỗi nghiêm trọng review bắt được — ghi lại vì cả hai đều phản trực giác

**1. Giữ docker context bị vô hiệu hoá bởi `DOCKER_HOST`.** `helpers::build_cmd`
đặt `DOCKER_HOST` cho mọi lệnh `docker`, và CLI luôn báo `default` từ
`context show` khi biến đó có mặt. Nên `previous_context` luôn là `default`,
so sánh `default == default` rồi không khôi phục gì. Đúng cái bug mà phát hiện
Gate 0 sinh ra để chặn, tái xuất hiện thấp hơn một tầng. Sửa: dựng `Command`
trực tiếp với `.env_remove("DOCKER_HOST")`.

**2. `detect_docker_host` có thể trỏ cả app vào VM detonation.** Nó trả về
profile đang chạy **đầu tiên** theo thứ tự `read_dir`. Trong lúc detonation,
toàn bộ mặt docker của app — danh sách container, metrics, bollard — có thể bám
vào VM đang chạy mẫu, rồi bám vào hư không khi VM đó bị xoá. Sửa: bỏ qua theo
prefix, và prefix giờ nằm một chỗ duy nhất (`path_util`) dùng chung.

Ngoài ra: chặn tiêm cờ qua image ref (`image: "--privileged"`), khoá một phiên
tại một thời điểm (kiểm lại dưới lock), tôn trọng Stop trong ~30s dựng máy,
lưới an toàn khi worker panic, ngưỡng "wedged" để một lệnh treo không khoá vĩnh
viễn tính năng, và giới hạn `dir` của export trong thư mục home.

### Còn nợ

- Sinkhole (thay cho `--network none`) — hoãn có chủ đích, cần test khẳng định
  không gói nào rời VM.
- Hai component Svelte **chưa có test chạy được**: `pnpm test` hỏng toàn repo do
  `undici@8.10.0` + `jsdom@30` (`webidl.util.markAsUncloneable is not a
  function`), hỏng ở mọi file test kể cả file không liên quan. Có trước thay đổi
  này.

## Gate 0 — đã đo 2026-08-13

Báo cáo: `plans/reports/from-cook-gate0-to-owner-260813-0823-detonation-and-falco-viability-report.md`

| Phép đo | Kết quả | Hệ quả |
|---|---|---|
| Tạo instance nguội | **24 giây** | Dưới ngưỡng 3 phút → **tạo theo yêu cầu**, bỏ phương án instance chờ sẵn |
| Huỷ | 2 giây | — |
| Khởi động lại instance cũ | 30 giây | **Chậm hơn tạo mới** → không giữ lại instance giữa các phiên |
| Metadata trên host | 12 KB | Không đáng lo |

**Mạng v1: `--network none`.** Sinkhole hoãn sang vòng sau, kèm test khẳng định
không gói nào rời VM (chủ dự án chốt 2026-08-13).

### Ràng buộc mới phát hiện khi đo — docker context bị cướp

`colima start -p <name>` **đổi docker context toàn cục**. Bỏ qua điều này thì mọi
lệnh docker sau đó — kể cả của chính ColimaUI — trỏ vào VM detonation, và danh
sách container của người dùng sẽ hiện container của mẫu đang bị kích nổ.

1. Lưu `docker context show` trước khi tạo; khôi phục **ngay sau** khi
   `colima start` trả về, không đợi tới teardown.
2. Mọi lệnh docker của detonation dùng `--context colima-<name>` tường minh.
3. `sweep_orphans` cũng khôi phục context — nhánh crash không chạy bước 1.
4. Test khẳng định context môi trường không đổi qua trọn một phiên, kể cả đường
   lỗi và đường timeout.

## Overview

Lợi thế cấu trúc mà Docker Desktop không có: **mọi thứ đã chạy trong Lima VM.**
Ranh giới cô lập đã tồn tại và người dùng đã trả tiền cho nó bằng RAM.

Phase này khai thác điều đó: chạy một image chưa tin cậy trong **instance Colima
riêng, dùng một lần**, không mạng ra ngoài, và quan sát nó làm gì.

Câu hỏi nó trả lời: *"image này, khi chạy, có làm gì ngoài việc nó nói không?"*
Scan tĩnh (phase 1) không trả lời được câu đó.

## Giới hạn phải nói thẳng — trước cả khi thiết kế

Chạy phần mềm đáng ngờ trong Lima VM trên macOS **không phải cô lập cấp độ phân
tích malware**. VM escape tồn tại. Người dùng có thể vô ý mount thư mục thật.

**Ngôn từ bắt buộc trong UI:** "quan sát hành vi trong môi trường cô lập".
**Ngôn từ bị cấm:** "phân tích malware an toàn", "sandbox an toàn tuyệt đối",
bất cứ câu nào ngụ ý người dùng có thể chạy malware thật mà không sao.

Đây là ràng buộc sản phẩm, không phải kỹ thuật, và nó không thương lượng được.

## Requirements

**Functional**
- Tạo instance Colima riêng cho một phiên detonation, huỷ sạch sau khi xong.
- Chạy image trong đó với: không mạng ra ngoài (hoặc sinkhole ghi lại mọi kết nối), không mount host, tài nguyên giới hạn, timeout cứng.
- Ghi lại: tiến trình sinh ra, kết nối mạng bị chặn (đích + port), thay đổi filesystem so với layer gốc, exit code, stderr/stdout.
- Timeline theo thời gian, xuất được ra file.
- Huỷ phiên bất cứ lúc nào; timeout tự huỷ.

**Non-functional**
- **Không mount gì từ host.** Không ngoại lệ, không tuỳ chọn "nâng cao".
- Instance detonation có prefix tên riêng, không bao giờ trùng instance thật của người dùng.
- Huỷ instance là **bắt buộc chạy**, kể cả khi app crash giữa chừng → dọn rác lúc khởi động.
- Mặc định không mạng. Bật sinkhole là hành động có xác nhận rõ ràng.
- Gate `paid`; nút huỷ phiên luôn render kể cả khi entitlement hết hạn.

## Architecture

```
commands/detonation.rs
  ├─ SessionId, DetonationConfig { image, timeout_secs, network: None|Sinkhole,
  │                                cpus, memory_gb, cmd_override }
  ├─ create_instance()   ← tái dùng pattern của commands/kind.rs (sinh/huỷ môi trường tạm)
  │     tên: "colimaui-detonate-<short-id>"  — prefix cố định để dọn rác nhận ra
  ├─ run_and_observe()   ← chạy qua run_cmd_streaming (prereq P4)
  ├─ collectors:
  │     processes  — `docker top` lấy mẫu theo chu kỳ (thô nhưng không cần agent)
  │     network    — `docker network` sinkhole + đọc log DNS/conn bị chặn
  │     filesystem — `docker diff` so với image gốc
  │     lifecycle  — exit code, thời điểm, stdout/stderr (qua redact)
  ├─ teardown()  — LUÔN chạy, kể cả đường lỗi
  └─ sweep_orphans()  — gọi lúc app khởi động, xoá mọi instance có prefix

routes/detonation.rs  →  SSE timeline dùng publish_sse_event có sẵn

UI: src/components/security/DetonationPanel.svelte
      + DetonationTimeline.svelte
```

**Vì sao `docker top` lấy mẫu thay vì tracing syscall:** tracing syscall thật cần
eBPF/agent trong VM — đó là phase khác, một sản phẩm khác. Lấy mẫu tiến trình
bắt được thứ có giá trị nhất và rẻ nhất: image "hello world" mà sinh
`curl`/`sh`/miner là kết luận rõ ràng ngay. Đừng đánh đổi phase này lấy độ phủ
mà ta không cần.

**Vì sao sinkhole đáng giá hơn "no network":** `--network none` cho kết quả
"không có gì xảy ra" — image tốt và image xấu trông giống nhau. Sinkhole (mạng
nội bộ có DNS trả về địa chỉ giả) khiến malware **cố gọi ra và bị ghi lại**.
Đó chính là tín hiệu: image tĩnh mà gọi domain lạ là câu trả lời.

**Vì sao dọn rác lúc khởi động:** app crash giữa phiên detonation để lại một
instance Colima ăn RAM vĩnh viễn. Prefix cố định + sweep là cách duy nhất chắc chắn.

## Related Code Files

- Create: `src-tauri/src/commands/detonation.rs`, `src-tauri/src/routes/detonation.rs`
- Create: `src/components/security/{DetonationPanel,DetonationTimeline}.svelte`
- Modify: `src-tauri/src/lib.rs` — gọi `sweep_orphans()` lúc khởi động
- Modify: `src/pages/Security.svelte`, `src/lib/api/security.ts`
- Modify: `src/locales/{en,vi,ja,zh}.json` — kể cả các chuỗi cảnh báo giới hạn
- Đọc tham chiếu (không sửa): `src-tauri/src/commands/kind.rs`, `instance_reader.rs`, `commands/shell_sandbox.rs`

## Implementation Steps

1. ~~Đo chi phí tạo/huỷ một instance Colima.~~ **XONG 2026-08-13 — 24s, tạo theo yêu cầu.** Xem Gate 0 ở trên.
2. `create_instance` + `teardown` + `sweep_orphans` + lưu/khôi phục docker context. **Viết teardown và sweep trước khi viết run.**
3. Test rò rỉ: tạo 5 phiên, kill app giữa chừng, khởi động lại → không còn instance nào sót.
4. ~~Network sinkhole.~~ **v1 dùng `--network none`**; sinkhole hoãn vòng sau.
5. Bốn collector, gộp về một timeline có timestamp thống nhất.
6. `run_and_observe` + timeout cứng + huỷ.
7. UI timeline + banner giới hạn (không phải tooltip, không phải footnote).
8. Xuất báo cáo phiên ra file qua `assert_path_within`.

## Tests

- Teardown chạy trên cả đường thành công, đường lỗi, và đường timeout.
- `sweep_orphans` xoá instance có prefix, **không đụng** instance thật của người dùng — test với instance tên `default` đang chạy.
- Image cố `curl` ra ngoài → kết nối bị chặn xuất hiện trên timeline với đích.
- Không có mount nào từ host trong lệnh chạy — assert trên chuỗi lệnh sinh ra.
- Timeout đạt → container chết, instance bị huỷ.
- Output của image đi qua `redact` trước khi vào timeline.

## Risks

| Rủi ro | Xử lý |
|---|---|
| Người dùng tin đây là sandbox phân tích malware và chạy mẫu thật | Ngôn từ UI bị ràng buộc như mục trên. Banner thường trực, không tắt được |
| Instance mồ côi ăn RAM | Prefix + sweep lúc khởi động + test kill-app |
| Chi phí tạo instance làm tính năng khó dùng | Bước 1 đo trước, quyết kiến trúc dựa trên số thật |
| Sinkhole rò ra mạng thật do cấu hình sai | Test khẳng định: image cố gọi IP public → không có gói nào rời khỏi VM. Nếu không kiểm chứng được, ship `--network none` trước, sinkhole sau |
| Phase này hấp dẫn hơn phase 1–3 nên bị làm trước | plan.md đã ghi: đường tới hạn là 1→2→3 |

## Success Criteria

- Chạy một image lành → timeline nhàm chán, instance biến mất sạch sau đó.
- Chạy image có script gọi domain lạ → domain đó hiện trên timeline.
- Kill app giữa phiên, mở lại → không rác.
- Không có chuỗi UI nào hứa quá mức — rà thủ công cả 4 locale.
