# Phase 7: Mặt tiền Falco

**Tier:** Pro · **Chờ:** phase 5 · **Trạng thái: XONG 2026-08-13**

## Đã giao

| Thành phần | Đường dẫn |
|---|---|
| Detect trong VM, parser, correlate, vòng đọc theo byte offset | `src-tauri/src/commands/falco_bridge.rs` |
| Prompt diễn giải (không sinh verdict) | `src-tauri/src/commands/falco_triage.rs` |
| Lưu trữ + retention 7 ngày / 20k dòng | `security_history.rs` bảng `falco_events` |
| Alert | `alerts.rs::AlertMetric::RuntimeEvent`, qua `emit_security` |
| HTTP | 4 route trong `routes/security.rs` |
| UI | `components/security/EventStream.svelte`, tab Runtime |
| Bài KB | `resources/kb/{en,vi,ja,zh}/install-falco.md` |
| Chuỗi | 30 khoá `falco.*` × 4 locale + 2 khoá `activity.*` |

Kiểm: clippy `-D warnings` sạch · 505 test Rust (20 test falco) · 319 test
frontend · lint/typecheck/check sạch.

Review: `plans/reports/from-code-reviewer-to-cook-260813-0907-falco-phase7-review.md`
và `…-260813-0932-falco-phase7-fix-verification.md`.

### Lỗi nghiêm trọng nhất review bắt được — có trước phase này

**Cảnh báo bảo mật chưa bao giờ tới được người dùng.** `emit_security` không
publish `containerId`/`containerName`, nên phía nhận gọi
`alert.containerId.slice(0, 12)` trên `undefined`, ném lỗi, và `catch` rỗng nuốt
mất. Nghĩa là **mọi cảnh báo của phase 5** (điểm posture, CVE mới) đều được ghi
vào DB rồi im lặng biến mất. TypeScript không bắt được vì `FiredAlert.metric`
chỉ liệt kê 3 trong 6 biến thể. Đã sửa cả hai phía; kiểu giờ đầy đủ nên biến thể
mới sẽ fail lúc biên dịch.

### Ba lỗi do chính phase này tạo ra, đã sửa

1. **Đọc lại toàn bộ lịch sử.** Offset khởi tạo 0 → lần gắn đầu nuốt cả tệp log,
   lưu mọi sự kiện cũ và bắn alert cho từng cái. Sửa: nhảy tới cuối tệp khi gắn
   lần đầu — nhưng **giữ nguyên offset khi gắn lại cùng một tệp**, vì nhảy tới
   cuối lần nữa sẽ âm thầm vứt mọi sự kiện ghi trong lúc mất kết nối.
2. **Offset trôi.** `run_cmd` trả `from_utf8_lossy`: một byte hỏng thành 3 byte,
   `\r` được đếm nhưng `lines()` bỏ. Trôi cả hai chiều, mất sự kiện vĩnh viễn mà
   không dấu vết. Sửa: đọc byte thô, cắt tại `\n` cuối, cộng offset theo byte.
3. **Cooldown tin vào đồng hồ VM.** Lima chỉnh lại giờ sau khi Mac ngủ; một
   timestamp tương lai sẽ khoá cảnh báo của rule đó tới khi giờ thật đuổi kịp.
   Sửa: đo cooldown bằng đồng hồ host.

Ngoài ra: gộp alert theo (rule, subject) + cooldown 5 phút (đúng hợp đồng của
`emit_security`), cache `detect()` 30s (trước đó SSH + dump toàn bộ rule mỗi 5
giây), `ID` rỗng khớp mọi sự kiện qua `starts_with("")`, quote đường dẫn lấy từ
config người dùng sửa tay, và bỏ dòng `grep -c '"name"'` trong bài KB — chính là
kiểu đếm nhầm macros/lists mà module cảnh báo.

### Còn nợ

- **Chưa chạy thật với Falco sống.** Transport đã có test đơn vị; đường đi
  đầu-cuối thì chưa. Gate 0 chứng minh Falco chạy được và sinh event đúng shape,
  nhưng chưa ai bấm "Bắt đầu đọc" trên một máy có Falco đang chạy.
- Rotation trong vòng một chu kỳ 5 giây mà tệp mới dài hơn offset cũ sẽ đọc giữa
  dòng (không kiểm inode).
- gRPC output — cố ý hoãn, tail file là đủ.

## Gate 0 — ĐÃ QUA 2026-08-13

Báo cáo: `plans/reports/from-cook-gate0-to-owner-260813-0823-detonation-and-falco-viability-report.md`

Câu hỏi chặn — *Falco có chạy được trong Lima VM trên macOS ARM không?* —
**có**. Đo thật, không suy đoán:

| Hạng mục | Kết quả |
|---|---|
| Guest kernel | Linux 6.8.0-117-generic aarch64, Ubuntu 24.04 |
| Gói arm64 | Có, repo chính thức. Cài mất **9 giây** |
| Phiên bản | Falco 0.44.1 (aarch64), engine 0.62.0 |
| Driver `modern_ebpf` | Nạp và gắn được |
| Event thật | Có, kèm `container.id` / `container.image.repository` / `container.name` |

Shape event thu được đúng thứ `correlate()` cần — không phải tự chế schema.

### Ba ràng buộc phát sinh từ phép đo

**1. Detect không được dừng ở "tiến trình có chạy không".**
Lần chạy đầu: driver gắn thành công, tiến trình sống, **không một event nào** —
vì `falcoctl` bị tắt nên engine nạp **rỗng** rule (`Loading rules from:` trống),
dù `/etc/falco/falco_rules.yaml` có sẵn trên đĩa. Đây là trạng thái nguy hiểm
nhất một công cụ bảo mật có thể ở: đang chạy, trông khoẻ, không phát hiện gì.
→ Detect phải trả lời được **"đang nạp bao nhiêu rule"**. Không rule = trạng thái
riêng, hiển thị như cảnh báo, **không** phải "đã sẵn sàng".

**2. `brew install falco` cài nhầm phần mềm.**
`brew info falco` → trình lint VCL của Fastly (`ysugimoto/falco`), không phải
Falco của CNCF. Và Falco CNCF **không có bản chạy trên host macOS**.
→ Không `which falco` trên host; luôn dò **bên trong VM**. `install_hint` tuyệt
đối không được là `brew install falco`. Phân biệt hai nhị phân bằng chuỗi
version, không bằng sự tồn tại của file.

**3. Cảnh báo tracepoint arm64 là bình thường, và sẽ bị báo là lỗi.**
`open`/`creat` không tồn tại như syscall trên arm64 (chỉ có `openat`), nên mỗi
lần khởi động trên Mac ARM đều in `failed to determine tracepoint
'syscalls/sys_enter_open'` + `TOCTOU mitigation may not properly work`. Phát
hiện vẫn chạy bình thường. → Nếu UI hiện log Falco thô, phải lọc hoặc chú thích;
đây là thứ đầu tiên người dùng thấy và tưởng là hỏng.

**Unresolved #3 của plan.md (bundle Falco hay không):** nghiêng hẳn về *người
dùng tự cài* — cài mất 9 giây, ta bundle không thêm giá trị nào.

## Overview

Đây là câu trả lời đã rút gọn cho yêu cầu ban đầu *"AI bắt event realtime, thăm
dò rootkit/worm"*. Quyết định #2 của chủ dự án: **ColimaUI là mặt tiền cho
Falco, không phải đối thủ của Falco.**

Ta **không** viết engine phát hiện. Falco (CNCF, Apache-2.0) đã làm việc đó
bằng eBPF, có đội ngũ lớn hơn ta nhiều lần, và miễn phí. Ta bán thứ Falco không
có: giao diện tử tế, tương quan với container/compose đang chạy, và diễn giải
bằng AI.

## Đính chính phải giữ trong đầu suốt phase

| Yêu cầu ban đầu | Thực tế kỹ thuật |
|---|---|
| "Bắt event realtime để thăm dò rootkit" | Rootkit sống **dưới** lớp Docker events. Docker events không bao giờ thấy nó. Cần eBPF → chính là Falco |
| "Chạy ngầm như Grafana" | Grafana không phát hiện gì cả — nó hiển thị thứ Prometheus thu thập. Đúng ra ta là **Grafana**, còn Falco là **Prometheus** |
| "AI chạy analysis realtime" | LLM **không** đứng ở đường phát hiện: chậm, đắt, false positive. Luật Falco phát hiện, LLM diễn giải sau |
| "Cắm vào hệ thống remote" | Mỗi host remote cần Falco của chính nó. Ta kết nối tới output của nó, không cài agent do ta viết |

Nếu ba dòng đầu bị lãng quên giữa lúc implement, phase này sẽ trượt thành viết
lại một EDR. Đó là cách chắc chắn nhất để phá roadmap.

## Requirements

**Functional**
- Detect Falco qua `system_capabilities.rs` (đúng pattern phase 1).
- Kết nối tới output của Falco: đọc file JSON output và/hoặc gRPC output.
- Tương quan event với container/compose đang chạy: `container.id` → tên service người dùng biết.
- Hiển thị event stream + lọc theo priority/rule/container.
- Event priority cao → alert qua `alerts.rs` (Pro P2), **cùng đường với phase 5**.
- AI diễn giải một event hoặc một cụm event: "chuỗi này nghĩa là gì, có đáng lo không, làm gì tiếp".

**Non-functional**
- Không cài Falco hộ người dùng. Không viết agent. Không yêu cầu quyền root từ app của ta.
- Không có Falco → tab hiện hướng dẫn cài, không phải lỗi.
- Event lưu vào `security.db` (phase 5) với retention. Không giữ vô hạn.
- AI diễn giải là **hành động người dùng bấm**, không tự chạy trên mọi event (BYOK, nhưng vẫn là tiền của người dùng).
- Gate `paid`; kiểm `paid` trong vòng lặp đọc event, không phải lúc đăng ký.

## Architecture

```
commands/falco_bridge.rs
  ├─ detect qua system_capabilities (thêm "falco")
  ├─ FalcoEvent { time, priority, rule, output, fields, container_id, tags }
  │     giữ nguyên shape của Falco, KHÔNG tự định nghĩa lại schema phát hiện
  ├─ nguồn: tail file JSON output  (đơn giản, ít phụ thuộc — làm trước)
  │          gRPC output           (chuẩn hơn — làm sau nếu cần)
  ├─ correlate(event, containers) -> Option<ServiceRef>
  │     container_id → tên container → compose project/service
  └─ vòng đọc: check `paid` mỗi vòng; false → dừng

commands/falco_triage.rs
  └─ explain(events: &[FalcoEvent]) -> Explanation
       LLM nhận: rule name, output, container context — đã qua redact
       LLM KHÔNG sinh: priority, rule id, verdict "an toàn/không an toàn"

lưu trữ: security.db (phase 5) — bảng falco_events + retention
alert:  alerts.rs (Pro P2) — priority >= Critical → emit
UI: src/pages/Security.svelte → tab "Runtime"
      EventStream.svelte, EventDetail.svelte
```

**Vì sao tail file trước, gRPC sau:** file output là cấu hình mặc định, không
cần TLS, không cần thêm crate, chạy được ngay cả với Falco cài qua package
manager mà người dùng không cấu hình gì. gRPC đẹp hơn nhưng là tối ưu hoá cho
vấn đề chưa có.

**Vì sao LLM không được sinh verdict:** "an toàn" là kết luận người dùng hành
động dựa vào. Một LLM nói nhầm "an toàn" cho một event thật gây hại là loại lỗi
tệ nhất phase này có thể tạo ra. LLM viết bối cảnh và câu hỏi cần kiểm; kết luận
để người dùng.

## Related Code Files

- Create: `src-tauri/src/commands/falco_bridge.rs`, `falco_triage.rs`
- Create: `src/components/security/{EventStream,EventDetail}.svelte`
- Modify: `src-tauri/src/commands/system_capabilities.rs` — thêm `falco`
- Modify: `src-tauri/src/commands/security_history.rs` — bảng `falco_events` + retention
- Modify: `src-tauri/src/commands/alerts.rs` — nguồn alert `Runtime`
- Modify: `src-tauri/src/routes/security.rs`, `src/pages/Security.svelte`
- Modify: `src-tauri/src/commands/kb_articles.rs::seed` — bài "cài và cấu hình Falco cho Colima"

## Implementation Steps

1. ~~**Bước 0** — xác minh Falco chạy được trong Lima VM trên macOS ARM.~~ **ĐÃ QUA 2026-08-13.** Xem Gate 0 ở trên.
2. Detect Falco **trong VM** (không phải trên host) + đếm rule đang nạp + bài KB hướng dẫn cài kèm bước xác minh rule đã nạp (bài KB có giá trị kể cả khi phần còn lại chưa xong).
3. Parser event từ file output, fixture đóng băng, không phụ thuộc Falco thật trong unit test.
4. `correlate` → tên service người dùng nhận ra.
5. Lưu + retention trong `security.db`.
6. `EventStream` + lọc. Alert cho priority cao qua `alerts.rs`.
7. `explain()` — bấm mới chạy, qua redact, không sinh verdict.

## Tests

- Parser chịu được event thiếu field, rule lạ, output nhiều dòng.
- `correlate` khớp đúng container; container đã chết → hiện id thô, không crash.
- Không có Falco → tab hiện hướng dẫn, route trả 200.
- Falco đang chạy nhưng **0 rule** → trạng thái cảnh báo riêng, không phải "sẵn sàng".
- Nhị phân `falco` của Fastly (lint VCL) → **không** bị nhận nhầm là Falco CNCF.
- Entitlement hết hạn → vòng đọc dừng, có log.
- Retention xoá event cũ, không đụng `scan_runs` của phase 5.
- Payload gửi LLM không chứa đường dẫn/biến môi trường thật.

## Risks

| Rủi ro | Xử lý |
|---|---|
| **Falco không chạy được trên Colima/Lima ARM** | Bước 0 là cổng chặn. Không giả định |
| Phase trượt thành viết engine phát hiện | Mọi PR chạm `falco_bridge.rs` phải trả lời: "cái này Falco đã làm chưa?" |
| Event flood làm nghẽn UI/DB | Ngưỡng priority tối thiểu để lưu; UI ảo hoá danh sách; đếm số bị bỏ và hiển thị (không im lặng) |
| Người dùng tưởng ColimaUI đang bảo vệ họ | UI ghi rõ nguồn phát hiện là Falco và nó do người dùng cấu hình. Ta không hứa độ phủ |
| Hứa "phát hiện rootkit" trong marketing | Chỉ nói được đúng những gì rule Falco người dùng đang bật phát hiện được |

## Success Criteria

- Falco chạy sẵn → event xuất hiện trong ColimaUI kèm tên service, không phải container id thô.
- Event Critical → đúng một alert, qua đường alert đã có từ Pro P2.
- Không có Falco → không có gì hỏng, có hướng dẫn cài.
- Không dòng code nào trong repo tự phát hiện hành vi độc — mọi phát hiện đến từ Falco.
