# Phase 2: Rule pack + scoring engine

**Tier:** Free · **Chờ:** phase 1 · **Trạng thái: DONE 2026-08-12**
(xem "Kết quả" ở cuối file)

## Overview

Phase 1 nói "image có gì". Phase này nói **"nó tốt hay tệ, và vì sao"**.

Hai thứ, cùng một phase vì tách ra là ranh giới giả — điểm không có nghĩa nếu
không có rule pack, rule pack không hiển thị được nếu không có điểm:

1. **Rule pack** — checklist cấu hình machine-checkable, có phiên bản.
2. **Scoring engine** — hàm thuần biến findings + rule results thành điểm phân rã được.

## Bước 0 — ĐÃ CHẠY 2026-08-12. Phán quyết: GO, chặt hơn bản plan này viết

Báo cáo đầy đủ kèm URL và nguyên văn từng giấy phép:
`plans/reports/from-security-phase2-step0-to-owner-260812-2109-rule-source-licensing-verdict-report.md`

| Nguồn | Giấy phép **thật** (kiểm 2026-08-12) | Được làm gì |
|---|---|---|
| CIS Docker Benchmark v1.8.0 | **CC BY-NC-SA 4.0** (bản PDF miễn phí) — không phải "ToU hạn chế" chung chung | **Chỉ số hiệu**, kèm phiên bản (`CIS Docker Benchmark v1.8.0 §4.1`). Mô tả, lý do, cách sửa: tự viết 100% |
| Docker Bench for Security | Apache-2.0 ✅ như plan viết | Đọc **cách kiểm tra**, tự viết lại. Port logic thì phải có NOTICE |
| OWASP Docker Top 10 (D01–D10) | **CC BY-NC-SA 4.0** — plan ghi sai là CC-BY-SA | Chỉ mã mục + tên nhóm rủi ro. NC nghĩa là **ghi nguồn cũng không đủ** cho sản phẩm bán tiền |
| OWASP CSVS | CC BY-SA 4.0 ✅ — nhưng repo **archived từ 2019** | Mượn **ý tưởng ba mức** L1/L2/L3, tự định nghĩa mỗi mức gồm rule nào |
| NIST SP 800-190 | Public domain (US Gov), trừ phần đánh dấu có bản quyền | **Nguồn duy nhất copy được**, kèm ghi nguồn |

**Điều đổi so với giả định của plan:** trong 5 nguồn chỉ **1 nguồn (NIST)** copy
được. CIS và OWASP Docker Top 10 đều mang điều khoản **NonCommercial**, mà
ColimaUI có tier trả phí — nên phái sinh văn bản của họ không phải "nên tránh",
mà là **vi phạm giấy phép**.

**Không** copy-paste mô tả rule từ bất kỳ nguồn nào ở trên. Số hiệu rule là
dữ kiện; văn bản là tác phẩm có bản quyền.

### Ba ràng buộc rơi xuống các bước sau

1. **`standard_refs` chỉ có `{ standard, version, id }` — không có trường `text`.**
   Schema không cho chỗ dán thì không ai dán nhầm. Đây là biện pháp thật, khác với
   một dòng nhắc nhở trong plan.
2. **Cấm chuỗi UI ngụ ý chứng nhận**: "CIS Certified", "CIS compliant",
   "OWASP compliant", logo CIS/OWASP. "CIS Benchmarks™" là nhãn hiệu của họ —
   rủi ro này độc lập với bản quyền. Nói được: *"tham chiếu CIS Docker Benchmark
   v1.8.0"*.
3. **Repo chưa có `LICENSE` lẫn `NOTICE`.** Cần có trước khi commit rule pack v1,
   vì rule pack thừa hưởng giấy phép của repo — mà giấy phép đó chưa tồn tại.

### Success criterion "rà thủ công" cần thay

"Không dòng nào copy — rà thủ công" không kiểm chứng lại được. Bước 2 phải chọn
một trong hai: test n-gram đối chiếu corpus tham chiếu để ngoài repo, hoặc
`authored_by` + ngày trên mỗi rule kèm yêu cầu trả lời nguồn khi thêm rule.

## Đính chính quan trọng về "follow OWASP"

OWASP Top 10 (web application) **không áp dụng** cho image scanning. Nhầm chỗ
này thì checklist sẽ đầy rule vô nghĩa (SQL injection trong một image?).

Chuẩn đúng cho container, theo thứ tự mức độ machine-checkable:

| Chuẩn | Số rule dùng được | Dạng |
|---|---|---|
| CIS Docker Benchmark §4 (Image & Build) + §5 (Runtime) | ~40 | Kiểm tra được tự động, ID ổn định |
| OWASP Docker Top 10 | 10 | Nguyên tắc — mỗi mục ánh xạ ra 2–5 rule cụ thể |
| OWASP CSVS L1/L2/L3 | — | Dùng làm **mức nghiêm ngặt** người dùng chọn |

## Requirements

**Functional**
- Rule pack là **file dữ liệu có phiên bản**, nhúng trong binary làm baseline.
- Mỗi rule: `id`, `title`, `rationale`, `remediation`, `severity`, `standard_refs[]`, `check`.
- Chạy rule trên: image config (`docker inspect`), image history/layers, và findings từ phase 1.
- Điểm 0–100, **luôn phân rã được** thành các thành phần con.
- Chế độ nghiêm ngặt: L1 (mặc định) / L2 / L3 — cùng rule pack, khác tập rule bật.

**Non-functional — quan trọng nhất**
- `score = f(findings, rule_results, rule_pack_version, db_snapshot_date)` — **hàm thuần**. Cùng đầu vào → cùng đầu ra, mãi mãi.
- Điểm **không bao giờ** hiển thị trần. Luôn kèm rule pack version + ngày DB.
- Không so điểm giữa hai scanner khác nhau. Điểm gắn với `(scanner, db_date, pack_version)`.

## Architecture

```
commands/security_rules.rs
  ├─ RULE_PACK: &str = include_str!("../resources/rules/v1.json")
  │     nhúng lúc build → offline luôn chạy, không phụ thuộc mạng
  ├─ Rule { id, title, rationale, remediation, severity,
  │         standard_refs: Vec<StandardRef>, check: Check }
  ├─ Check — enum đóng, KHÔNG phải script:
  │     RunsAsRoot | NoHealthcheck | LatestTag | ExposesPort(range)
  │   | HasSecretEnvPattern | WritableRootFs | AddInsteadOfCopy
  │   | NoUserInstruction | ExcessiveLayers(n) | StaleBaseImage(days)
  └─ evaluate(image_config, history, level) -> Vec<RuleResult>

commands/security_score.rs
  ├─ ScoreBreakdown {
  │     vulnerabilities: u8,   ← từ severity mix của findings
  │     hardening:       u8,   ← từ rule_results
  │     provenance:      u8,   ← có digest pin? base image biết nguồn?
  │     freshness:       u8,   ← tuổi base image vs bản mới nhất đã biết
  │     total:           u8,
  │     inputs: ScoreInputs { pack_version, db_snapshot_date, scanner, level }
  │   }
  └─ score(&ScanResult, &[RuleResult], Level) -> ScoreBreakdown

src-tauri/resources/rules/v1.json    (NEW — artifact dữ liệu)
```

**Vì sao `Check` là enum đóng, không phải script/expression:**
Rule pack sẽ được tải từ mạng ở phase 3. Một rule pack chứa script tuỳ ý,
tải từ mạng, chạy trên máy người dùng — đó là remote code execution có thiết kế.
Enum đóng nghĩa là rule pack chỉ **cấu hình** các kiểm tra đã được compile vào
binary; một pack độc hại tệ nhất chỉ báo sai, không chạy được gì.

**Vì sao 4 thành phần chứ không 1 số:**
`nginx:latest` 42/100 không nói được gì. "Vulnerabilities 30/40, Hardening
5/25, Provenance 0/20, Freshness 7/15" nói ngay rằng vấn đề là dùng tag
`latest` và chạy root, không phải CVE.

## Related Code Files

- Create: `src-tauri/src/commands/security_rules.rs`, `security_score.rs`
- Create: `src-tauri/resources/rules/v1.json`
- Modify: `src-tauri/src/routes/security.rs` — thêm `GET /api/security/score/:digest`
- Modify: `src-tauri/src/commands/security_scan.rs` — `ScanResult` gắn thêm image config
- Modify: `src/lib/api/security.ts`

## Implementation Steps

1. **Bước 0** — rà giấy phép, ghi phán quyết.
2. `Check` enum + `evaluate()` cho 8–10 rule đầu, tự viết mô tả. Không cố phủ hết 110 rule CIS ngay.
3. Rule pack v1 JSON + schema + test "pack nhúng parse được".
4. `ScoreBreakdown` + `score()` thuần. Test bằng bảng đầu vào → đầu ra cố định.
5. Chế độ L1/L2/L3 — cùng pack, cờ `min_level` trên mỗi rule.
6. Route `/api/security/score/:digest`.
7. Đo trên corpus 20 image của phase 1: phân bố điểm có phân biệt được image tốt/tệ không? Nếu mọi image đều 40–50 thì công thức vô dụng — **chỉnh trọng số trước khi ship**.

## Tests

- `score()` là hàm thuần: cùng đầu vào chạy 100 lần → cùng kết quả.
- Điểm 0 finding + pass hết rule = 100. Mọi rule fail + nhiều Critical → gần 0.
- Rule pack lạ (field thừa, rule id không biết) → bỏ qua rule đó, không panic, không đổi điểm rule khác.
- `ScoreInputs` luôn có mặt trong response — test contract, vì UI phase 3 phụ thuộc.
- L3 bật nhiều rule hơn L1 trên cùng image → điểm hardening ≤ điểm ở L1.

## Risks

| Rủi ro | Xử lý |
|---|---|
| Điểm không phân biệt được image tốt/tệ | Bước 7 là cổng: đo phân bố trên corpus trước khi ship |
| Người dùng so điểm giữa hai máy có DB khác ngày → thấy khác nhau, mất tin | Hiển thị `db_snapshot_date` cạnh mọi điểm, không phải trong tooltip |
| Rule pack tải về chứa payload độc | `Check` là enum đóng — pack không thể mô tả hành vi mới. Cộng chữ ký ở phase 3 |
| Sa đà viết 110 rule CIS | Ship 10 rule đúng và giải thích tốt hơn 110 rule copy-paste. Mở rộng theo phản hồi |

## Success Criteria

- [x] Bước 0: phán quyết giấy phép, có bằng chứng nguồn (2026-08-12).
- [x] `node:20-alpine` **63** vs `node:latest` **46** — đo thật, không phải giả định.
- [x] Mọi điểm hiển thị kèm đầu vào (`ScoreInputs`, có test contract).
- [x] `standard_refs` không có trường văn bản (schema không cho chỗ dán).
- [ ] Chuỗi UI ngụ ý chứng nhận — **chưa áp dụng được**, phase 2 không có UI.
      Ràng buộc chuyển sang phase 3.
- [ ] Kiểm chống copy bằng test/quy trình PR — **chưa làm**. Rule v1 tự viết và
      đã được reviewer đọc đối chiếu, nhưng đó vẫn là kiểm một lần.

## Kết quả — 2026-08-12

`cargo test` 345/345 (23 test mới), clippy `-D warnings` sạch, 2 test e2e chạy
thật với Trivy + Docker.

**Tạo:** `resources/rules/v1.json` (12 rule), `commands/security_rules.rs`,
`commands/security_score.rs`, `tests/security_score_against_corpus.rs`,
`LICENSE` (MIT), `NOTICE`.
**Sửa:** `security_scan.rs` (+`audit_image_blocking`), `routes/security.rs`
(+`POST /api/security/audit`, `GET /api/security/rules`), `lib.rs`,
`api_server.rs`, `src/lib/api/security.ts`, `docs/api.md`, `package.json`,
`Cargo.toml`.

### Bước 7 (cổng hiệu chỉnh) — chạy thật, và nó bắt được lỗi

Điểm trên 12 image thật của máy dev. Lần đo **đầu tiên** cho thấy công thức hỏng:
với `VULNERABILITY_SATURATION = 300`, **6/11 image ăn đúng 0 điểm** ở thành phần
lỗ hổng — không phân biệt được image tệ với image rất tệ. Đo phân bố thật
(weighted 36 → 2881) rồi đặt lại ngưỡng **1200**. Sau khi sửa: spread 46..80,
8 giá trị phân biệt.

Đây chính là lý do bước 7 tồn tại. Nếu ship theo con số đoán ban đầu thì mọi
image có lịch sử đều hiện 0 và người dùng học được rằng thành phần đó vô nghĩa.

### Quyết định thiết kế đáng ghi

**Mẫu số cố định, rule không bật thì không mất điểm.** Đó là cái làm cho
"L3 ≤ L1" đúng theo cấu trúc chứ không phải theo may mắn: nếu chuẩn hoá theo tập
rule đang bật, một rule L3 *pass* có thể **nâng** tỉ lệ và L3 hoá ra cao hơn L1.

**Rule không chạy được ≠ rule pass.** Pack mới có `check` mà build cũ không hiểu
→ trọng số rời khỏi **mẫu số** (`22/22` thay vì `25/25`), kèm `skippedRules` và
`engineVersion` trong `ScoreInputs`. Bản đầu cho không điểm — hai build cùng pack
cùng image ra hai tổng khác nhau mà không có gì giải thích.

**Bằng chứng của rule secret chỉ có tên biến, không bao giờ có giá trị.** Một
rule in ra secret nó tìm được là đã phát tán secret đó vào notification list,
diagnostic bundle và ảnh chụp màn hình. Có test khoá lại điều này.

**Tag "2.0" cũng là tag di động.** Không chỉ `latest`: major/minor-only được xuất
bản lại khi có patch, cùng với `stable`/`edge`/`main`/`nightly`.

### Ba lỗi do review tìm ra

1. **Rule bị skip được tính là pass** (đã mô tả trên) — nghiêm trọng nhất vì
   phase 5 sẽ lưu lịch sử điểm, và điểm không so được giữa các build là điểm vô
   dụng.
2. **`parse_rfc3339_ms` bỏ qua offset múi giờ.** `docker inspect` trả `Z` nhưng
   `image history` trả `+07:00`; đọc nhầm lệch tới 14 giờ, đủ để một image nhảy
   qua mốc 90 ngày. Kèm: `2023-02-29` từng được nhận.
3. **`AUTH` khớp cả `MAINTAINER_AUTHOR`**, và exception khớp kiểu substring lại
   nuốt mất `AUTH_METHOD_SECRET` — sai cả hai chiều.
