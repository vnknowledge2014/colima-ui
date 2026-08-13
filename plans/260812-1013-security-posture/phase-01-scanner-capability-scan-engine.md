# Phase 1: Scanner capability + scan engine

**Tier:** Free · **Chờ:** prereq P4 (`run_cmd_streaming`), prereq P2 (redact mở rộng)
· **Trạng thái: DONE 2026-08-12** (xem "Kết quả" ở cuối file)

## Overview

Trả lời đúng một câu: *"image này có gì trong nó, và có lỗ hổng nào đã biết?"*
Không chấm điểm (phase 2), không UI đầy đủ (phase 3), không AI (phase 4).

Phase này dựng: detect scanner → chạy scan có streaming progress → chuẩn hoá
output về một shape duy nhất → cache theo image digest.

## Bước 0 bắt buộc: chọn scanner mặc định

Chưa chốt Trivy hay Grype. Đo trước, quyết sau. Ghi kết quả vào
`plans/reports/from-security-phase1-to-owner-<date>-scanner-bakeoff-report.md`.

Corpus: 15 image thật lấy từ `docker images` của máy dev + 5 image phổ biến
(`nginx:latest`, `node:20-alpine`, `postgres:16`, `python:3.12-slim`, `redis:7`).

| Đo | Vì sao quan trọng |
|---|---|
| Thời gian scan lần đầu (DB lạnh) và lần sau (DB ấm) | Quyết định có cần progress streaming hay chỉ spinner |
| Kích thước DB sau khi tải | Xác nhận giả định "không bundle được" |
| Có xuất được SBOM không, format gì (SPDX / CycloneDX) | Phase 3 cần |
| Shape JSON có ổn định giữa 2 minor version không | Quyết định độ chặt của parser |
| Có chạy được khi **không có mạng** với DB đã tải sẵn | Acceptance "offline" của plan phụ thuộc câu này |
| Số finding lệch nhau bao nhiêu giữa hai công cụ trên cùng image | Nếu lệch lớn → điểm ở phase 2 phải gắn với công cụ, không được so chéo |

**Cổng go/no-go:** nếu scan lần sau (DB ấm) trên image ~200MB vượt 60s, thiết kế
UI của phase 3 phải là "chạy nền + thông báo khi xong", không phải "bấm và chờ".

---

## Bước 0 ĐÃ CHẠY — 2026-08-12

**Phán quyết: GO. Scanner mặc định = Trivy.**
Báo cáo đầy đủ: `plans/reports/from-security-phase1-to-owner-260812-1259-scanner-bakeoff-report.md`
Script: `scripts/security-scanner-bakeoff.py`. Corpus: 23 image thật, 6MB–4.86GB.

### Cổng: QUA rộng rãi

Image ~200MB: Trivy **2.5s** — nhanh hơn ngưỡng 60s khoảng 23 lần. UI phase 3
**được phép** "bấm và chờ". Nhưng image 4.86GB mất 49.5s → ngưỡng theo kích
thước, **>2GB chuyển sang chạy nền**.

### Năm thay đổi bắt buộc cho phase này

1. **`ScannerKind` chỉ có `Trivy`.** Không dựng trừu tượng cho công cụ thứ hai.
2. **Bỏ detect `syft`.** Trivy sinh SBOM native cả CycloneDX lẫn SPDX-JSON; Grype
   chỉ tiêu thụ SBOM chứ không sinh. Bớt một binary phải dò, một install hint,
   một bài KB.
3. **`ScanResult` phải biểu diễn được thất bại của từng image** kèm lý do đọc
   được. Đo thật: Trivy hỏng 2/23 image (`failed to initialize the struct from
   the temporary file … not found in tar`), Grype hỏng 5/23. Scan hỏng là kết quả
   bình thường của một image cụ thể, **không được làm hỏng cả danh sách**.
4. **Ngưỡng UI theo kích thước image**, không phải một hằng số thời gian.
5. **Nếu sau này thêm Grype: BẮT BUỘC set `DOCKER_HOST`** qua
   `path_util.rs::detect_docker_host()`. Lý do ở mục dưới.

### Cái bẫy Grype — ghi lại để không ai vấp

Grype **không đọc docker context, chỉ đọc `DOCKER_HOST`**. Colima để biến đó
trống. Grype không thấy daemon rồi **im lặng rơi xuống kéo image từ registry công
cộng** — gửi tên image riêng tư của người dùng tới Docker Hub:

```
oci-registry: GET https://index.docker.io/v2/library/recipe-nutrition-platform-api/manifests/latest
  UNAUTHORIZED: authentication required
```

Vi phạm thẳng cam kết "không gửi danh sách image đi đâu" của `plan.md`. Export
`DOCKER_HOST` thì hết (đã kiểm chứng). Trivy không có bẫy này.

### Số liệu chốt

| | Trivy | Grype |
|---|---|---|
| DB trên đĩa | **1229 MB** | 1996 MB |
| Tải DB lần đầu | 41.6s | 59.0s |
| Scan image 200MB | **2.5s** | không đo sạch được (số của Grype là thời gian tải mạng) |
| Sinh SBOM | ✅ CycloneDX + SPDX | ❌ |
| Đọc daemon Colima mặc định | ✅ | ❌ |
| Image hỏng | 2/23 | 5/23 |

Plan ước tính DB Trivy ~700MB; thực tế **1.2GB**. Quyết định "detect, không
bundle" càng đúng.

### Severity: enum đóng an toàn, nhưng phải chuẩn hoá chữ hoa

Không công cụ nào phát ra nhãn ngoài tập dự kiến trên cả 23 image. Nhưng Trivy
viết `CRITICAL`/`HIGH`, Grype viết `Critical`/`High` và có thêm `Negligible`.
Chuẩn hoá tại biên parse — đúng như đặc tả bên dưới đã yêu cầu.

### Chênh lệch giữa hai scanner: tới 13×

`postgres:17.6.1.158` — Trivy 28 finding, Grype 373. `postgrest` thì ngược lại
789 vs 301. Không có hướng nhất quán. **Xác nhận bằng số** quy tắc của plan: điểm
phải gắn cứng với `(scanner, scanner_version, db_snapshot_date, pack_version)`,
và nếu sau này đổi scanner thì lịch sử điểm ở phase 5 phải **đứt đoạn** y như khi
đổi rule pack.

## Requirements

**Functional**
- Detect `trivy`, `syft` (và/hoặc `grype`) qua đúng pattern `system_capabilities.rs`.
- Scan một image theo id/tag → danh sách finding đã chuẩn hoá.
- Sinh SBOM cho một image → lưu được ra file.
- Cache kết quả theo **image digest**, không theo tag. Tag di động, digest thì không.
- Huỷ được scan đang chạy.

**Non-functional**
- Scan chạy qua `run_cmd_streaming` (prereq P4). Không `output()` chặn — scan là lệnh chạy phút.
- Kết quả scan qua `redact::redact` trước khi ra khỏi process (bundle/telemetry/LLM).
- Không tự động scan. Scan là hành động người dùng bấm.
- Không có scanner → trả `CapabilityState::Missing` + install hint, **không phải lỗi**.

## Architecture

```
commands/security_scan.rs
  ├─ ScannerKind { Trivy, Grype }          ← chốt ở bước 0
  ├─ Finding { id, package, installed_version, fixed_version,
  │            severity, source, cvss, published }
  │     severity: enum đóng — Critical|High|Medium|Low|Unknown
  │     KHÔNG dùng String: mỗi scanner đặt tên khác nhau, chuẩn hoá tại biên
  ├─ ScanResult { image_digest, image_ref, findings, sbom_ref,
  │               scanner, scanner_version, db_snapshot_date, scanned_at }
  │     db_snapshot_date là BẮT BUỘC — phase 2 cần nó để giải thích điểm
  ├─ scan_image(image_ref, on_progress) -> ScanResult
  └─ cache: HashMap<digest, ScanResult> + TTL,
       cùng pattern LazyLock<Mutex<_>> như system_capabilities.rs

commands/system_capabilities.rs   (MODIFY)
  └─ detect_all_blocking() += trivy, syft
     install_hint() += arm mới;  doc_id() += slug KB mới

routes/security.rs   (NEW)  →  đăng ký trong routes/mod.rs + api_server.rs
  ├─ POST /api/security/scan        (stream progress qua SSE có sẵn)
  ├─ GET  /api/security/scan/:digest
  └─ POST /api/security/sbom
```

**Vì sao `severity` là enum đóng, không phải String:** Trivy dùng
`CRITICAL/HIGH/...`, Grype dùng `Critical/High/...`, một số nguồn dùng
`Important`. Chuẩn hoá ở biên parse; nếu để String chảy vào, phase 2 chấm điểm
sẽ phải đoán và phase 5 lọc theo ngưỡng sẽ sai âm thầm.

**Vì sao cache theo digest:** người dùng scan `myapp:latest`, rebuild, scan lại
— cùng tag, khác image. Cache theo tag sẽ trả kết quả của image cũ, và đó là
loại lỗi không ai phát hiện ra cho tới khi nó quan trọng.

## Related Code Files

- Create: `src-tauri/src/commands/security_scan.rs`, `src-tauri/src/routes/security.rs`
- Create: `src/lib/api/security.ts`
- Modify: `src-tauri/src/commands/system_capabilities.rs` — `detect_all_blocking`, `install_hint`, `doc_id`
- Modify: `src-tauri/src/commands/mod.rs`, `src-tauri/src/routes/mod.rs`, `src-tauri/src/api_server.rs`, `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/redact.rs` — kiểm tra pattern có phủ nội dung SBOM (đường dẫn build, tên nội bộ)
- Modify: `src/lib/api/index.ts`

## Implementation Steps

1. **Bước 0** — bakeoff scanner, ghi report, chốt `ScannerKind` mặc định.
2. Thêm scanner vào `detect_all_blocking()`. Test: capability mới có install hint khi thiếu, không có khi đã cài (test hiện có đã bao pattern này).
3. `Finding` + `ScanResult` + parser cho scanner đã chốt. Test bằng fixture JSON đóng băng trong `src-tauri/tests/fixtures/`, **không** gọi scanner thật trong unit test.
4. `scan_image` trên `run_cmd_streaming`, có huỷ.
5. Cache theo digest + TTL + `invalidate()`.
6. Route + SSE progress. Dùng lại `publish_sse_event` — **không** dựng kênh mới.
7. SBOM export ra file, qua `assert_path_within` (`validation.rs:169`) theo chính sách path của prereq P4.
8. `src/lib/api/security.ts` — client thuần, chưa UI.

## Tests

- Parser: fixture của 3 image (0 finding / vài chục / hàng trăm) → shape đúng, severity map đúng.
- Parser gặp field lạ / version JSON mới → không panic, đánh dấu `Unknown`, không mất finding.
- Cache: hai lần scan cùng digest → một lần chạy scanner. Khác digest cùng tag → hai lần chạy.
- Capability: không có `trivy` → `Missing` + hint, và route trả 200 với trạng thái, **không phải 5xx**.
- Redact: SBOM chứa đường dẫn `/Users/<name>/...` và biến môi trường → bị che.

## Risks

| Rủi ro | Xử lý |
|---|---|
| Shape JSON của scanner đổi giữa các version | Parser khoan dung: field lạ bỏ qua, field thiếu → `Unknown`, không panic. Ghi `scanner_version` vào kết quả |
| Scan image lớn ăn hết RAM của Lima VM | Scan chạy trên host (trivy đọc qua docker socket), không trong VM. Xác nhận ở bước 0 |
| DB CVE chưa tải → lần scan đầu tải 700MB im lặng | Progress phải hiển thị rõ giai đoạn "đang tải DB", tách khỏi giai đoạn scan |
| Người dùng scan image chưa pull | Trả lỗi rõ ràng + đề nghị pull, không tự pull (pull là hành động tốn băng thông, phải do người dùng quyết) |

## Success Criteria

- [x] Scan `nginx:alpine` trả findings có severity đã chuẩn hoá + `db_snapshot_date`.
- [x] Không có trivy → UI nhận được `Missing` + install hint, app không báo lỗi.
- [x] Huỷ scan giữa chừng → process con chết, không rò rỉ (dùng lại
      `streaming_cmd`, đã có process-group kill).
- [ ] Toàn bộ chạy được khi rút mạng — **chưa kiểm bằng airplane mode**, vẫn nằm
      ở Unresolved của báo cáo bakeoff. `--skip-db-update` đã dùng, nhưng chứng
      minh cờ hoạt động không phải là chứng minh offline.

## Kết quả — 2026-08-12

13 unit test + 3 test end-to-end (`#[ignore]`, đã chạy thật với Trivy 0.73.0 trên
máy dev). `cargo test` 322/322, clippy `-D warnings` sạch.

**Tạo:** `commands/security_scan.rs`, `routes/security.rs`,
`src/lib/api/security.ts`, 4 bài KB `install-trivy`, 4 fixture JSON,
`tests/security_scan_against_trivy.rs`.
**Sửa:** `system_capabilities.rs` (+`trivy`), `kb_articles.rs`, `mod.rs` ×2,
`api_server.rs`, `lib.rs`, `docs/api.md`.

### Quyết định trong lúc code

**`--scanners vuln`.** Unresolved #3 của bakeoff: Trivy bật quét secret mặc định,
nghĩa là nó đọc **nội dung file** trong image. Phase này hỏi đúng một câu về lỗ
hổng package; đọc nội dung file là phạm vi rộng hơn câu hỏi, nên tắt.

**Tải DB là một giai đoạn riêng, có tên.** `trivy image --download-db-only` chạy
trước, phát sự kiện `stage: "database"`, rồi scan chạy với `--skip-db-update`.
Gộp làm một thì lần đầu là gần một phút im lặng (DB 1.2GB, đo được 41.6s), và
người dùng không phân biệt được "đang tải" với "treo". Tách ra cũng là cái làm
cho scan chạy offline được.

**Không có `syft`, không có `ScannerKind::Grype`.** Đúng theo bakeoff.

**SBOM không redact.** `redact` dành cho thứ rời khỏi máy (bundle, telemetry,
LLM). SBOM là file người dùng chủ động xuất ra đĩa của chính họ; che nội dung sẽ
làm hỏng chính tài liệu họ vừa yêu cầu. Finding trả về UI chỉ có tên package và
version — không có đường dẫn host.

### Ba lỗi thật, tìm ra ở review

1. **Cancel không bao giờ được disarm.** `scanId` do client đặt, nên một cancel
   tới sau khi scan đã xong sẽ nằm lại trong `PENDING_CANCEL` và **giết scan kế
   tiếp dùng lại id đó ngay khi vừa sinh**. Sửa bằng `CancelGuard` (Drop), vì
   hàm có nhiều đường ra kể cả `?`.
2. **Cancel không dừng được giai đoạn tải DB** — nó chạy dưới job id
   `{scan_id}-db`. Đúng giai đoạn dài nhất, đúng giai đoạn người ta bỏ cuộc.
3. **Điểm CVSS không tất định.** Trivy trả một điểm cho mỗi vendor trong một map,
   `values().find_map()` lấy phần tử tuỳ thứ tự duyệt — cùng một report cho 9.8
   lần này, 7.5 lần sau. Phase 2 chấm điểm từ đó thì điểm sẽ nhảy mà image không
   đổi. Giờ: NVD trước, rồi v3 cao nhất, rồi v2 cao nhất.

Kèm: SBOM ghi qua file tạm rồi rename và từ chối ghi đè khi chưa cho phép (giống
`file_transfer`); cache dọn bản hết hạn khi ghi; `invalidate()` được gọi thật —
khi DB đổi ngày, vì một DB mới có thể biến image sạch thành image có lỗ hổng mà
image không đổi gì; finding thiếu id vẫn được đếm (`id: "unknown"`) thay vì bị bỏ
im lặng.
