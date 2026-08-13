---
phase: 1
title: "Compose auto-fix patch engine"
status: completed
priority: P1
dependencies: ["prereq:P7", "commercial-foundation:phase-05-gate"]
effort: ""
---

# Phase 1: Compose auto-fix patch engine

> **Viết lại sau red-team 2026-08-11.** Bản đầu nhảy qua một cổng go/no-go đã thoả thuận và cam kết lại một điều đã bị bác về mặt kỹ thuật.

## Overview

`compose_diagnose.rs` trả lời được "file này hỏng vì gì" và "Knowledge Bank biết gì". Phase này trả lời câu tiếp theo: **"sửa thế nào"**.

## Bước 0 bắt buộc: chạy cổng go/no-go đã có

`260810-2258-colimaui-commercial-foundation/phase-05-compose-auto-fix-spike.md` đã quyết định auto-fix là **spike có cổng**, và cổng đó **chưa chạy**:

| Điều đã ghi ở spike | Ý nghĩa cho phase này |
|---|---|
| `:44,52,59` "Không đặt auto-patch đầy đủ làm cổng launch" | Không được cam kết auto-fix đầy đủ trước khi có dữ liệu |
| `:144` corpus đánh giá có **0 file** | Không có cơ sở nào để nói "70% sửa được" |
| `:19` giữ comment YAML **bất khả thi** với `serde_yml 0.0.12` (`Cargo.toml:29`) | Bản đầu của tôi cam kết lại đúng điều này. Sai. |

**Bước 0:** dựng corpus 15-20 file compose hỏng thật, đo tỉ lệ mỗi category có thể sửa **deterministic**, rồi mới quyết định phạm vi phase. Nếu tỉ lệ thấp, kết luận đúng có thể là "chỉ ship gợi ý, không ship apply" — và đó là kết quả hợp lệ của một cổng.

Corpus lấy từ: bug report thật (Free P2), issue GitHub, và lỗi tự dựng theo từng category trong `categorize()` (`compose_diagnose.rs:45`).

## Ranh giới Free / Pro

| Free (giữ nguyên hôm nay) | Pro (phase này) |
|---|---|
| `compose_validate` — báo lỗi | Sinh patch |
| `kb_query` — tra Knowledge Bank | Xem diff trước/sau |
| Xem trước payload LLM | Áp dụng patch + hoàn tác |
| | Không giới hạn số lần chẩn đoán qua LLM |

## Đính chính kỹ thuật: comment YAML

Bản đầu viết "sửa trên text theo dòng để giữ comment". Đúng về nguyên tắc nhưng chưa đủ:

- Với **thêm/sửa một dòng** (biến chưa định nghĩa, thiếu key): thao tác text theo dòng giữ được comment. Khả thi.
- Với **sửa cấu trúc** (indent sai, block lồng nhau hỏng): cần hiểu AST, và round-trip qua `serde_yml 0.0.12` **mất hết comment** — đã kết luận ở spike.

Nên: **không hứa giữ comment cho mọi loại fix.** Fix cấu trúc phải cảnh báo rõ "comment sẽ mất" và cho người dùng chọn. Đừng bán một lời hứa không giữ được.

Tên crate: `serde_yml`, không phải `serde_yaml`.

## Đính chính bảo mật: LLM round-trip xoá secret

`llm_payload_preview` được dựng từ `redact_compose()`, hàm này **bôi trắng** `environment:`, `secrets:`, `env_file:` (`compose_diagnose.rs:293`).

**Hệ quả bản đầu bỏ sót:** bảo model trả về file đầy đủ rồi ghi đè = **xoá sạch các block đó**, và `docker compose config --quiet` (`:212-256`) vẫn pass. Tiêu chí "0 file bị hỏng thêm" không cưỡng chế được bằng validate cú pháp.

Hai ràng buộc bắt buộc:
1. LLM **không** được trả về file đầy đủ. Nó trả về **patch có phạm vi giới hạn** (dòng nào, đổi thành gì).
2. Trước khi hiển thị bất kỳ patch nào: **diff-check theo khoá** — mọi key có trong file gốc phải còn trong file mới, trừ khi patch nói rõ là xoá key đó. Đây là kiểm tra duy nhất bắt được lỗi này; validate cú pháp thì không.

Không gửi YAML thô để né vấn đề — đó là rò rỉ credential.

## Architecture

```
commands/compose_autofix.rs
  ├─ fn propose(yaml, validation) -> Vec<Patch>
  │     match validation.category { ... }          ← match statement, KHÔNG phải trait
  │     (categorize() trả tập &'static str đóng, compose_diagnose.rs:45 —
  │      một trait + 5 struct cho tập đóng là trừu tượng thừa)
  ├─ KB: dùng KBMatch.solution nếu KB có patch kèm
  ├─ LLM: chỉ patch giới hạn phạm vi, cần xác nhận
  └─ Patch { unified_diff, new_content, explanation,
             source: Deterministic|Kb|Llm, confidence, comments_preserved: bool }

commands/compose_autofix_apply.rs
  ├─ key_diff_check(old, new) -> Result<(), Vec<MissingKey>>   ← CHẶN, không cảnh báo
  ├─ backup → app data dir, KHÔNG cạnh file gốc
  ├─ ghi file mới
  └─ undo(fix_id)
```

## Related Code Files

- Create: `src-tauri/src/commands/compose_autofix.rs`, `compose_autofix_apply.rs`
- Create: `src/components/compose/PatchDiff.svelte`, `FixHistory.svelte`
- Modify: `src-tauri/src/commands/compose_diagnose.rs` (expose `categorize`, `error_signature:83`)
- Modify: `src-tauri/src/routes/compose.rs`
- Modify: `src/components/compose/DiagnosePanel.svelte` (bọc `ProGate` chế độ `paid`)
- Modify: `src/lib/api/compose.ts`, `src/locales/{en,vi,ja,zh}.json`
- Modify: `scripts/compose-diagnose-benchmark.sh`

## Implementation Steps

1. **Bước 0** — dựng corpus, đo tỉ lệ deterministic theo category, quyết định phạm vi. Ghi kết quả vào `plans/reports/`.
2. Nếu cổng qua: `Patch` + `propose()` dạng match theo category.
3. Fix deterministic theo thứ tự dễ→khó: biến/volume/network chưa định nghĩa → file thiếu → indent đơn giản. Không cố sửa YAML hỏng nặng.
4. `key_diff_check` — viết **trước** mọi đường LLM.
5. KB strategy: mở rộng schema solution có trường patch tuỳ chọn.
6. LLM strategy: patch giới hạn phạm vi; chạy `compose_validate` **và** `key_diff_check`; trượt cái nào thì không hiển thị.
7. Apply + undo, backup trong app data, dọn backup >30 ngày.
8. `PatchDiff.svelte`: diff hai cột, hiện `source`, `confidence`, và cờ `comments_preserved`.
9. `ProGate` chế độ `paid`, `variant="preview"` — Free thấy diff mẫu từ file ví dụ, ghi rõ là ví dụ.

## Success Criteria

- [ ] **Bước 0 chạy xong và có kết luận ghi lại**, trước khi viết engine.
- [ ] Corpus 15-20 file: tỉ lệ sửa đúng đạt ngưỡng do bước 0 đặt ra, và **0 file bị làm hỏng thêm** (tuyệt đối).
- [ ] `key_diff_check` chặn được: dựng thủ công một patch xoá block `environment:` → bị từ chối, không hiển thị.
- [ ] Áp dụng rồi hoàn tác → file byte-identical với gốc.
- [ ] Fix deterministic chạy được với network tắt.
- [ ] Patch có `comments_preserved: false` hiện cảnh báo rõ trước khi áp dụng.
- [ ] Khách trả tiền không có sidecar vẫn dùng được (gate theo `paid`).

## Risk Assessment

| Rủi ro | Giảm thiểu |
|---|---|
| **LLM xoá secret mà validate không thấy** | `key_diff_check` là cổng chặn, không phải cảnh báo; LLM chỉ trả patch giới hạn phạm vi |
| Patch sai làm hỏng file | Backup + hoàn tác + criteria "0 file bị hỏng thêm" |
| Hứa giữ comment rồi không giữ được | Cờ `comments_preserved` hiển thị cho từng patch |
| Bỏ qua cổng spike lần nữa | Bước 0 là success criteria đầu tiên |
| Corpus tự sướng (chỉ mẫu dễ) | Lấy lỗi thật từ Free P2 và issue GitHub |
| Gửi YAML thô để né redact | Cấm tường minh; payload luôn đi qua `redact_compose()` |

## Bước 0 đã chạy — 2026-08-12

**Kết quả đầy đủ:** `plans/reports/from-autofix-gate-to-owner-260812-0025-compose-deterministic-fixability-verdict-report.md`

Corpus 20 file (`tests/compose-corpus/`), đo bằng `scripts/compose-autofix-gate.py`.
Toàn bộ là **synthetic** — chưa có corpus bug report thật (Free P2 chưa dựng).

### Phán quyết: **go có điều kiện, phạm vi hẹp**

| Category | Sửa được | Tổng | Tỉ lệ | Phán quyết |
|---|---:|---:|---:|---|
| `undefined_reference` | 3 | 5 | 60% | **Apply** — append khai báo, giữ comment |
| `schema` | 3 | 6 | 50% | **Apply** — chỉ ép kiểu, không di chuyển key |
| `yaml_syntax` | 1 | 5 | 20% | Chỉ gợi ý — mỗi tab là sửa được |
| `missing_file` | 0 | 1 | 0% | Chỉ gợi ý |
| `other` | 0 | 1 | 0% | Chỉ gợi ý |

Tổng 35%. **Không** ship auto-fix diện rộng.

### Bốn phát hiện đã kiểm chứng trực tiếp (Docker Compose 5.4.0)

1. **Nhánh `structure` của `categorize()` là code chết — bug đang ship.**
   `compose_diagnose.rs:70` tìm `services must be a mapping`, nhưng chuỗi đó chứa
   `must be a` nên nhánh `schema` ở `:66` bắt trước. Nửa còn lại tìm `no services`
   trong khi Docker nói `empty compose file`. **Cả hai arm đều không bao giờ chạy.**
   Sửa rẻ: đưa nhánh `structure` lên trước `schema`. Độc lập với auto-fix.

2. **Biến chưa set KHÔNG làm validate fail** — `config` exit 0, chỉ warning, và
   thay bằng chuỗi rỗng. Nghĩa là `POSTGRES_PASSWORD` chưa set validate sạch
   thành **mật khẩu rỗng**. Muốn bắt được phải đọc *warning*, thứ luồng hiện tại
   đang vứt đi.

3. **Build context thiếu cũng pass** — `docker compose config` không kiểm tra.
   Category `missing_file` hẹp hơn tên gọi nhiều.

4. **Docker chỉ báo lỗi tham chiếu ĐẦU TIÊN.** Sửa xong lộ ra cái tiếp theo →
   fixer phải **lặp tới điểm bất động**, không phải một lượt.

### Hệ quả thiết kế: `key_diff_check` như đặc tả sẽ chặn nhầm

Sửa typo `imge:` → `image:` **xoá key** `imge`, nên kiểm tra "không key nào được
biến mất" từ chối đúng cái fix nó định bảo vệ. Mệnh đề "trừ khi patch nói rõ là
xoá key đó" trong plan từ chỗ tuỳ chọn trở thành **bắt buộc**:
`key_diff_check` phải so `observed_removals − declared_removals`.

### Phạm vi đề xuất cho phase 1 (khi P7 xong)

1. `key_diff_check` có declared-removals — viết **trước tiên**.
2. Lặp tới điểm bất động.
3. Apply: khai báo volume/network; ép scalar→list; bỏ tab. Cả ba giữ comment
   (case probe giữ nguyên 3/3).
4. Chỉ gợi ý: lỗi cấu trúc YAML, file thiếu, secret, biến chưa set, di chuyển key.
5. Sửa bug thứ tự nhánh `categorize()`.
6. Đọc cả warning của Docker, không chỉ error.

## Engine đã dựng — 2026-08-13

Đúng phạm vi hẹp bước 0 đặt ra, không rộng hơn.

### Đã tạo / sửa

| Tệp | Vai trò |
|---|---|
| `src-tauri/src/commands/compose_autofix.rs` | `key_diff_check`, `Patch`, unified diff, vòng lặp `compose_autofix_propose` |
| `src-tauri/src/commands/compose_autofix_fixers.rs` | 3 fixer thuần văn bản: tab, khai báo volume/network, scalar→list |
| `src-tauri/src/commands/compose_autofix_apply.rs` | apply / undo / history, backup trong app data, prune 30 ngày |
| `src/components/compose/PatchDiff.svelte` | Diff, `source`, `confidence`, cờ `comments_preserved` |
| `src/components/compose/FixHistory.svelte` | Lịch sử fix, hoàn tác được sau khi khởi động lại app |
| `routes/compose.rs`, `payloads.rs`, `api_server.rs`, `lib.rs` | 4 lệnh Tauri + 4 route HTTP |
| `src/lib/api/compose.ts`, `src/locales/{en,vi,ja,zh}.json` | Client + 15 khoá dịch |

### Đo trên corpus thật (Docker Compose 5.4.0)

`cargo test --lib -- --ignored corpus` — **7/20 giải quyết trọn vẹn (35%)**, đúng
bằng con số bước 0 dự báo. **0 patch bị `key_diff_check` từ chối** (không fixer
nào xoá khoá mà không khai báo) và **0 file bị làm hỏng thêm** — tiêu chí tuyệt
đối, kiểm bằng cách ghi patch ra rồi validate lại chứ không tin `resolved`.

### Ba quyết định đáng ghi

1. **Tệp scratch nằm cạnh tệp gốc, không nằm trong temp dir.** `build:` và
   `env_file:` phân giải tương đối với thư mục chứa compose file, nên validate
   một bản sao ở nơi khác là validate một project khác. Xoá khi drop.
2. **`key_diff_check` chạy lại lúc apply.** Nội dung đi qua client rồi quay lại;
   một lệnh tin caller đã kiểm hộ là một lệnh không được kiểm. Test dựng patch
   xoá block `environment:` — bị chặn, tệp trên đĩa không đổi.
3. **Undo khôi phục nguyên byte**, vì backup là byte đọc từ đĩa chứ không phải
   serialize lại — điều duy nhất an toàn với tệp đầy comment.

### Một bug đã bắt trong lúc port

Fixer scalar→list của bản Python quote lồng hai lần: `ports: "8080:80"` thành
`- ""8080:80""`, tức đổi giá trị chứ không phải đổi kiểu. Bản Rust kiểm tra giá
trị đã được quote chưa; có test cho cả hai nhánh.

### Cố ý không làm

- **Fixer sửa typo khoá** (`imge:` → `image:`): bước 0 xếp `schema` là "chỉ ép
  kiểu, không di chuyển khoá". Cơ chế `declared_removals` mà nó cần đã có và đã
  test, nên thêm sau là rẻ.
- **Đường KB và đường LLM**: `PatchSource::Kb|Llm` đã có trong kiểu dữ liệu,
  chưa có nguồn sinh. Chúng phụ thuộc schema solution của KB (bước 5) và
  security phase 4.
- **`scripts/compose-diagnose-benchmark.sh`**: test corpus trong Rust chạy đúng
  đường dẫn sản phẩm thật; thêm một đường đo thứ hai là trùng lặp.

### Cổng chất lượng

`pnpm lint` · `pnpm typecheck` · `pnpm check` (0 lỗi / 0 cảnh báo) ·
`cargo clippy -D warnings` sạch · `cargo test --lib` **387 passed**.

### Cảnh báo về con số 35%

Corpus synthetic — đúng cái bias "corpus tự sướng" mà plan cảnh báo. Bằng chứng:
**2/20 case hoá ra không hỏng** (phát hiện 2 và 3), tức chính giả định của người
soạn corpus cũng đã sai. Chạy lại gate với file thật khi Free P2 xong;
`scripts/compose-autofix-gate.py <corpus-dir>` nhận tham số thư mục cho việc đó.
