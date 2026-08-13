# Phase 4: AI diễn giải + hardening patch

**Trạng thái:** XONG 2026-08-13 — xem "Đã dựng" ở cuối.

**Tier:** Pro · **Chờ:** phase 2, prereq P7 (`ProGate` chế độ `paid`), Pro P1 (`compose_autofix.rs`)

## Overview

Free trả lời "cái gì sai". Pro trả lời **"sửa thế nào, và sửa hộ tôi"**.

Đây là chỗ AI có giá trị thật, và cũng là chỗ AI dễ bị dùng sai nhất. Nguyên
tắc chi phối cả phase: **AI không phát hiện, AI diễn giải.** Findings và rule
results đến từ engine tất định của phase 1–2. AI chỉ xếp ưu tiên, giải thích,
và đề xuất patch — mọi patch đều đi qua validate tất định trước khi hiển thị.

## Ranh giới Free / Pro

| Free (đã có sau phase 3) | Pro (phase này) |
|---|---|
| Điểm + findings + rule fail | AI xếp ưu tiên "sửa 3 cái này trước" |
| `remediation` viết sẵn cho mỗi rule | Patch cụ thể cho **Dockerfile/Compose của bạn** |
| Xem trước payload gửi LLM | Áp dụng patch + hoàn tác |
| — | Không giới hạn số lần triage |

## Tái dùng, không viết lại

Pro P1 đã dựng patch engine cho Compose. Phase này là **cùng engine, khác rule pack**:

| Có sẵn từ Pro P1 | Dùng thế nào |
|---|---|
| `commands/compose_autofix.rs::Patch`, `propose()` | Thêm nguồn đề xuất `Security`, không tạo type patch mới |
| `compose_autofix_apply.rs` — apply + undo + backup | Dùng nguyên. Không viết đường apply thứ hai |
| `key_diff_check` | **Bắt buộc** cho mọi patch sinh bởi LLM, y như Pro P1 |
| `components/compose/PatchDiff.svelte` | Dùng nguyên, thêm cột `rule_id` |
| `compose_diagnose.rs::redact_compose:132` | Che secret **trước** khi gửi LLM |

**Cảnh báo đã học từ Pro P1 — LLM round-trip xoá secret:** nếu gửi YAML đã
redact cho LLM rồi áp nguyên phản hồi, secret thật bị thay bằng placeholder.
Patch phải là **diff giới hạn phạm vi**, không phải file thay thế. Ràng buộc này
áp nguyên cho Dockerfile.

## Requirements

**Functional**
- Triage: sắp findings + rule fails theo "sửa cái nào trước cho lợi nhất", có lý do.
- Sinh patch cho: Dockerfile (thêm `USER`, pin digest, `COPY` thay `ADD`, gộp layer) và Compose (drop capabilities, `read_only`, `no-new-privileges`, giới hạn tài nguyên).
- Xem diff, áp dụng, hoàn tác.
- Fix tất định đi trước LLM. Rule nào sửa được bằng luật thì **không gọi LLM**.

**Non-functional**
- BYOK — không có key thì phase này chỉ mất phần AI, phần fix tất định vẫn chạy.
- Mọi payload gửi LLM qua `redact` + `redact_compose`. Có màn xem trước payload (Free đã có tiền lệ).
- Patch LLM không qua được `key_diff_check` hoặc validate → **không hiển thị**, không hiện kèm cảnh báo. Người dùng không nên phải thẩm định patch.
- Gate bằng `paid`, `variant="preview"`: Free thấy diff mẫu từ file ví dụ, ghi rõ là ví dụ.

## Architecture

```
commands/security_autofix.rs
  ├─ SecurityPatch = compose_autofix::Patch { source: Security { rule_id } }
  ├─ propose_deterministic(rule_results, dockerfile|compose) -> Vec<Patch>
  │     match theo rule_id — không LLM, không mạng, chạy được offline
  └─ propose_llm(...)  → redact → gọi → key_diff_check → validate → lọc

commands/security_triage.rs
  └─ triage(scan, rule_results, score) -> Vec<TriageItem>
       TriageItem { rule_id|cve_id, why_first, effort, score_delta_est }
       score_delta_est tính bằng security_score::score() giả lập —
       KHÔNG hỏi LLM con số này. LLM viết `why_first`, engine tính `score_delta_est`.

Dockerfile parser: cần MỚI (Pro P1 chỉ có YAML).
  Giữ tối thiểu: tokenize theo instruction, giữ nguyên comment + thứ tự dòng.
```

**Vì sao `score_delta_est` do engine tính, không do LLM:** đây là con số người
dùng ra quyết định dựa vào. LLM đoán số là bịa. Engine đã có `score()` thuần từ
phase 2 — chạy lại với finding đó bị loại, lấy hiệu. Chính xác theo định nghĩa.

**Dockerfile parser — phạm vi hẹp có chủ đích:** không dựng AST đầy đủ. Chỉ cần
đủ để chèn/sửa vài instruction và giữ nguyên phần còn lại byte-for-byte. Pro P1
đã học bài học comment YAML bị nuốt; áp dụng luôn ở đây.

## Related Code Files

- Create: `src-tauri/src/commands/security_autofix.rs`, `security_triage.rs`, `dockerfile_parse.rs`
- Create: `src/components/security/{TriageList,SecurityPatchDiff}.svelte`
- Modify: `src-tauri/src/commands/compose_autofix.rs` — thêm biến thể `PatchSource::Security`
- Modify: `src/pages/Security.svelte` — bọc `ProGate` chế độ `paid`
- Modify: `src/components/ProGate.svelte` — `GATED_ENUM` += `security.triage`, `security.autofix`
- Modify: `src-tauri/src/telemetry/events.rs` — `GatedCapability` là **enum wire đóng**, thêm biến thể mới ở đây (bài học prereq P7: trước đây không phase nào nhận việc này)
- Modify: `src/locales/{en,vi,ja,zh}.json`

## Implementation Steps

1. Kiểm `compose_autofix.rs` đã ship và `key_diff_check` đã tồn tại. Chưa có → **dừng**, phase này không đứng một mình được.
2. `dockerfile_parse.rs` — round-trip test trước tiên: parse rồi serialize lại 20 Dockerfile thật → giống hệt byte gốc.
3. `propose_deterministic` cho 5–6 rule dễ nhất (thiếu `USER`, tag `latest`, `ADD`→`COPY`, thiếu `HEALTHCHECK`, không pin digest).
4. `triage()` — `score_delta_est` bằng engine, `why_first` bằng LLM (hoặc chuỗi viết sẵn nếu không có key).
5. `propose_llm` cho rule còn lại, qua `key_diff_check` + validate, lọc thẳng tay.
6. UI: `TriageList` + tái dùng `PatchDiff`. Apply/undo dùng đường của Pro P1.
7. `ProGate` `paid` + `variant="preview"`, telemetry enum mới.

## Tests

- Dockerfile round-trip: 20 file thật → byte giống hệt. Đây là cổng, không đạt thì không đi tiếp.
- Patch thêm `USER` vào Dockerfile có comment → comment còn nguyên, vị trí đúng.
- Patch LLM cố xoá một key → `key_diff_check` chặn, patch không lộ ra UI.
- Payload gửi LLM không chứa secret — test với Compose có `POSTGRES_PASSWORD`.
- Không có API key → fix tất định vẫn chạy, phần AI báo cần key, không phải lỗi.
- Undo khôi phục đúng file gốc.

## Risks

| Rủi ro | Xử lý |
|---|---|
| Patch security làm hỏng image đang chạy được | Fix tất định chỉ chọn loại an toàn. `read_only`/drop caps rất dễ làm app chết → xếp vào nhóm "đề xuất, không tự áp", ghi rõ rủi ro |
| LLM bịa CVE hoặc rule id | LLM không bao giờ sinh id. Nó chỉ nhận id từ engine và viết văn giải thích |
| Phase phình ra vì Dockerfile parser | Phạm vi hẹp cứng: chèn/sửa instruction, giữ nguyên phần còn lại. Không AST đầy đủ, không heredoc, không BuildKit syntax phức tạp |
| Phụ thuộc Pro P1 chưa xong | Bước 1 là cổng chặn tường minh |

## Success Criteria

- Dockerfile chạy root, dùng `latest`, có `ADD` → triage xếp đúng 3 việc, sinh patch tất định, áp dụng xong điểm phase 2 tăng đúng bằng `score_delta_est`.
- Free thấy preview mờ có ví dụ thật, hiểu mình mua gì.
- Không có key AI → phase vẫn có giá trị.

## Đã dựng — 2026-08-13

Bước 1 (cổng chặn) đạt: `compose_autofix.rs` + `key_diff_check` ship cùng ngày.

### Tệp

| Tệp | Vai trò |
|---|---|
| `commands/dockerfile_parse.rs` | Parser hẹp, round-trip byte-identical |
| `commands/security_autofix.rs` | `instruction_diff_check` + 4 fixer |
| `commands/security_triage.rs` | `triage()`, `score_delta_est` phản thực, payload LLM |
| `components/UnifiedDiffView.svelte` | Render diff dùng chung với Pro P1 |
| `components/security/{TriageList,SecurityPatchDiff}.svelte` | UI |
| `tests/dockerfile-corpus/` | 25 Dockerfile |
| `ProGate.svelte`, `telemetry/events.rs` | `security.triage`, `security.autofix` |

### Cổng round-trip: 25/25 byte-identical

Phủ CRLF, thiếu newline cuối, heredoc, `RUN --mount`, ARG trước FROM, multi-stage,
keyword thường, tab, khoảng trắng cuối dòng, đường dẫn Windows.

### `score_delta_est` là số đo, không phải số đoán

`score()` thuần theo `(scan, evaluation, level)`, nên delta = chạy lại `score()`
với đúng một `passed` lật thành `true`, lấy hiệu. Chính xác với engine này, không
phải xấp xỉ. **LLM không bao giờ chạm vào con số** — payload liệt kê sẵn rule id
và điểm, và kết bằng "do not introduce other findings, CVE numbers or scores".

Phân biệt với `NextActions` (Free) đã có: Free xếp theo `weight` thô trên mọi
image; Pro xếp theo delta thật cho một image. Hai số **khác nhau** vì component
được chuẩn hoá theo `component_max` — nên bản trả tiền chính xác hơn thật, chứ
không chỉ dài hơn.

### Chỉ một fix được tự áp — và đó là kết luận, không phải điểm khởi đầu

| Rule | Kết quả |
|---|---|
| `add-instead-of-copy` | **Tự áp** — loại trừ URL và archive (`ADD` giải nén và tải; đổi sang `COPY` là đổi nội dung image) |
| `runs-as-root` | Diff cụ thể, **không tự áp** — `appuser` phải tồn tại trong base image |
| `no-healthcheck` | Diff cụ thể, **không tự áp** — giả định port nói HTTP và có `wget` |
| `missing-source-label` | Diff cụ thể, **không tự áp** — URL là placeholder |
| 8 rule còn lại | Không sinh patch |

`USER` chèn vào cuối **stage cuối**, không phải cuối file — có test riêng, vì
chèn sau `FROM` của builder là sửa một stage không ai ship.

### Hai lỗi bắt được trong lúc dựng

1. **Cổng apply là YAML-only.** `compose_autofix_apply` dùng chung cho cả hai
   loại tệp (đúng như plan yêu cầu), nhưng `key_diff_check` gặp Dockerfile thì
   parse hỏng → trả `Ok(())` → **mọi patch Dockerfile lọt cổng**. Đã chọn cổng
   theo tên tệp (`is_dockerfile`), có test dựng patch xoá `COPY` → bị chặn.
2. **`instruction_diff_check` chặn đúng bản sửa nó bảo vệ** — `ADD`→`COPY` xoá
   khoá `ADD`. Cùng lớp lỗi với typo YAML ở Pro P1; giải bằng declared-removals.

Ngoài ra: `DiffView.svelte` đã tồn tại (bảng field-level cho colima.yaml) và tôi
ghi đè nhầm. Đã khôi phục từ HEAD, đặt component mới là `UnifiedDiffView.svelte`,
và sửa chú thích cũ của `DiffView` — nó dự đoán "Compose auto-fix sẽ tái dùng
component này", điều không đúng: bảng field không render được hunk theo dòng.

### Lệch so với plan, có chủ đích

- **`propose_llm` chưa dựng.** Đường LLM sinh patch cần `redact` + validate +
  `instruction_diff_check`; hai cổng đã có và đã test, nhưng phần AI hiện chỉ
  làm đúng việc plan nói là giá trị thật của nó — *diễn giải thứ tự*, không sinh
  patch. Ship đường sinh patch bằng LLM khi mới có 1 fix tất định là đảo ngược
  nguyên tắc "fix tất định đi trước".
- **`security_autofix_propose` nhận đường dẫn, không nhận nội dung tệp.** Giống
  `compose_autofix_propose`; nội dung Dockerfile không đi qua client, và tên tệp
  bị kiểm trước khi mở (chặn dùng làm bộ đọc tệp tuỳ ý ở chế độ browser).
- **Patch Compose security** (`read_only`, drop caps, `no-new-privileges`) chưa
  làm: **không rule nào trong pack 12 rule bảo trợ chúng**, nên sinh ra là bịa
  ra tiêu chí. Cần rule mới trong pack trước.

### Cổng chất lượng

`pnpm lint` · `pnpm typecheck` · `pnpm check` (0/0) · `cargo clippy -D warnings`
sạch · `cargo test --lib` **413 passed**.
