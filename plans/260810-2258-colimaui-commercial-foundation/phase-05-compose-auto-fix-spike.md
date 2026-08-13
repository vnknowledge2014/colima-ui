---
phase: 5
title: "Compose Auto-Fix Spike"
status: in_progress
priority: P1
effort: "2 ngày spike + 5-8 ngày có điều kiện"
dependencies: [3]
---

# Phase 5: Compose Auto-Fix Spike

## Overview

Tính năng bán hàng chính — nhưng làm theo kiểu **spike trước, cam kết sau**.

Bản v1 cam kết 8-10 ngày cho kiến trúc 3 tầng với corpus 30 file, trong đó tầng 1 là bộ quy tắc tất định tự viết. Red-team chỉ ra hai vấn đề nghiêm trọng:

1. **`grep -c '"config"' src-tauri/src/commands/compose.rs` = 0** — `docker compose config` chưa từng được gọi. Docker đã báo phần lớn lỗi cú pháp/schema/biến chưa định nghĩa **miễn phí**. Tầng 1 định xây lại thứ đã có sẵn.
2. **Patch giữ nguyên comment/format là bất khả thi với dependency hiện tại.** Dependency YAML duy nhất là `serde_yml 0.0.12` — không có span, không có comment, không có tiền lệ trong repo. Mà v1 lại đặt nó làm cổng launch.

Điểm sáng: **tầng 2 và 3 đã tồn tại**. `knowledge_bank.rs` (`:337,459,498`) và `ai_chat.rs` (571 dòng) đã chạy được. `compose.rs` đã bắt stderr ở `:43,86,104,122,142,166`.

## Cấu trúc phase

### Bước A — Spike (2 ngày, làm trước, không điều kiện)

Ship phiên bản mỏng nhất có thể:

```
docker compose config  (chưa từng gọi — chỉ cần thêm)
        │  bắt stderr, parse thông báo lỗi của Docker
        ▼
truy vấn Knowledge Bank theo chữ ký lỗi  (knowledge_bank.rs đã có)
        │
        ▼
nếu không khớp → LLM với BYOK  (ai_chat.rs đã có)
        │
        ▼
hiện chẩn đoán + gợi ý sửa dạng VĂN BẢN (chưa auto-patch)
```

Đo trên ≥15 file compose lỗi thật: bao nhiêu % được `compose config` + KB giải quyết mà không cần LLM? Gợi ý có hữu ích không?

### Bước B — Cổng go/no-go

| Kết quả spike | Hành động |
|---|---|
| `compose config` + KB đã đủ hữu ích cho ≥60% case | **Không xây tầng quy tắc riêng.** Đầu tư vào chất lượng gợi ý và auto-patch |
| Chỉ giải quyết được <30%, và LLM cũng kém | **Dừng.** Đây không phải flagship. Đổi tính năng bán hàng chính |
| Ở giữa | Xây quy tắc **chỉ cho các lớp lỗi mà Docker không báo rõ**, tối đa 3 quy tắc |

### Bước C — Auto-patch (5-8 ngày, chỉ khi qua cổng)

Áp dụng bản vá tự động. **Vấn đề giữ nguyên comment/format phải giải quyết trước:**
- Phương án 1: thêm crate YAML có span (đánh giá riêng, thêm dependency).
- Phương án 2: chỉ auto-patch các sửa đổi đơn giản theo dòng (thêm/sửa 1 dòng), phức tạp hơn thì chỉ hướng dẫn thủ công.
- Phương án 3: hiện diff để người dùng tự áp bằng editor của họ.

Phương án 2-3 rẻ hơn nhiều và có thể đã đủ. **Không đặt auto-patch đầy đủ làm cổng launch.**

## Requirements

**Bước A**
- Gọi `docker compose config` để validate trước khi `up`.
- Parse thông báo lỗi Docker → chữ ký lỗi.
- Truy vấn KB; nếu trượt thì gọi LLM với BYOK.
- Hiện chẩn đoán ở UI, có phản hồi 👍/👎 nạp về KB.

**Non-functional**
- Bước A không phát sinh request mạng nào ngoài LLM (và LLM phải hỏi trước).
- **Không gửi file compose lên LLM khi chưa lọc secret.** Compose thường chứa mật khẩu, token. Lọc theo schema (bỏ giá trị `environment`, `secrets`, không bao giờ inline `env_file`) là **mặc định**, không phải tuỳ chọn.
- Không bao giờ sửa file khi chưa được xác nhận rõ ràng.

## Related Code Files

**Bước A — có thể làm phần lớn trong core hoặc sidecar:**
- Modify: `src-tauri/src/commands/compose.rs` — thêm gọi `docker compose config`
- Modify: `src-tauri/src/commands/knowledge_bank.rs` — truy vấn theo chữ ký lỗi compose
- Create: `src/components/compose/DiagnosePanel.svelte`
- Modify: `src/pages/Compose.svelte` — nút "Phân tích"
- Reuse: `src/components/DiffView.svelte` (tạo ở plan P0 Phase 5)

**Bước C — repo private:**
- Create: `src/compose_fix/{engine,patch,report}.rs`
- Create: `src/compose_fix/redact.rs` — lọc secret theo schema trước khi gửi LLM

## Implementation Steps

1. Thu ≥15 file compose lỗi thật (GitHub issues, Stack Overflow, dự án riêng) kèm bản vá đúng.
2. Thêm gọi `docker compose config`; bắt và chuẩn hoá stderr.
3. Nối truy vấn KB theo chữ ký lỗi.
4. Nối LLM (BYOK) làm phương án cuối, **có lọc secret mặc định** + hiện đúng nội dung sắp gửi.
5. `DiagnosePanel.svelte` — hiện chẩn đoán dạng văn bản, phản hồi 👍/👎.
6. **Đo trên corpus, ghi kết quả vào `plans/reports/`.**
7. **Cổng go/no-go** — quyết định dựa trên số liệu, không dựa trên mong muốn.
8. (Có điều kiện) Bước C: chọn phương án auto-patch, backup + xác nhận + hoàn tác.

## Tests / Validation

- Integration: `docker compose config` bắt được lỗi trên corpus; ghi lại tỉ lệ.
- Unit: lọc secret — `environment`, `secrets`, `env_file` không bao giờ rời máy (test enforce).
- Unit: truy vấn KB khớp đúng chữ ký lỗi.
- **Benchmark corpus 15 file** — đây là đầu ra chính của bước A.
- Thủ công: chẩn đoán có hữu ích không (đánh giá chủ quan nhưng phải ghi lại).
- (Bước C) Thủ công: từ chối bản vá → file không đổi.

## Success Criteria

**Bước A (bắt buộc):**
- [ ] `docker compose config` được gọi và bắt lỗi.
- [ ] KB được truy vấn theo chữ ký lỗi compose.
- [ ] LLM là phương án cuối, có lọc secret **mặc định**, có xác nhận trước khi gửi.
- [ ] Benchmark trên ≥15 file, kết quả ghi thành báo cáo.
- [ ] **Cổng go/no-go đã được quyết định bằng số liệu.**

**Bước C (có điều kiện):**
- [ ] Phương án auto-patch được chọn dựa trên chi phí thật, không mặc định làm bản đầy đủ.
- [ ] Luôn xác nhận + backup + hoàn tác.
- [ ] Giữ nguyên comment/format, **hoặc** nói rõ giới hạn và chỉ patch trường hợp đơn giản.

## Tiến độ — 2026-08-11

**Bước A: code xong, benchmark chưa chạy.**

Đã làm:

- [x] `docker compose config --quiet` được gọi thật — `src-tauri/src/commands/compose_diagnose.rs`.
- [x] `error_signature()` chuẩn hoá bỏ đường dẫn tuyệt đối + số dòng, để giải pháp
      lưu trong KB dùng lại được giữa các máy.
- [x] Truy vấn KB theo chữ ký, qua `kb_query` sẵn có.
- [x] `redact_compose()` — lọc secret theo schema, **mặc định, không có tuỳ chọn tắt**.
      `environment` xoá giá trị giữ key, `secrets` xoá hẳn, `env_file` không bao giờ đọc.
- [x] LLM là bước cuối, **do người dùng bấm**, có nút xem trước đúng nội dung sẽ gửi.
- [x] `DiagnosePanel.svelte` + tab "Diagnose" trong modal Compose; 👍/👎 nối `kb_feedback`.
- [x] REST route `/api/compose/diagnose` cho browser mode.
- [x] 13 test Rust, gồm 5 test enforce việc lọc secret.
- [x] `scripts/compose-diagnose-benchmark.sh` — chạy trên thư mục corpus, xuất CSV + tỉ lệ.
- [x] `DiagnosePanel` lắp `ProGate capability="compose.autofix"` — offer flagship ở
      trạng thái Free (affordance, chưa blur vì chưa có preview mẫu thật). Slot children
      là nơi hành động auto-patch (Bước C) sẽ mount khi sidecar khai báo capability.

Chưa làm — **chặn cổng go/no-go**:

- [ ] Bước 1: thu ≥15 file compose lỗi **thật** kèm bản vá đúng. Chưa có file nào.
- [ ] Bước 6: chạy benchmark, ghi kết quả vào `plans/reports/`.
- [ ] Bước 7: quyết cổng go/no-go bằng số liệu.
- [ ] Bước C auto-patch — chỉ làm nếu qua cổng.

### Quyết định lệch so với chữ trong plan

**LLM gọi từ frontend, không phải Rust.** Plan liệt kê bước gọi LLM ở backend, nhưng
yêu cầu phi chức năng bắt "LLM phải hỏi trước" và "hiện đúng nội dung sắp gửi".
Backend trả về `llm_payload_preview` đã lọc; frontend gọi `aiApi.chat` sau khi người
dùng xác nhận. Tránh nhân bản đường truyền API key vào Rust — key đã nằm ở
`settingsState` phía frontend.

**Thêm guard đường dẫn.** `compose_diagnose` đọc file và trả nội dung về cho caller —
các lệnh compose khác thì không. Ở chế độ server, đó sẽ là một primitive đọc file bất
kỳ cho client đã xác thực. Nên chỉ chấp nhận `.yml`/`.yaml`.

### Ghi chú từ lần chạy thử harness

Chạy trên 2 file (chỉ để smoke-test script, **không phải benchmark**), output Docker
thật bắt được 2 bug trong bản `categorize()` đầu tiên: Docker in `additional
properties` (số nhiều) chứ không phải `additional property`, và bản copy của
`categorize()` trong script bash đã lệch khỏi bản Rust ngay lập tức. Đã sửa; script
không còn phân loại nữa, chỉ ghi thông báo thô vào CSV. Test giờ dùng chuỗi stderr
nguyên văn của Docker Compose 5.4.0.

## Risk Assessment

| Rủi ro | Mức | Giảm thiểu |
|---|---|---|
| **Xây lại thứ `docker compose config` cho không** | Cao | Bước A dùng chính nó trước khi viết quy tắc nào |
| **Auto-patch không khả thi với serde_yml** | Cao | Không đặt làm cổng launch; có 3 phương án thay thế rẻ hơn |
| Gửi secret trong compose lên LLM | Cao | Lọc theo schema là mặc định, có test enforce |
| Bản vá sai làm hỏng file người dùng | Cao | Xác nhận + backup + hoàn tác; ưu tiên phương án chỉ hiện diff |
| Không đủ giá trị cho $6 | Cao | Cổng go/no-go phát hiện sau 2 ngày thay vì sau 10 |
| LLM bịa bản vá nghe hợp lý nhưng sai | Cao | Ưu tiên `compose config` + KB; nói rõ đâu là gợi ý từ AI |
